//! Instanced SDF stroke renderer with chunk culling and a cached dry layer.

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use rupok_core::StrokeData;
use rupok_ink::{StrokePoint, point_radius};

use crate::{Camera, pack_color};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct InkVertex {
    pub pos: [f32; 2],
    pub radius: f32,
    pub color: [u8; 4],
    pub depth: f32,
}

const STRIDE: u64 = std::mem::size_of::<InkVertex>() as u64;
const SEP: InkVertex = InkVertex {
    pos: [0.0, 0.0],
    radius: -1.0,
    color: [0; 4],
    depth: 1.0,
};
/// Vertices per culling chunk.
const CHUNK: usize = 2048;
/// Depth spacing between strokes: supports ~4M strokes before a renumber.
const DEPTH_STEP: f32 = 1.0 / (1u32 << 22) as f32;
const MAX_DEPTH_IDX: u32 = (1 << 22) - 2;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

// The same vertex buffer is bound four times (prev, a, b, next).
const A_PREV: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];
const A_CUR: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![1 => Float32x3, 2 => Unorm8x4, 3 => Float32];
const A_B: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![4 => Float32x3];
const A_NEXT: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![5 => Float32x3];

#[derive(Clone, Copy, Debug)]
struct Slot {
    start: usize,
    len: usize,
    depth_idx: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RenderStats {
    pub strokes: usize,
    pub vertices: usize,
    pub dead_vertices: usize,
    pub segments_drawn: u32,
    pub draw_calls: u32,
    pub dry_redrawn: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    cam: [f32; 2],
    scale: f32,
    _pad0: f32,
    size: [f32; 2],
    _pad1: [f32; 2],
}

struct Layer {
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    depth_view: wgpu::TextureView,
    w: u32,
    h: u32,
}

pub struct InkRenderer {
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    blit_pipeline: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
    dry_globals: wgpu::Buffer,
    dry_bg: wgpu::BindGroup,
    wet_globals: wgpu::Buffer,
    wet_bg: wgpu::BindGroup,

    /// Invariant: `[SEP] + (stroke points + [SEP])* + [SEP]`.
    verts: Vec<InkVertex>,
    vbuf: wgpu::Buffer,
    vcap: usize,
    upload_from: Option<usize>,
    tombstones: Vec<(usize, usize)>,
    slots: HashMap<String, Slot>,
    chunk_bounds: Vec<[f32; 4]>,
    dead: usize,
    next_depth: u32,
    appended_since_sort: usize,

    wet: Vec<InkVertex>,
    wet_buf: wgpu::Buffer,
    wet_cap: usize,

    layer: Option<Layer>,
    dry_dirty: bool,
    last_view: Option<(Camera, f32, u32, u32)>,
}

fn empty_bounds() -> [f32; 4] {
    [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ]
}

fn grow(b: &mut [f32; 4], v: &InkVertex) {
    if v.radius < 0.0 {
        return;
    }
    b[0] = b[0].min(v.pos[0] - v.radius);
    b[1] = b[1].min(v.pos[1] - v.radius);
    b[2] = b[2].max(v.pos[0] + v.radius);
    b[3] = b[3].max(v.pos[1] + v.radius);
}

/// Morton (Z-order) code of the 512-world-unit cell containing (x, y).
fn morton_cell(x: f32, y: f32) -> u64 {
    fn spread(v: u32) -> u64 {
        let mut v = u64::from(v);
        v = (v | (v << 16)) & 0x0000_FFFF_0000_FFFF;
        v = (v | (v << 8)) & 0x00FF_00FF_00FF_00FF;
        v = (v | (v << 4)) & 0x0F0F_0F0F_0F0F_0F0F;
        v = (v | (v << 2)) & 0x3333_3333_3333_3333;
        v = (v | (v << 1)) & 0x5555_5555_5555_5555;
        v
    }
    let cx = ((x / 512.0).floor() as i64 + (1 << 31)) as u32;
    let cy = ((y / 512.0).floor() as i64 + (1 << 31)) as u32;
    spread(cx) | (spread(cy) << 1)
}

fn intersects(a: &[f32; 4], b: &[f32; 4]) -> bool {
    a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1]
}

