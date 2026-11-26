//! Instanced SDF stroke renderer with chunk culling and a cached dry layer.
//!
//! While the pen writes, nothing here scales with the screen: the ink that is
//! changing (the wet stroke, and strokes just finished) is drawn into a small
//! *wet tile*, a second target the size of a handwriting area that the host
//! lays over the main one. The main target is presented only when dry ink or
//! the camera changes, or when wet ink leaves the tile. See
//! docs/performance.md, "The wet tile".

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use rupok_core::StrokeData;
use rupok_ink::{Brush, StrokePoint};

use crate::{Camera, pack_color};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct InkVertex {
    pub pos: [f32; 2],
    pub radius: f32,
    pub color: [u8; 4],
    pub depth: f32,
    /// Distance along the stroke from its first point (dashes, rainbow).
    pub arc: f32,
    /// `[brush id, width × 64 (u16, little end first), 0]` (`ink.wgsl`).
    pub style: [u8; 4],
}

const STRIDE: u64 = std::mem::size_of::<InkVertex>() as u64;
const SEP: InkVertex = InkVertex {
    pos: [0.0, 0.0],
    radius: -1.0,
    color: [0; 4],
    depth: 1.0,
    arc: 0.0,
    style: [0; 4],
};
/// Vertices per culling chunk.
const CHUNK: usize = 2048;
/// Depth spacing between strokes: supports ~4M strokes before a renumber.
const DEPTH_STEP: f32 = 1.0 / (1u32 << 22) as f32;
const MAX_DEPTH_IDX: u32 = (1 << 22) - 2;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

// The same vertex buffer is bound four times (prev, a, b, next).
const A_PREV: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];
const A_CUR: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![1 => Float32x3, 2 => Unorm8x4, 3 => Float32, 6 => Float32, 7 => Unorm8x4];
const A_B: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![4 => Float32x3];
const A_NEXT: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![5 => Float32x3];
/// Side of the square noise texture (pencil grain, chalk).
const NOISE_SIZE: u32 = 256;

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
    /// The whole dry layer was drawn again (pan, zoom, erase, resize).
    pub dry_redrawn: bool,
    /// Only strokes added on top were drawn into the dry layer.
    pub dry_appended: bool,
}

/// A rectangle of the main target, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl TileRect {
    fn contains(&self, b: &[f32; 4]) -> bool {
        b[0] >= self.x as f32
            && b[1] >= self.y as f32
            && b[2] <= (self.x + self.w) as f32
            && b[3] <= (self.y + self.h) as f32
    }
}

/// What the next frame has to draw ([`InkRenderer::plan`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FramePlan {
    /// Present the main target ([`InkRenderer::render_main`]).
    pub main: bool,
    /// Present the wet tile ([`InkRenderer::render_tile`], then
    /// [`InkRenderer::tile_presented`]).
    pub tile: bool,
    /// Where the tile lies over the main target after this frame.
    pub rect: Option<TileRect>,
}

