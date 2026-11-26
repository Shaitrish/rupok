//! Wet ink (dense, unsimplified points) and the same stroke dry (simplified)
//! must look the same, or the stroke visibly changes at pen-up
//! (docs/performance.md), for every brush: from a pen's dense samples, and
//! from a mouse's sparse ones, which the stroke builder joins with a curve
//! (`rupok_ink::curve`). Needs a GPU adapter; skipped without one.

use rupok_core::StrokeData;
use rupok_ink::filter::OneEuroParams;
use rupok_ink::simplify::simplify;
use rupok_ink::{Brush, BuilderConfig, InputPoint, StrokeBuilder, StrokePoint, StrokeStyle};
use rupok_render::{Camera, Gpu, InkRenderer};

const W: u32 = 320;
const H: u32 = 160;

fn read_alpha(gpu: &Gpu, tex: &wgpu::Texture) -> Vec<u8> {
    let row = (W * 4).div_ceil(256) * 256;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(row * H),
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
                rows_per_image: Some(H),
            },
        },
        wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit(Some(enc.finish()));
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    let data = buf.slice(..).get_mapped_range().expect("mapped");
    (0..H)
        .flat_map(|y| (0..W).map(move |x| (y, x)))
        .map(|(y, x)| data[(y * row + x * 4 + 3) as usize])
        .collect()
}

/// A handwriting-like wave sampled every ~0.5 px, as a 240 Hz pen gives.
fn dense_wave() -> Vec<StrokePoint> {
    (0..520)
        .map(|i| {
            let x = 20.0 + i as f32 * 0.5;
            StrokePoint {
                x,
                y: 80.0 + (x * 0.06).sin() * 40.0,
                p: 0.5 + 0.3 * (i as f32 * 0.01).sin(),
                dt: i * 4,
            }
        })
        .collect()
}

