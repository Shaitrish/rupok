//! The wet tile (docs/performance.md, "The wet tile"): while the pen writes,
//! changing ink is drawn into a small second target laid over the main one.
//! Composited, the two must show exactly what one target would, at every
//! step of a stroke and for every brush, and writing inside the tile must
//! not present the main target at all. Needs a GPU adapter; skipped without
//! one.

use rupok_core::StrokeData;
use rupok_ink::simplify::simplify;
use rupok_ink::{Brush, StrokePoint};
use rupok_render::{Camera, Gpu, InkRenderer};

const W: u32 = 640;
const H: u32 = 360;
const TW: u32 = 256;
const TH: u32 = 192;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn target(gpu: &Gpu, w: u32, h: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let t = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let v = t.create_view(&Default::default());
    (t, v)
}

fn read(gpu: &Gpu, tex: &wgpu::Texture, w: u32, h: u32) -> Vec<[u8; 4]> {
    let row = (w * 4).div_ceil(256) * 256;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit(Some(enc.finish()));
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    let data = buf.slice(..).get_mapped_range().expect("mapped");
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (y, x)))
        .map(|(y, x)| {
            let i = (y * row + x * 4) as usize;
            [data[i], data[i + 1], data[i + 2], data[i + 3]]
        })
        .collect()
}

fn stroke(
    points: Vec<StrokePoint>,
    brush: Brush,
    width: f32,
    color: u32,
    opacity: f32,
) -> StrokeData {
    StrokeData {
        tool: "pen".into(),
        color,
        width,
        opacity,
        points,
        offset: [0.0, 0.0],
        shape: None,
        geom: None,
        brush: Some(brush.as_str().into()),
    }
}

/// A pen path: loops for `n` samples from (x0, y0), `step` px apart in x.
fn path(x0: f32, y0: f32, n: usize, step: f32) -> Vec<StrokePoint> {
    (0..n)
        .map(|i| {
            let x = x0 + i as f32 * step;
            StrokePoint {
                x,
                y: y0 + (x * 0.07).sin() * 28.0,
                p: 0.5 + 0.3 * (i as f32 * 0.02).sin(),
                dt: i as u32 * 4,
            }
        })
        .collect()
}

/// The two targets as the browser composites them: the tile over the main
/// target at its rectangle (premultiplied source-over).
fn composite(
    main: &[[u8; 4]],
    tile: &[[u8; 4]],
    rect: Option<rupok_render::TileRect>,
) -> Vec<[u8; 4]> {
    let mut out = main.to_vec();
    let Some(r) = rect else { return out };
    for ty in 0..r.h {
        for tx in 0..r.w {
            let t = tile[(ty * r.w + tx) as usize];
            if t[3] == 0 {
                continue;
            }
            let o = &mut out[((r.y + ty) * W + r.x + tx) as usize];
            for c in 0..4 {
                let under = u32::from(o[c]) * (255 - u32::from(t[3]));
                o[c] = (u32::from(t[c]) + (under + 127) / 255).min(255) as u8;
            }
        }
    }
    out
}

/// One board drawn two ways: `split` with a wet tile (its targets keep what
/// they showed, as canvases do), and `whole`, everything in one target.
struct Rig {
    gpu: Gpu,
    split: InkRenderer,
    whole: InkRenderer,
    tile: InkRenderer,
    main_tex: (wgpu::Texture, wgpu::TextureView),
    tile_tex: (wgpu::Texture, wgpu::TextureView),
    ref_tex: (wgpu::Texture, wgpu::TextureView),
    cam: Camera,
    main_frames: u32,
    tile_frames: u32,
}

impl Rig {
    fn new(gpu: Gpu) -> Self {
        let mut split = InkRenderer::new(&gpu.device, &gpu.queue, FORMAT);
        split.set_tile_size(Some((TW, TH)));
        let tile = InkRenderer::new(&gpu.device, &gpu.queue, FORMAT);
        let whole = InkRenderer::new(&gpu.device, &gpu.queue, FORMAT);
        let main_tex = target(&gpu, W, H);
        let tile_tex = target(&gpu, TW, TH);
        let ref_tex = target(&gpu, W, H);
        Self {
            gpu,
            split,
            whole,
            tile,
            main_tex,
            tile_tex,
            ref_tex,
            cam: Camera::default(),
            main_frames: 0,
            tile_frames: 0,
        }
    }

    fn add(&mut self, id: &str, s: StrokeData) {
        self.split.add_stroke(id, &s);
        self.whole.add_stroke(id, &s);
    }