/// More pending vertices than this and the main target is presented instead.
const MAX_PENDING_VERTS: usize = 24 * 1024;
/// The depth the tile gives the `k`-th pending dry stroke: behind all wet ink
/// (0.5 and nearer), later strokes nearer.
fn pending_depth(k: usize) -> f32 {
    0.9 - (k.min(30_000) as f32) * 1e-5
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
    /// The wet ink is shown by something else (a native layer): not drawn.
    wet_hidden: bool,
    wet_buf: wgpu::Buffer,
    wet_cap: usize,

    layer: Option<Layer>,
    /// The dry layer must be drawn again from scratch.
    dry_dirty: bool,
    /// Strokes added on top since the dry layer was drawn: `(start, len)`.
    appended: Vec<(usize, usize)>,
    last_view: Option<(Camera, f32, u32, u32)>,

    // ---- the wet tile (see the module docs)
    mask_pipeline: wgpu::RenderPipeline,
    /// Size of the tile target; `None` draws everything in the main target.
    tile_size: Option<(u32, u32)>,
    /// Where the tile lies, as of the last frame that used it.
    tile_rect: Option<TileRect>,
    /// The main target shows something the tile cannot stand in for.
    main_dirty: bool,
    /// The wet vertices as the main target last drew them (outside
    /// `tile_rect`; everywhere without one).
    baked: Vec<InkVertex>,
    /// Dry strokes added since the main target was presented: `(start, len)`.
    pending: Vec<(usize, usize)>,
    pending_verts: usize,
    planned: FramePlan,
    /// What the tile has to show, and what it shows now.
    tile_verts: Vec<InkVertex>,
    tile_drawn: Vec<InkVertex>,
    tile_drawn_rect: Option<TileRect>,
    tile_stale: bool,
    /// Depth for [`InkRenderer::render_tile`] (a renderer used as the tile's).
    tile_depth: Option<(wgpu::TextureView, u32, u32)>,
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
    let r = v.radius * reach_of(v.style[0]);
    b[0] = b[0].min(v.pos[0] - r);
    b[1] = b[1].min(v.pos[1] - r);
    b[2] = b[2].max(v.pos[0] + r);
    b[3] = b[3].max(v.pos[1] + r);
}

fn reach_of(id: u8) -> f32 {
    Brush::ALL.get(usize::from(id)).map_or(1.0, |b| b.reach())
}

/// Tileable value noise (`NOISE_SIZE`², R8): four octaves on integer
/// lattices that divide the size, so it wraps without a seam. Made once per
/// renderer and kept on the GPU; never per frame.
pub fn noise_texels() -> Vec<u8> {
    fn hash(x: u32, y: u32, o: u32) -> f32 {
        let mut h =
            x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841) ^ o.wrapping_mul(0xcb1a_b31f);
        h ^= h >> 13;
        h = h.wrapping_mul(0x5bd1_e995);
        h ^= h >> 15;
        (h & 0xffff) as f32 / 65535.0
    }
    let n = NOISE_SIZE;
    let mut acc = vec![0.0f32; (n * n) as usize];
    let octaves = [(64u32, 0.5f32), (32, 0.25), (16, 0.15), (4, 0.1)];
    for (o, (cells, amp)) in octaves.into_iter().enumerate() {
        let step = n / cells;
        for y in 0..n {
            for x in 0..n {
                let (cx, cy) = (x / step, y / step);
                let (fx, fy) = (
                    (x % step) as f32 / step as f32,
                    (y % step) as f32 / step as f32,
                );
                let s = |t: f32| t * t * (3.0 - 2.0 * t);
                let (sx, sy) = (s(fx), s(fy));
                let at = |i: u32, j: u32| hash(i % cells, j % cells, o as u32);
                let top = at(cx, cy) + (at(cx + 1, cy) - at(cx, cy)) * sx;
                let bot = at(cx, cy + 1) + (at(cx + 1, cy + 1) - at(cx, cy + 1)) * sx;
                acc[(y * n + x) as usize] += amp * (top + (bot - top) * sy);
            }
        }
    }
    let (lo, hi) = acc
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    acc.iter()
        .map(|v| (((v - lo) / (hi - lo).max(1e-6)) * 255.0).round() as u8)
        .collect()
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

fn is_empty(b: &[f32; 4]) -> bool {
    b[0] > b[2] || b[1] > b[3]
}

fn union(a: &mut [f32; 4], b: &[f32; 4]) {
    a[0] = a[0].min(b[0]);
    a[1] = a[1].min(b[1]);
    a[2] = a[2].max(b[2]);
    a[3] = a[3].max(b[3]);
}

/// World bounds of `verts` (separators skipped).
fn bounds_of(verts: &[InkVertex]) -> [f32; 4] {
    let mut b = empty_bounds();
    for v in verts {
        grow(&mut b, v);
    }
    b
}

/// The strokes of a wet vertex list (`SEP`-separated runs).
fn strokes_of(verts: &[InkVertex]) -> impl Iterator<Item = &[InkVertex]> {
    verts.split(|v| v.radius < 0.0).filter(|s| !s.is_empty())
}