struct Rig {
    gpu: Gpu,
    target: wgpu::Texture,
    view: wgpu::TextureView,
}

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn rig() -> Option<Rig> {
    let Ok(gpu) = Gpu::headless() else {
        eprintln!("no GPU adapter: skipped");
        return None;
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    Some(Rig { gpu, target, view })
}

impl Rig {
    /// The alpha of `points` drawn as wet ink.
    fn wet(&self, points: &[StrokePoint], width: f32, brush: Brush) -> Vec<u8> {
        let mut r = InkRenderer::new(&self.gpu.device, &self.gpu.queue, FORMAT);
        r.set_wet(points, width, brush, 0x1c2024ff, 1.0);
        let cam = Camera::default();
        r.render(
            &self.gpu.device,
            &self.gpu.queue,
            &self.view,
            W,
            H,
            cam,
            1.0,
        );
        read_alpha(&self.gpu, &self.target)
    }

    /// The alpha of `points` drawn as a stored stroke.
    fn dry(&self, points: Vec<StrokePoint>, width: f32, brush: Brush) -> Vec<u8> {
        let mut r = InkRenderer::new(&self.gpu.device, &self.gpu.queue, FORMAT);
        let s = StrokeData {
            tool: "pen".into(),
            color: 0x1c2024ff,
            width,
            opacity: 1.0,
            points,
            offset: [0.0, 0.0],
            shape: None,
            geom: None,
            brush: Some(brush.as_str().into()),
        };
        r.add_stroke("s", &s);
        let cam = Camera::default();
        r.render(
            &self.gpu.device,
            &self.gpu.queue,
            &self.view,
            W,
            H,
            cam,
            1.0,
        );
        read_alpha(&self.gpu, &self.target)
    }
}

/// Fail when more than 0.5 % of the inked pixels differ visibly. With
/// RUPOK_WETDRY_DUMP=dir, write both as PGM images to look at.
fn assert_same_ink(what: &str, brush: Brush, width: f32, wet: &[u8], dry: &[u8]) {
    if let Ok(dir) = std::env::var("RUPOK_WETDRY_DUMP") {
        for (name, img) in [("wet", wet), ("dry", dry)] {
            let mut f = format!("P5 {W} {H} 255\n").into_bytes();
            f.extend_from_slice(img);
            let path = format!("{dir}/{what}-{}-{width}-{name}.pgm", brush.as_str());
            std::fs::write(path, f).expect("dump");
        }
    }
    let inked = wet
        .iter()
        .zip(dry)
        .filter(|(a, b)| **a > 0 || **b > 0)
        .count();
    let changed = wet
        .iter()
        .zip(dry)
        .filter(|(a, b)| a.abs_diff(**b) > 64)
        .count();
    assert!(
        inked > 500,
        "{what} {brush:?}: stroke did not render ({inked} px)"
    );
    eprintln!(
        "{what:<7} {:<10} width {width}: {changed} of {inked} px differ",
        brush.as_str()
    );
    assert!(
        changed * 200 <= inked,
        "{what} {brush:?} width {width}: {changed} of {inked} px differ between wet and dry"
    );
}

#[test]
fn dense_wet_ink_matches_the_dry_stroke() {
    let Some(rig) = rig() else { return };
    let pts = dense_wave();
    for (brush, width) in Brush::ALL.into_iter().flat_map(|b| [(b, 3.0f32), (b, 8.0)]) {
        // Wet: every sample.
        let wet = rig.wet(&pts, width, brush);
        // Dry: simplified as on pen-up (`StrokeBuilder::finish`).
        let dry = rig.dry(
            if brush.keeps_every_point() {
                pts.clone()
            } else {
                simplify(&pts, 0.2, 0.06)
            },
            width,
            brush,
        );
        assert_same_ink("dense", brush, width, &wet, &dry);
    }
}

/// A fast hand on a mouse or a smartboard that reports as one: 34 samples a
/// second, about 40 px apart, around a loop and through an S-bend.
fn sparse_sample(i: i32) -> InputPoint {
    let a = i as f32 * 0.42;
    InputPoint {
        x: 40.0 + i as f32 * 9.0 + 55.0 * a.sin(),
        y: 80.0 - 52.0 * a.cos() + 10.0 * (a * 0.5).sin(),
        pressure: 0.5 + 0.3 * (i as f32 * 0.3).sin(),
        t_ms: f64::from(i) * 29.0,
    }
}

const SPARSE_SAMPLES: i32 = 22;

#[test]
fn sparse_wet_ink_is_a_curve_and_matches_the_dry_stroke() {
    let Some(rig) = rig() else { return };
    for (brush, width) in Brush::ALL.into_iter().flat_map(|b| [(b, 3.0f32), (b, 8.0)]) {
        for smoothing in [0.0, 0.25] {
            let style = StrokeStyle {
                width,
                brush,
                ..Default::default()
            };
            let cfg = BuilderConfig {
                filter: OneEuroParams::from_smoothing(smoothing),
                ..Default::default()
            };
            let mut b = StrokeBuilder::begin(style, cfg, sparse_sample(0), true);
            let mut pts = Vec::new();
            // The wet ink four samples before the end, and the last sample
            // whose ink was final then.
            let mut before = Vec::new();
            let mut settled = sparse_sample(0);
            for i in 1..SPARSE_SAMPLES {
                b.push(sparse_sample(i));
                if i == SPARSE_SAMPLES - 5 {
                    b.wet(&[], &mut before);
                    settled = sparse_sample(i - 1);
                }
            }
            // The last wet frame, and the stroke as stored at pen-up.
            b.wet(&[], &mut pts);
            let samples = b.points().len();
            assert!(
                pts.len() > 3 * samples,
                "{brush:?}: {samples} samples made only {} points",
                pts.len()
            );
            let wet = rig.wet(&pts, width, brush);
            let dry = rig.dry(b.finish(), width, brush);
            let what = if smoothing == 0.0 {
                "sparse"
            } else {
                "sparse+"
            };
            assert_same_ink(what, brush, width, &wet, &dry);

            // Ink that was final four samples ago is drawn the same now:
            // dashes and the rainbow do not creep, widths do not change.
            // (Raw samples, so the settled sample is a point of both curves.
            // The brush's taper is the exception by design: its thin end
            // travels with the pen.)
            if smoothing != 0.0 || width != 3.0 || brush == Brush::Brush {
                continue;
            }
            let cut = |c: &[StrokePoint]| {
                c.iter()
                    .position(|p| p.x == settled.x && p.y == settled.y)
                    .expect("the settled sample is on the curve")
            };
            let (k0, k1) = (cut(&before), cut(&pts));
            assert_eq!(before[..=k0], pts[..=k1], "{brush:?}: settled ink moved");
            // Pixels out of reach of everything after that sample, then and now.
            let reach = brush.reach() * width + 3.0;
            let later: Vec<&[StrokePoint]> = vec![&before[k0..], &pts[k1..]];
            let near_later = |x: f32, y: f32| {
                later
                    .iter()
                    .any(|run| run.windows(2).any(|w| seg_dist(x, y, &w[0], &w[1]) < reach))
            };
            let early = rig.wet(&before, width, brush);
            let (mut inked, mut moved) = (0, 0);
            for y in 0..H {
                for x in 0..W {
                    let k = (y * W + x) as usize;
                    if (early[k] > 0 || wet[k] > 0) && !near_later(x as f32 + 0.5, y as f32 + 0.5) {
                        inked += 1;
                        if early[k].abs_diff(wet[k]) > 24 {
                            moved += 1;
                        }
                    }
                }
            }
            assert!(inked > 300, "{brush:?}: only {inked} px of settled ink");
            assert_eq!(
                moved, 0,
                "{brush:?}: {moved} of {inked} px of settled ink changed as the stroke grew"
            );
        }
    }
}

fn seg_dist(x: f32, y: f32, a: &StrokePoint, b: &StrokePoint) -> f32 {
    let (vx, vy) = (b.x - a.x, b.y - a.y);
    let l2 = vx * vx + vy * vy;
    let t = if l2 > 0.0 {
        (((x - a.x) * vx + (y - a.y) * vy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a.x + t * vx - x).hypot(a.y + t * vy - y)
}

/// The rendered stroke follows the hand's arc, not the polygon through the
/// samples: every inked pixel lies on the true circle's band.
#[test]
fn a_sparse_arc_renders_round() {
    let Some(rig) = rig() else { return };
    let (cx, cy, r, width) = (160.0f32, 80.0f32, 62.0f32, 3.0f32);
    // Samples 40 px apart: a 37.6° turn at each, a 9-gon joined straight.
    let step = 2.0 * (20.0f32 / r).asin();
    let at = |i: i32| {
        let a = i as f32 * step;
        InputPoint {
            x: cx + r * a.cos(),
            y: cy + r * a.sin(),
            pressure: 1.0,
            t_ms: f64::from(i) * 29.0,
        }
    };
    let off_band = |img: &[u8]| -> f32 {
        let mut worst = 0.0f32;
        for y in 0..H {
            for x in 0..W {
                if img[(y * W + x) as usize] >= 128 {
                    let d = ((x as f32 + 0.5 - cx).hypot(y as f32 + 0.5 - cy) - r).abs();
                    worst = worst.max(d - width / 2.0);
                }
            }
        }
        worst
    };
    let run = |smooth: bool| {
        let cfg = BuilderConfig {
            filter: OneEuroParams::from_smoothing(0.0),
            smooth,
            ..Default::default()
        };
        let style = StrokeStyle {
            width,
            ..Default::default()
        };
        let mut b = StrokeBuilder::begin(style, cfg, at(0), false);
        for i in 1..=9 {
            b.push(at(i));
        }
        let mut pts = Vec::new();
        b.wet(&[], &mut pts);
        (rig.wet(&pts, width, Brush::Pen), b.finish())
    };
    let (straight, _) = run(false);
    let (curved, stored) = run(true);
    let dry = rig.dry(stored, width, Brush::Pen);
    let (was, wet_off, dry_off) = (off_band(&straight), off_band(&curved), off_band(&dry));
    eprintln!(
        "ink off the true arc: polygon {was:.2} px, curve wet {wet_off:.2} px, dry {dry_off:.2} px"
    );
    assert!(was > 2.0, "the polygon cuts {was} px inside the arc");
    assert!(wet_off < 0.75, "wet ink is {wet_off} px off the arc");
    assert!(dry_off < 0.75, "dry ink is {dry_off} px off the arc");
}