    fn remove(&mut self, id: &str) {
        self.split.remove_stroke(id);
        self.whole.remove_stroke(id);
    }

    fn set_wet(&mut self, wet: Option<(&[StrokePoint], f32, Brush, u32, f32)>) {
        for r in [&mut self.split, &mut self.whole] {
            match wet {
                Some((p, w, b, c, o)) => r.set_wet(p, w, b, c, o),
                None => r.clear_wet(),
            }
        }
    }

    /// Draw a frame both ways and compare. Returns (main presented, tile presented).
    fn frame(&mut self, flush: bool, what: &str) -> (bool, bool) {
        let (d, q) = (&self.gpu.device, &self.gpu.queue);
        let plan = self.split.plan(self.cam, 1.0, W, H, flush);
        if plan.main {
            self.split
                .render_main(d, q, &self.main_tex.1, W, H, self.cam, 1.0);
            self.main_frames += 1;
        }
        if plan.tile {
            let cam = self.split.tile_camera(self.cam, 1.0);
            self.tile.render_tile(
                d,
                q,
                &self.tile_tex.1,
                TW,
                TH,
                cam,
                1.0,
                self.split.tile_vertices(),
            );
            self.split.tile_presented();
            self.tile_frames += 1;
        }
        self.whole
            .render(d, q, &self.ref_tex.1, W, H, self.cam, 1.0);

        let main = read(&self.gpu, &self.main_tex.0, W, H);
        let tile = read(&self.gpu, &self.tile_tex.0, TW, TH);
        let want = read(&self.gpu, &self.ref_tex.0, W, H);
        let got = composite(&main, &tile, self.split.tile_rect());
        let inked = want.iter().filter(|p| p[3] > 0).count();
        // Blending in two steps rounds twice: a channel may be one off.
        let off = want
            .iter()
            .zip(&got)
            .filter(|(a, b)| (0..4).any(|c| a[c].abs_diff(b[c]) > 2))
            .count();
        if off > 0
            && let Ok(dir) = std::env::var("RUPOK_WETDRY_DUMP")
        {
            for (name, img) in [("want", &want), ("got", &got)] {
                let mut f = format!("P6 {W} {H} 255\n").into_bytes();
                f.extend(
                    img.iter()
                        .flat_map(|p| [255 - p[3], 255 - p[3], 255 - p[3]]),
                );
                std::fs::write(format!("{dir}/tile-{name}.ppm"), f).expect("dump");
            }
        }
        assert_eq!(
            off,
            0,
            "{what}: {off} of {inked} ink px differ between the tile path and one target \
             (plan {plan:?}, tile at {:?})",
            self.split.tile_rect()
        );
        (plan.main, plan.tile)
    }
}