/// World bounds of everything that differs between two wet vertex lists,
/// stroke by stroke. A stroke that only grew differs in its tail: from two
/// points before the first changed one, because a segment's shape also
/// depends on its neighbours (`ink.wgsl`).
fn changed_bounds(wet: &[InkVertex], baked: &[InkVertex]) -> [f32; 4] {
    let mut out = empty_bounds();
    let mut a = strokes_of(wet);
    let mut b = strokes_of(baked);
    loop {
        let (x, y) = (a.next(), b.next());
        if x.is_none() && y.is_none() {
            return out;
        }
        let (x, y) = (x.unwrap_or(&[]), y.unwrap_or(&[]));
        let same = x.iter().zip(y).take_while(|(p, q)| p == q).count();
        if same == x.len() && same == y.len() {
            continue;
        }
        let from = same.saturating_sub(2);
        union(&mut out, &bounds_of(&x[from.min(x.len())..]));
        union(&mut out, &bounds_of(&y[from.min(y.len())..]));
    }
}

/// Convert stroke points into GPU vertices (single-point strokes become a
/// dot). Radii come from the brush ([`Brush::radii`]); each vertex carries
/// its distance along the stroke and the brush for the shader.
#[allow(clippy::too_many_arguments)]
pub fn stroke_vertices(
    points: &[StrokePoint],
    width: f32,
    brush: Brush,
    offset: [f32; 2],
    color: [u8; 4],
    depth: f32,
    out: &mut Vec<InkVertex>,
) {
    let w64 = (width * 64.0).round().clamp(0.0, 65535.0) as u16;
    let style = [brush.shader_id(), w64 as u8, (w64 >> 8) as u8, 0];
    let tapered = brush.shapes_by_path();
    let mut radii = Vec::new();
    if tapered {
        brush.radii(points, width, &mut radii);
    }
    let mut arc = 0.0f32;
    let mut prev: Option<&StrokePoint> = None;
    for (i, p) in points.iter().enumerate() {
        if let Some(q) = prev {
            arc += (p.x - q.x).hypot(p.y - q.y);
        }
        prev = Some(p);
        out.push(InkVertex {
            pos: [p.x + offset[0], p.y + offset[1]],
            radius: if tapered {
                radii[i]
            } else {
                brush.radius(p.p, width)
            },
            color,
            depth,
            arc,
            style,
        });
    }
    if points.len() == 1 {
        let v = out[out.len() - 1];
        out.push(v);
    }
}