/// Convert stroke points into GPU vertices (single-point strokes become a dot).
pub fn stroke_vertices(
    points: &[StrokePoint],
    width: f32,
    offset: [f32; 2],
    color: [u8; 4],
    depth: f32,
    out: &mut Vec<InkVertex>,
) {
    let mut push = |p: &StrokePoint| {
        out.push(InkVertex {
            pos: [p.x + offset[0], p.y + offset[1]],
            radius: point_radius(p.p, width),
            color,
            depth,
        })
    };
    for p in points {
        push(p);
    }
    if points.len() == 1 {
        push(&points[0]);
    }
}

impl InkRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let ink_shader = device.create_shader_module(wgpu::include_wgsl!("ink.wgsl"));
        let blit_shader = device.create_shader_module(wgpu::include_wgsl!("blit.wgsl"));

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ink globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let mk_globals = |label| {
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: std::mem::size_of::<Globals>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &globals_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                }],
            });
            (buf, bg)
        };
        let (dry_globals, dry_bg) = mk_globals("dry globals");
        let (wet_globals, wet_bg) = mk_globals("wet globals");

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ink"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });

        let vb = |attrs: &'static [wgpu::VertexAttribute]| wgpu::VertexBufferLayout {
            array_stride: STRIDE,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: attrs,
        };
        let buffers = [
            Some(vb(&A_PREV)),
            Some(vb(&A_CUR)),
            Some(vb(&A_B)),
            Some(vb(&A_NEXT)),
        ];

        let premul = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ink"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &ink_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &buffers,
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &ink_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(premul),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let blit_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit"),
            bind_group_layouts: &[Some(&blit_layout)],
            immediate_size: 0,
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit"),
            layout: Some(&blit_pl),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let vcap = 64 * 1024;
        let vbuf = Self::make_vbuf(device, vcap, "ink vertices");
        let wet_cap = 8 * 1024;
        let wet_buf = Self::make_vbuf(device, wet_cap, "wet vertices");

        Self {
            format,
            pipeline,
            blit_pipeline,
            blit_layout,
            dry_globals,
            dry_bg,
            wet_globals,
            wet_bg,
            verts: vec![SEP, SEP],
            vbuf,
            vcap,
            upload_from: Some(0),
            tombstones: Vec::new(),
            slots: HashMap::new(),
            chunk_bounds: Vec::new(),
            dead: 0,
            next_depth: 0,
            appended_since_sort: 0,
            wet: Vec::new(),
            wet_buf,
            wet_cap,
            layer: None,
            dry_dirty: true,
            last_view: None,
        }
    }

    fn make_vbuf(device: &wgpu::Device, cap: usize, label: &str) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: cap as u64 * STRIDE,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn stroke_count(&self) -> usize {
        self.slots.len()
    }

    pub fn contains(&self, id: &str) -> bool {
        self.slots.contains_key(id)
    }

    fn depth_of(idx: u32) -> f32 {
        1.0 - (idx + 1) as f32 * DEPTH_STEP
    }

    fn bump_chunks(&mut self, idx: usize) {
        let v = self.verts[idx];
        let c = idx / CHUNK;
        while self.chunk_bounds.len() <= c {
            self.chunk_bounds.push(empty_bounds());
        }
        grow(&mut self.chunk_bounds[c], &v);
        // A segment starting in the previous chunk ends here.
        if idx.is_multiple_of(CHUNK) && c > 0 {
            grow(&mut self.chunk_bounds[c - 1], &v);
        }
    }

    /// Remove all strokes.
    pub fn clear(&mut self) {
        self.verts = vec![SEP, SEP];
        self.slots.clear();
        self.chunk_bounds.clear();
        self.tombstones.clear();
        self.dead = 0;
        self.next_depth = 0;
        self.appended_since_sort = 0;
        self.upload_from = Some(0);
        self.dry_dirty = true;
    }

    /// Append a stroke on top. Replaces an existing stroke with the same id.
    pub fn add_stroke(&mut self, id: &str, s: &StrokeData) {
        if self.slots.contains_key(id) {
            self.remove_stroke(id);
        }
        if s.points.is_empty() {
            return;
        }
        if self.next_depth >= MAX_DEPTH_IDX {
            self.compact();
        }
        let depth_idx = self.next_depth;
        self.next_depth += 1;
        let color = pack_color(s.color, s.opacity);
        // Pop trailing pad, append points + SEP, restore pad.
        self.verts.pop();
        let start = self.verts.len();
        stroke_vertices(
            &s.points,
            s.width,
            s.offset,
            color,
            Self::depth_of(depth_idx),
            &mut self.verts,
        );
        let len = self.verts.len() - start;
        self.verts.push(SEP);
        self.verts.push(SEP);
        for i in start..start + len {
            self.bump_chunks(i);
        }
        self.slots.insert(
            id.to_string(),
            Slot {
                start,
                len,
                depth_idx,
            },
        );
        self.appended_since_sort += 1;
        self.upload_from = Some(self.upload_from.map_or(start, |u| u.min(start)));
        self.dry_dirty = true;
    }

    /// Tombstone a stroke in place (no buffer rebuild).
    pub fn remove_stroke(&mut self, id: &str) -> bool {
        let Some(slot) = self.slots.remove(id) else {
            return false;
        };
        for v in &mut self.verts[slot.start..slot.start + slot.len] {
            v.radius = -1.0;
        }
        self.tombstones.push((slot.start, slot.len));
        self.dead += slot.len + 1;
        self.dry_dirty = true;
        true
    }

    /// Rebuild the vertex array from live strokes: drops tombstones, renumbers
    /// depth in z-order, and lays strokes out by spatial cell (Morton order) so
    /// culling chunks are spatially coherent. Draw order no longer has to match
    /// z-order because the per-stroke depth test resolves stacking.
    pub fn compact(&mut self) {
        let mut order: Vec<(String, Slot)> = self.slots.drain().collect();
        order.sort_by_key(|(_, s)| s.depth_idx);
        let old = std::mem::replace(&mut self.verts, vec![SEP]);
        // Assign new depth numbers in z-order, then a spatial key per stroke.
        let mut placed: Vec<(u64, u32, String, Slot)> = order
            .into_iter()
            .enumerate()
            .map(|(z, (id, slot))| {
                let mut b = empty_bounds();
                for v in &old[slot.start..slot.start + slot.len] {
                    grow(&mut b, v);
                }
                let key = morton_cell((b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5);
                (key, z as u32, id, slot)
            })
            .collect();
        placed.sort_by_key(|(key, z, _, _)| (*key, *z));
        self.chunk_bounds.clear();
        for (_, z, id, slot) in placed {
            let start = self.verts.len();
            let depth = Self::depth_of(z);
            for v in &old[slot.start..slot.start + slot.len] {
                self.verts.push(InkVertex { depth, ..*v });
            }
            self.verts.push(SEP);
            self.slots.insert(
                id,
                Slot {
                    start,
                    len: slot.len,
                    depth_idx: z,
                },
            );
        }
        self.next_depth = self.slots.len() as u32;
        self.verts.push(SEP);
        for i in 0..self.verts.len() {
            self.bump_chunks(i);
        }
        self.dead = 0;
        self.appended_since_sort = 0;
        self.tombstones.clear();
        self.upload_from = Some(0);
        self.dry_dirty = true;
    }

    /// Re-sort spatially if many strokes were appended since the last sort.
    /// Call when idle (e.g. pen up for a while, or after loading a lesson).
    pub fn maintain(&mut self) -> bool {
        if self.appended_since_sort > 4096 || (self.dead > 4096 && self.dead * 4 > self.verts.len())
        {
            self.compact();
            return true;
        }
        false
    }

    /// Replace the wet (in-progress) stroke.
    pub fn set_wet(&mut self, points: &[StrokePoint], width: f32, color: u32, opacity: f32) {
        self.wet.clear();
        self.wet.push(SEP);
        stroke_vertices(
            points,
            width,
            [0.0, 0.0],
            pack_color(color, opacity),
            0.5,
            &mut self.wet,
        );
        self.wet.push(SEP);
        self.wet.push(SEP);
    }

    /// Show several wet strokes at once (multi-pen boards, lasso outline, replay).
    pub fn set_wet_many<'a>(
        &mut self,
        strokes: impl IntoIterator<Item = (&'a [StrokePoint], f32, u32, f32)>,
    ) {
        self.wet.clear();
        self.wet.push(SEP);
        for (i, (pts, w, c, o)) in strokes.into_iter().enumerate() {
            stroke_vertices(
                pts,
                w,
                [0.0, 0.0],
                pack_color(c, o),
                0.5 - i as f32 * 1e-4,
                &mut self.wet,
            );
            self.wet.push(SEP);
        }
        self.wet.push(SEP);
    }

    pub fn clear_wet(&mut self) {
        self.wet.clear();
    }

    pub fn has_wet(&self) -> bool {
        self.wet.len() > 3
    }

    /// Whether a frame must be drawn for this view.
    pub fn needs_redraw(&self, cam: Camera, dpr: f32, w: u32, h: u32) -> bool {
        self.dry_dirty || self.last_view != Some((cam, dpr, w, h))
    }

    /// Force the dry layer to re-render on the next frame.
    pub fn invalidate(&mut self) {
        self.dry_dirty = true;
    }

    fn sync_gpu(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.verts.len() > self.vcap {
            self.vcap = self.verts.len().next_power_of_two();
            self.vbuf = Self::make_vbuf(device, self.vcap, "ink vertices");
            self.upload_from = Some(0);
        }
        for (start, end) in pending_uploads(
            self.upload_from.take(),
            &mut self.tombstones,
            self.verts.len(),
        ) {
            queue.write_buffer(
                &self.vbuf,
                start as u64 * STRIDE,
                bytemuck::cast_slice(&self.verts[start..end]),
            );
        }
        if self.wet.len() > self.wet_cap {
            self.wet_cap = self.wet.len().next_power_of_two();
            self.wet_buf = Self::make_vbuf(device, self.wet_cap, "wet vertices");
        }
        if self.has_wet() {
            queue.write_buffer(&self.wet_buf, 0, bytemuck::cast_slice(&self.wet));
        }
    }

    fn ensure_layer(&mut self, device: &wgpu::Device, w: u32, h: u32) {
        if self.layer.as_ref().is_some_and(|l| l.w == w && l.h == h) {
            return;
        }
        let size = wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        };
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("dry ink layer"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ink depth"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit"),
            layout: &self.blit_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });
        self.layer = Some(Layer {
            view,
            bind_group,
            depth_view: depth.create_view(&Default::default()),
            w,
            h,
        });
        self.dry_dirty = true;
    }

    /// Visible instance ranges `[lo, hi)` over the dry vertex array.
    fn visible_ranges(&self, vis: [f32; 4]) -> Vec<(usize, usize)> {
        let last_instance = self.verts.len().saturating_sub(2); // exclusive
        let mut ranges: Vec<(usize, usize)> = Vec::new();
        for (c, b) in self.chunk_bounds.iter().enumerate() {
            if !intersects(b, &vis) {
                continue;
            }
            let lo = (c * CHUNK).max(1);
            let hi = ((c + 1) * CHUNK).min(last_instance);
            if lo >= hi {
                continue;
            }
            match ranges.last_mut() {
                Some(r) if r.1 == lo => r.1 = hi,
                _ => ranges.push((lo, hi)),
            }
        }
        ranges
    }

    fn bind_and_draw(pass: &mut wgpu::RenderPass<'_>, buf: &wgpu::Buffer, lo: usize, hi: usize) {
        for slot in 0..4u32 {
            let off = (lo + slot as usize - 1) as u64 * STRIDE;
            pass.set_vertex_buffer(slot, buf.slice(off..));
        }
        pass.draw(0..4, 0..(hi - lo) as u32);
    }

    fn write_globals(
        queue: &wgpu::Queue,
        buf: &wgpu::Buffer,
        cam: Camera,
        dpr: f32,
        w: u32,
        h: u32,
    ) {
        let g = Globals {
            cam: [cam.x, cam.y],
            scale: cam.zoom * dpr,
            _pad0: 0.0,
            size: [w as f32, h as f32],
            _pad1: [0.0; 2],
        };
        queue.write_buffer(buf, 0, bytemuck::bytes_of(&g));
    }

    /// Render one frame into `target` (`w`×`h` device px). `dpr` converts CSS
    /// px to device px. The target is cleared to transparent so the ink layer
    /// composites over the DOM (objects, images, page background).
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        w: u32,
        h: u32,
        cam: Camera,
        dpr: f32,
    ) -> RenderStats {
        self.ensure_layer(device, w, h);
        self.sync_gpu(device, queue);
        if self.last_view != Some((cam, dpr, w, h)) {
            self.dry_dirty = true;
        }
        let mut stats = RenderStats {
            strokes: self.slots.len(),
            vertices: self.verts.len(),
            dead_vertices: self.dead,
            ..Default::default()
        };
        let layer = self.layer.as_ref().expect("layer");
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ink frame"),
        });

        if self.dry_dirty {
            Self::write_globals(queue, &self.dry_globals, cam, dpr, w, h);
            let vis_css = cam.visible(w as f32 / dpr, h as f32 / dpr);
            let ranges = self.visible_ranges(vis_css);
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("dry ink"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &layer.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &layer.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.dry_bg, &[]);
            for (lo, hi) in ranges {
                Self::bind_and_draw(&mut pass, &self.vbuf, lo, hi);
                stats.segments_drawn += (hi - lo) as u32;
                stats.draw_calls += 1;
            }
            drop(pass);
            stats.dry_redrawn = true;
        }

        {
            if self.has_wet() {
                Self::write_globals(queue, &self.wet_globals, cam, dpr, w, h);
            }
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &layer.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, &layer.bind_group, &[]);
            pass.draw(0..3, 0..1);
            stats.draw_calls += 1;
            if self.has_wet() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.wet_bg, &[]);
                let hi = self.wet.len() - 2;
                Self::bind_and_draw(&mut pass, &self.wet_buf, 1, hi);
                stats.segments_drawn += (hi - 1) as u32;
                stats.draw_calls += 1;
            }
        }
        queue.submit(Some(enc.finish()));
        self.dry_dirty = false;
        self.last_view = Some((cam, dpr, w, h));
        stats
    }
}