#[test]
fn the_wet_tile_shows_what_one_target_would() {
    // The plain pen, a see-through marker (blending), a taper measured from
    // the stroke's end, a pattern along the path and a glow past the radius.
    for brush in [
        Brush::Pen,
        Brush::Marker,
        Brush::Brush,
        Brush::Dashed,
        Brush::Neon,
    ] {
        let Ok(gpu) = Gpu::headless() else {
            eprintln!("no GPU adapter: skipped");
            return;
        };
        let name = brush.as_str();
        let (opacity, color) = if brush == Brush::Marker {
            (0.5, 0x3e63ddff)
        } else {
            (1.0, 0x1c2024ff)
        };
        let width = 7.0;
        let mut rig = Rig::new(gpu);
        // Ink already on the page, some of it where the new strokes go.
        rig.add(
            "a",
            stroke(path(30.0, 90.0, 150, 3.0), Brush::Pen, 5.0, 0xe5484dff, 1.0),
        );
        rig.add(
            "b",
            stroke(
                path(60.0, 250.0, 120, 4.0),
                Brush::Marker,
                14.0,
                0xffe629ff,
                0.5,
            ),
        );
        // The first frame also clears the tile.
        assert_eq!(rig.frame(false, name), (true, true), "{name}: first frame");
        assert_eq!(
            rig.frame(false, name),
            (false, false),
            "{name}: idle draws nothing"
        );

        // A short stroke (a letter): the tile alone, never the main target.
        let letter = path(100.0, 120.0, 60, 1.5);
        let before = rig.main_frames;
        for n in (4..=letter.len()).step_by(14) {
            rig.set_wet(Some((&letter[..n], width, brush, color, opacity)));
            let (main, tile) = rig.frame(false, &format!("{name}: letter, {n} samples"));
            assert!(tile, "{name}: wet ink is the tile's");
            assert!(
                !main,
                "{name}: writing inside the tile presented the main target"
            );
        }
        // Pen-up: the dry stroke replaces the wet one, still in the tile.
        let dry = if brush.keeps_every_point() {
            letter.clone()
        } else {
            simplify(&letter, 0.2, 0.06)
        };
        rig.set_wet(None);
        rig.add("c", stroke(dry, brush, width, color, opacity));
        rig.frame(false, &format!("{name}: pen-up"));
        // The next letter, beside it.
        let second = path(150.0, 140.0, 50, 1.2);
        for n in (5..=second.len()).step_by(15) {
            rig.set_wet(Some((&second[..n], width, brush, color, opacity)));
            rig.frame(false, &format!("{name}: second letter, {n} samples"));
        }
        assert_eq!(
            rig.main_frames, before,
            "{name}: two letters, no main frame"
        );
        // The pen rests: everything goes to the main target, the tile empties.
        rig.set_wet(None);
        rig.add("d", stroke(second.clone(), brush, width, color, opacity));
        assert_eq!(
            rig.frame(true, &format!("{name}: flush")),
            (true, true),
            "{name}: flush"
        );
        assert_eq!(rig.frame(true, name), (false, false), "{name}: flushed");

        // A long stroke leaves the tile: the main target takes the part left
        // behind, a few times over the whole line, not every frame.
        let line = path(20.0, 200.0, 290, 2.1);
        let (before, mut frames) = (rig.main_frames, 0);
        for n in (3..=line.len()).step_by(11) {
            rig.set_wet(Some((&line[..n], width, brush, color, opacity)));
            rig.frame(false, &format!("{name}: long stroke, {n} samples"));
            frames += 1;
        }
        let rebakes = rig.main_frames - before;
        assert!(
            (1..=frames / 5).contains(&rebakes),
            "{name}: {rebakes} main frames in {frames} of a long stroke"
        );
        rig.set_wet(None);
        rig.add("e", stroke(line, brush, width, color, opacity));
        rig.frame(false, &format!("{name}: long stroke, pen-up"));

        // Erasing and moving the view are the main target's.
        rig.remove("a");
        assert!(rig.frame(false, &format!("{name}: erase")).0);
        rig.cam = Camera {
            x: -40.0,
            y: 12.5,
            zoom: 1.25,
        };
        assert!(rig.frame(false, &format!("{name}: pan and zoom")).0);
        // Wet ink too big for any tile is drawn in the main target, as before.
        let lasso = path(10.0, 60.0, 300, 2.0);
        rig.set_wet(Some((&lasso, 40.0, brush, color, opacity)));
        assert!(rig.frame(false, &format!("{name}: wide wet ink")).0);
        rig.set_wet(None);
        rig.frame(false, &format!("{name}: cleared"));
    }
}

/// Strokes added on top are drawn over the cached dry layer; the result is
/// the layer drawn from scratch.
#[test]
fn appended_strokes_match_a_full_redraw() {
    let Ok(gpu) = Gpu::headless() else {
        eprintln!("no GPU adapter: skipped");
        return;
    };
    let (d, q) = (&gpu.device, &gpu.queue);
    let a = target(&gpu, W, H);
    let b = target(&gpu, W, H);
    let cam = Camera::default();
    let strokes: Vec<StrokeData> = Brush::ALL
        .into_iter()
        .enumerate()
        .map(|(i, brush)| {
            let o = if brush == Brush::Marker { 0.5 } else { 1.0 };
            stroke(
                path(20.0 + i as f32 * 9.0, 60.0 + i as f32 * 21.0, 110, 4.0),
                brush,
                9.0,
                0x3e63ddff,
                o,
            )
        })
        .collect();
    let mut inc = InkRenderer::new(d, q, FORMAT);
    for (i, s) in strokes.iter().enumerate() {
        inc.add_stroke(&format!("s{i}"), s);
        let stats = inc.render(d, q, &a.1, W, H, cam, 1.0);
        assert_eq!(
            stats.dry_redrawn,
            i == 0,
            "stroke {i}: only the first draws the whole layer"
        );
        assert_eq!(stats.dry_appended, i > 0);
        let mut full = InkRenderer::new(d, q, FORMAT);
        for (k, s) in strokes[..=i].iter().enumerate() {
            full.add_stroke(&format!("s{k}"), s);
        }
        full.render(d, q, &b.1, W, H, cam, 1.0);
        let (x, y) = (read(&gpu, &a.0, W, H), read(&gpu, &b.0, W, H));
        let off = x.iter().zip(&y).filter(|(p, r)| p != r).count();
        assert_eq!(off, 0, "stroke {i}: {off} px differ from a full redraw");
    }
}