impl InkRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let ink_shader = device.create_shader_module(wgpu::include_wgsl!("ink.wgsl"));
        let blit_shader = device.create_shader_module(wgpu::include_wgsl!("blit.wgsl"));

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ink globals"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        // The brush noise texture: uploaded once, sampled by pencil and chalk.
        let noise = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("brush noise"),
            size: wgpu::Extent3d {
                width: NOISE_SIZE,
                height: NOISE_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &noise,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &noise_texels(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(NOISE_SIZE),
                rows_per_image: Some(NOISE_SIZE),
            },
            wgpu::Extent3d {
                width: NOISE_SIZE,
                height: NOISE_SIZE,
                depth_or_array_layers: 1,
            },
        );
        let noise_view = noise.create_view(&Default::default());
        let noise_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("brush noise"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
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
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&noise_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&noise_sampler),
                    },
                ],
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
            // Its pass has no depth attachment: nothing to clear at full size.
            depth_stencil: None,
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

        let mask_shader = device.create_shader_module(wgpu::include_wgsl!("mask.wgsl"));
        let mask_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mask"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let mask_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mask"),
            layout: Some(&mask_pl),
            vertex: wgpu::VertexState {
                module: &mask_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &mask_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::empty(),
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
            wet_hidden: false,
            wet_buf,
            wet_cap,
            layer: None,
            dry_dirty: true,
            appended: Vec::new(),
            last_view: None,
            mask_pipeline,
            tile_size: None,
            tile_rect: None,
            main_dirty: true,
            baked: Vec::new(),
            pending: Vec::new(),
            pending_verts: 0,
            planned: FramePlan::default(),
            tile_verts: Vec::new(),
            tile_drawn: Vec::new(),
            tile_drawn_rect: None,
            tile_stale: false,
            tile_depth: None,
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
        self.touch_all();
    }

    /// Everything dry must be drawn again, and the main target presented.
    fn touch_all(&mut self) {
        self.dry_dirty = true;
        self.main_dirty = true;
        self.appended.clear();
        self.pending.clear();
        self.pending_verts = 0;
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
            s.brush(),
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
        // A stroke on top of everything: the dry layer only needs this one
        // drawn over it, and until the main target is next presented the
        // wet tile can show it (`plan`).
        if !self.dry_dirty {
            self.appended.push((start, len));
        }
        if !self.main_dirty {
            self.pending.push((start, len));
            self.pending_verts += len;
        }
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
        self.touch_all();
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
        self.touch_all();
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
    pub fn set_wet(
        &mut self,
        points: &[StrokePoint],
        width: f32,
        brush: Brush,
        color: u32,
        opacity: f32,
    ) {
        self.wet.clear();
        self.wet.push(SEP);
        stroke_vertices(
            points,
            width,
            brush,
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
        strokes: impl IntoIterator<Item = (&'a [StrokePoint], f32, Brush, u32, f32)>,
    ) {
        self.wet.clear();
        self.wet.push(SEP);
        for (i, (pts, w, b, c, o)) in strokes.into_iter().enumerate() {
            stroke_vertices(
                pts,
                w,
                b,
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

    /// Whether there is wet ink to draw.
    pub fn has_wet(&self) -> bool {
        !self.wet_hidden && self.wet.len() > 3
    }

    /// Stop (or resume) drawing the wet ink while something else shows it:
    /// the native wet-ink layer of the Windows app. The strokes are kept.
    pub fn set_wet_hidden(&mut self, hidden: bool) {
        self.wet_hidden = hidden;
    }

    /// Whether a frame must be drawn for this view (everything in one
    /// target; with a wet tile, ask [`InkRenderer::plan`]).
    pub fn needs_redraw(&self, cam: Camera, dpr: f32, w: u32, h: u32) -> bool {
        self.dry_dirty
            || self.main_dirty
            || !self.pending.is_empty()
            || self.wet_shown() != self.baked.as_slice()
            || self.last_view != Some((cam, dpr, w, h))
    }

    /// Force the dry layer to re-render on the next frame.
    pub fn invalidate(&mut self) {
        self.touch_all();
    }

    /// Present the main target on the next frame, whatever changed (the
    /// tile then shows only what is still wet).
    pub fn request_main(&mut self) {
        self.main_dirty = true;
    }

    /// The wet vertices, empty when there is no wet ink.
    fn wet_shown(&self) -> &[InkVertex] {
        if self.has_wet() { &self.wet } else { &[] }
    }

    // ------------------------------------------------------------ wet tile

    /// Draw changing ink into a separate tile target of this size (device
    /// px), or back into the main target with `None`.
    pub fn set_tile_size(&mut self, size: Option<(u32, u32)>) {
        let size = size.filter(|(w, h)| *w > 0 && *h > 0);
        if self.tile_size != size {
            self.tile_size = size;
            self.tile_rect = None;
            self.main_dirty = true;
            self.tile_stale = true;
        }
    }

    pub fn tile_size(&self) -> Option<(u32, u32)> {
        self.tile_size
    }

    /// Where the tile lies over the main target (device px).
    pub fn tile_rect(&self) -> Option<TileRect> {
        self.tile_rect
    }

    /// The tile target lost its contents (resized, reconfigured).
    pub fn invalidate_tile(&mut self) {
        self.tile_stale = true;
    }

    /// Dry strokes the main target does not show yet.
    pub fn pending_strokes(&self) -> usize {
        self.pending.len()
    }

    /// Decide what this frame draws. The main target is needed when dry ink
    /// other than strokes added on top changed, when the view changed, or
    /// when the changing ink does not fit the tile; otherwise the tile alone
    /// is redrawn (or nothing). `flush` asks for the pending strokes to go
    /// to the main target now (the pen is resting).
    ///
    /// Nothing is committed here: [`InkRenderer::render_main`] and
    /// [`InkRenderer::tile_presented`] do that, so a frame that could not
    /// be drawn is planned again.
    pub fn plan(&mut self, cam: Camera, dpr: f32, w: u32, h: u32, flush: bool) -> FramePlan {
        if self.last_view != Some((cam, dpr, w, h)) {
            self.dry_dirty = true;
        }
        if self.dry_dirty {
            self.main_dirty = true;
        }
        self.tile_verts.clear();
        let tile = self
            .tile_size
            .map(|(tw, th)| (tw.min(w), th.min(h)))
            .filter(|(tw, th)| *tw > 0 && *th > 0);
        let Some((tw, th)) = tile else {
            let main = self.main_dirty
                || !self.pending.is_empty()
                || self.wet_shown() != self.baked.as_slice();
            self.planned = FramePlan {
                main,
                tile: false,
                rect: None,
            };
            return self.planned;
        };

        let scale = cam.zoom * dpr;
        // World bounds → device px of the main target, with room for the
        // antialiased edge; clipped to the target.
        let to_px = |b: [f32; 4]| -> Option<[f32; 4]> {
            if is_empty(&b) {
                return None;
            }
            let p = [
                ((b[0] - cam.x) * scale - 2.0).floor().max(0.0),
                ((b[1] - cam.y) * scale - 2.0).floor().max(0.0),
                ((b[2] - cam.x) * scale + 2.0).ceil().min(w as f32),
                ((b[3] - cam.y) * scale + 2.0).ceil().min(h as f32),
            ];
            (!is_empty(&p)).then_some(p)
        };
        let fits = |p: &[f32; 4]| p[2] - p[0] <= tw as f32 && p[3] - p[1] <= th as f32;
        // The pen's tip and where it is heading (device px), to put the tile
        // ahead of the pen rather than around it.
        let (tip, heading) = {
            let wet = self.wet_shown();
            let last = wet.iter().rposition(|v| v.radius >= 0.0);
            match last {
                Some(i) => {
                    let back = wet[..i]
                        .iter()
                        .rev()
                        .take(24)
                        .take_while(|v| v.radius >= 0.0)
                        .last()
                        .unwrap_or(&wet[i]);
                    let t = [
                        (wet[i].pos[0] - cam.x) * scale,
                        (wet[i].pos[1] - cam.y) * scale,
                    ];
                    let d = [
                        (wet[i].pos[0] - back.pos[0]) * scale,
                        (wet[i].pos[1] - back.pos[1]) * scale,
                    ];
                    let n = d[0].hypot(d[1]);
                    let d = if n > 1.0 {
                        [d[0] / n, d[1] / n]
                    } else {
                        [0.0, 0.0]
                    };
                    (Some(t), d)
                }
                None => (None, [0.0, 0.0]),
            }
        };
        // A tile holding `p`: centred a quarter-tile ahead of the tip.
        let place = |p: &[f32; 4]| -> TileRect {
            let axis =
                |lo: f32, hi: f32, size: u32, full: u32, tip: Option<f32>, dir: f32| -> u32 {
                    let max = (full - size) as f32;
                    let (min_x, max_x) = ((hi - size as f32).max(0.0), lo.min(max));
                    let want = match tip {
                        Some(t) => t - size as f32 * 0.5 + dir * size as f32 * 0.25,
                        None => (lo + hi - size as f32) * 0.5,
                    };
                    want.round().clamp(min_x, max_x.max(min_x)) as u32
                };
            TileRect {
                x: axis(p[0], p[2], tw, w, tip.map(|t| t[0]), heading[0]),
                y: axis(p[1], p[3], th, h, tip.map(|t| t[1]), heading[1]),
                w: tw,
                h: th,
            }
        };

        let mut rect = self
            .tile_rect
            .filter(|r| r.w == tw && r.h == th && r.x + tw <= w && r.y + th <= h);
        let mut main = self.main_dirty
            || (flush && !self.pending.is_empty())
            || self.pending_verts > MAX_PENDING_VERTS;
        if !main {
            // Everything that differs from what the main target shows.
            let mut c = changed_bounds(self.wet_shown(), &self.baked);
            for &(start, len) in &self.pending {
                union(&mut c, &bounds_of(&self.verts[start..start + len]));
            }
            match to_px(c) {
                None => {}
                Some(p) if rect.is_some_and(|r| r.contains(&p)) => {}
                // Nothing wet is in the main target: the tile is free to move.
                Some(p) if self.baked.is_empty() && fits(&p) => rect = Some(place(&p)),
                Some(_) => main = true,
            }
        }
        let mut pending_in_tile = !main;
        if main {
            // The main target takes all dry ink. Wet ink goes to the tile if
            // it fits; of a long stroke, the part still changing (its tail).
            pending_in_tile = false;
            let wet = self.wet_shown();
            if !wet.is_empty() {
                let all = to_px(bounds_of(wet));
                let tail = || {
                    let c = to_px(changed_bounds(wet, &self.baked)).filter(|p| fits(p));
                    c.or_else(|| {
                        let last = wet.iter().rposition(|v| v.radius >= 0.0)?;
                        let from = wet[..last]
                            .iter()
                            .rposition(|v| v.radius < 0.0)
                            .map_or(0, |i| i + 1)
                            .max(last.saturating_sub(8));
                        to_px(bounds_of(&wet[from..=last]))
                    })
                };
                rect = match all {
                    Some(p) if fits(&p) => Some(place(&p)),
                    Some(_) => tail().filter(|p| fits(p)).map(|p| place(&p)),
                    None => rect,
                };
            }
        }
        if rect.is_some() {
            let wet = if self.has_wet() { &self.wet[..] } else { &[] };
            let pending: &[(usize, usize)] = if pending_in_tile { &self.pending } else { &[] };
            if !wet.is_empty() || !pending.is_empty() {
                self.tile_verts.push(SEP);
                for (k, &(start, len)) in pending.iter().enumerate() {
                    let depth = pending_depth(k);
                    self.tile_verts.extend(
                        self.verts[start..start + len]
                            .iter()
                            .map(|v| InkVertex { depth, ..*v }),
                    );
                    self.tile_verts.push(SEP);
                }
                if wet.is_empty() {
                    self.tile_verts.push(SEP);
                } else {
                    // `[SEP, points, SEP, …, SEP]` without its leading SEP.
                    self.tile_verts.extend_from_slice(&wet[1..]);
                }
            }
        }
        let tile = self.tile_stale
            || self.tile_verts != self.tile_drawn
            || (!self.tile_verts.is_empty() && rect != self.tile_drawn_rect);
        self.planned = FramePlan { main, tile, rect };
        self.planned
    }

    /// The tile's vertex list for this frame ([`InkRenderer::render_tile`]).
    pub fn tile_vertices(&self) -> &[InkVertex] {
        &self.tile_verts
    }

    /// The camera that draws the planned tile: the main camera, moved to the
    /// tile's corner.
    pub fn tile_camera(&self, cam: Camera, dpr: f32) -> Camera {
        let r = self.planned.rect.unwrap_or(TileRect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        });
        let scale = cam.zoom * dpr;
        Camera {
            x: cam.x + r.x as f32 / scale,
            y: cam.y + r.y as f32 / scale,
            zoom: cam.zoom,
        }
    }

    /// The planned tile frame is on screen.
    pub fn tile_presented(&mut self) {
        self.tile_drawn.clone_from(&self.tile_verts);
        self.tile_drawn_rect = self.planned.rect;
        self.tile_rect = self.planned.rect;
        self.tile_stale = false;
        // With the pen up, a frame the tile could carry alone means the
        // main target shows none of the wet ink it once drew (it was all
        // under the tile): nothing is baked any more.
        if !self.planned.main && !self.has_wet() {
            self.baked.clear();
        }
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
        self.touch_all();
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

    /// Render one frame into `target` (`w`×`h` device px), all ink in this
    /// one target. `dpr` converts CSS px to device px. The target is cleared
    /// to transparent so the ink layer composites over the DOM (objects,
    /// images, page background).
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
        self.planned = FramePlan {
            main: true,
            tile: false,
            rect: None,
        };
        self.render_main(device, queue, target, w, h, cam, dpr)
    }

    /// Present the main target as planned ([`InkRenderer::plan`]): the dry
    /// layer (drawn again, or only the strokes added on top), then the wet
    /// ink that is not the tile's. With a tile, its rectangle is cut out of
    /// the wet ink so no pixel is drawn by both targets.
    #[allow(clippy::too_many_arguments)]
    pub fn render_main(
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

        let ranges: Vec<(usize, usize)> = if self.dry_dirty {
            let vis_css = cam.visible(w as f32 / dpr, h as f32 / dpr);
            self.visible_ranges(vis_css)
        } else {
            // Instances `start..start + len - 1` are a stroke's segments.
            self.appended
                .iter()
                .filter(|(_, len)| *len > 1)
                .map(|&(start, len)| (start, start + len - 1))
                .collect()
        };
        if self.dry_dirty || !ranges.is_empty() {
            Self::write_globals(queue, &self.dry_globals, cam, dpr, w, h);
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("dry ink"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &layer.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Strokes added on top are drawn over the layer as it is.
                        load: if self.dry_dirty {
                            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                        } else {
                            wgpu::LoadOp::Load
                        },
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
            stats.dry_redrawn = self.dry_dirty;
            stats.dry_appended = !self.dry_dirty;
        }

        {
            // The layer onto the frame: no depth attachment, nothing else at
            // full size.
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
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, &layer.bind_group, &[]);
            pass.draw(0..3, 0..1);
            stats.draw_calls += 1;
        }
        let mask = self
            .planned
            .rect
            .filter(|r| r.x + r.w <= w && r.y + r.h <= h);
        if self.has_wet() {
            Self::write_globals(queue, &self.wet_globals, cam, dpr, w, h);
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("wet ink"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
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
            if let Some(r) = mask {
                // The tile draws this rectangle: nearest depth, no colour.
                pass.set_pipeline(&self.mask_pipeline);
                pass.set_scissor_rect(r.x, r.y, r.w, r.h);
                pass.draw(0..3, 0..1);
                pass.set_scissor_rect(0, 0, w, h);
                stats.draw_calls += 1;
            }
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.wet_bg, &[]);
            let hi = self.wet.len() - 2;
            Self::bind_and_draw(&mut pass, &self.wet_buf, 1, hi);
            stats.segments_drawn += (hi - 1) as u32;
            stats.draw_calls += 1;
        }
        queue.submit(Some(enc.finish()));
        self.dry_dirty = false;
        self.main_dirty = false;
        self.appended.clear();
        self.pending.clear();
        self.pending_verts = 0;
        self.baked.clear();
        if self.has_wet() {
            self.baked.extend_from_slice(&self.wet);
        }
        self.tile_rect = mask;
        self.last_view = Some((cam, dpr, w, h));
        stats
    }

    /// Draw a tile frame: `verts` ([`InkRenderer::tile_vertices`] of the main
    /// renderer) through `cam` ([`InkRenderer::tile_camera`]) into `target`
    /// (`w`×`h` device px), cleared to transparent. `self` is the tile's own
    /// renderer: on WebGL2 the tile canvas has its own context and device.
    #[allow(clippy::too_many_arguments)]
    pub fn render_tile(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        w: u32,
        h: u32,
        cam: Camera,
        dpr: f32,
        verts: &[InkVertex],
    ) -> RenderStats {
        if !self
            .tile_depth
            .as_ref()
            .is_some_and(|d| d.1 == w && d.2 == h)
        {
            let depth = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("tile depth"),
                size: wgpu::Extent3d {
                    width: w.max(1),
                    height: h.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.tile_depth = Some((depth.create_view(&Default::default()), w, h));
        }
        let draws = verts.len() > 3;
        if draws {
            if verts.len() > self.wet_cap {
                self.wet_cap = verts.len().next_power_of_two();
                self.wet_buf = Self::make_vbuf(device, self.wet_cap, "wet vertices");
            }
            queue.write_buffer(&self.wet_buf, 0, bytemuck::cast_slice(verts));
            Self::write_globals(queue, &self.wet_globals, cam, dpr, w, h);
        }
        let mut stats = RenderStats::default();
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("wet tile"),
        });
        {
            let depth = &self.tile_depth.as_ref().expect("tile depth").0;
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("wet tile"),
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
                    view: depth,
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
            if draws {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.wet_bg, &[]);
                let hi = verts.len() - 2;
                Self::bind_and_draw(&mut pass, &self.wet_buf, 1, hi);
                stats.segments_drawn += (hi - 1) as u32;
                stats.draw_calls += 1;
            }
        }
        queue.submit(Some(enc.finish()));
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
            Brush::Pen,
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
    fn vertices_carry_arc_length_brush_and_width() {
        let pts: Vec<StrokePoint> = (0..4)
            .map(|i| StrokePoint {
                x: i as f32 * 3.0,
                y: 4.0 * (i % 2) as f32,
                p: 1.0,
                dt: 0,
            })
            .collect();
        let mut out = Vec::new();
        stroke_vertices(&pts, 6.5, Brush::Dashed, [0.0; 2], [0; 4], 0.5, &mut out);
        let arcs: Vec<f32> = out.iter().map(|v| v.arc).collect();
        assert_eq!(arcs, vec![0.0, 5.0, 10.0, 15.0]);
        assert_eq!(out[0].style[0], Brush::Dashed.shader_id());
        let w = u16::from(out[0].style[1]) | (u16::from(out[0].style[2]) << 8);
        assert_eq!(f32::from(w) / 64.0, 6.5);
        // Ballpoint ignores pressure, glow widens the culling bounds.
        let mut out = Vec::new();
        let soft = [StrokePoint { p: 0.0, ..pts[0] }];
        stroke_vertices(
            &soft,
            4.0,
            Brush::Ballpoint,
            [0.0; 2],
            [0; 4],
            0.5,
            &mut out,
        );
        assert_eq!(out[0].radius, 2.0);
        let mut b = empty_bounds();
        grow(
            &mut b,
            &InkVertex {
                style: [Brush::Neon.shader_id(), 0, 0, 0],
                ..out[0]
            },
        );
        assert!(b[2] - b[0] > 4.0 * 2.0);
    }

    #[test]
    fn noise_is_full_range_and_tiles() {
        let t = noise_texels();
        assert_eq!(t.len(), (NOISE_SIZE * NOISE_SIZE) as usize);
        assert_eq!(*t.iter().min().unwrap(), 0);
        assert_eq!(*t.iter().max().unwrap(), 255);
        // Opposite edges are close (no seam when it repeats).
        let n = NOISE_SIZE as usize;
        let seam: u32 = (0..n)
            .map(|y| u32::from(t[y * n].abs_diff(t[y * n + n - 1])))
            .sum();
        assert!(seam / (n as u32) < 24, "seam {}", seam / n as u32);
    }

    #[test]
    fn depth_decreases_with_order() {
        assert!(InkRenderer::depth_of(1) < InkRenderer::depth_of(0));
        assert!(InkRenderer::depth_of(MAX_DEPTH_IDX) > 0.0);
    }
}