/// Vertex ranges `[start, end)` to write to the GPU: the appended tail from
/// `upload_from`, plus every tombstone *before* it. A stroke replaced in place
/// (removed, then re-added at the end in the same frame) leaves a tombstone
/// ahead of the tail; skipping it would keep drawing the old geometry.
fn pending_uploads(
    upload_from: Option<usize>,
    tombstones: &mut Vec<(usize, usize)>,
    len: usize,
) -> Vec<(usize, usize)> {
    let mut out = Vec::with_capacity(tombstones.len() + 1);
    for (start, n) in tombstones.drain(..) {
        if upload_from.is_none_or(|from| start < from) {
            out.push((start, (start + n).min(upload_from.unwrap_or(len)).min(len)));
        }
    }
    if let Some(from) = upload_from {
        out.push((from, len));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stroke_replaced_in_one_frame_uploads_its_tombstone() {
        // Old copy at 1..11 tombstoned, new copy appended from 20.
        let mut t = vec![(1, 10)];
        assert_eq!(
            pending_uploads(Some(20), &mut t, 40),
            vec![(1, 11), (20, 40)]
        );
        assert!(t.is_empty());
        // Tombstones inside the tail are covered by the tail upload.
        let mut t = vec![(25, 5)];
        assert_eq!(pending_uploads(Some(20), &mut t, 40), vec![(20, 40)]);
        // Removal only.
        let mut t = vec![(3, 4)];
        assert_eq!(pending_uploads(None, &mut t, 40), vec![(3, 7)]);
    }

    #[test]
    fn dot_strokes_get_two_vertices() {
        let mut out = Vec::new();
        stroke_vertices(
            &[StrokePoint {
                x: 1.0,
                y: 2.0,
                p: 1.0,
                dt: 0,
            }],
            4.0,
            [10.0, 0.0],
            [0, 0, 0, 255],
            0.5,
            &mut out,
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].pos, [11.0, 2.0]);
        assert_eq!(out[0].radius, 2.0);
    }

    #[test]
    fn depth_decreases_with_order() {
        assert!(InkRenderer::depth_of(1) < InkRenderer::depth_of(0));
        assert!(InkRenderer::depth_of(MAX_DEPTH_IDX) > 0.0);
    }
}
