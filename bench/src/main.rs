//! `cargo run -p rupok-bench --release [-- --strokes 100000 --check]`
//!
//! Simulates a long class (default: 100k strokes ≈ several hours of constant
//! writing) and reports the budgets from the plan:
//! - document: insert, snapshot size, save/load time, package write/read;
//! - GPU: dry-layer frame time zoomed-in/zoomed-out, wet-only frame time.
//!
//! `--check` exits non-zero if a budget is missed (for CI).

use std::time::{Duration, Instant};

use rupok_core::{ItemData, Lesson, PageSpec, StrokeData};
use rupok_format::Package;
use rupok_ink::{Brush, BuilderConfig, InputPoint, StrokeBuilder, StrokeStyle};
use rupok_render::{Camera, Gpu, InkRenderer};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        ((self.0.wrapping_mul(0x2545F4914F6CDD1D) >> 40) as f32) / (1u64 << 24) as f32
    }
}

/// Handwriting-like stroke: a wobbly curve built from raw samples through the
/// real input pipeline (filter → pressure → simplify), like a pen would.
fn synth_stroke(rng: &mut Rng, cx: f32, cy: f32, t0: f64) -> StrokeData {
    let len = 20 + (rng.next() * 60.0) as usize;
    let mut x = cx;
    let mut y = cy;
    let mut a = rng.next() * std::f32::consts::TAU;
    let first = InputPoint {
        x,
        y,
        pressure: 0.4,
        t_ms: t0,
    };
    let mut b = StrokeBuilder::begin(
        StrokeStyle::default(),
        BuilderConfig::default(),
        first,
        true,
    );
    for i in 1..len {
        a += (rng.next() - 0.5) * 0.9;
        x += a.cos() * 2.5;
        y += a.sin() * 2.5;
        let p = 0.3 + 0.6 * (i as f32 / len as f32 * std::f32::consts::PI).sin();
        b.push(InputPoint {
            x,
            y,
            pressure: p,
            t_ms: t0 + i as f64 * 6.0,
        });
    }
    let color = [0x1d2433ff, 0xd63b3bff, 0x2563ebff, 0x16a34aff][(rng.next() * 4.0) as usize % 4];
    StrokeData {
        tool: "pen".into(),
        color,
        width: 2.5 + rng.next() * 2.0,
        opacity: 1.0,
        points: b.finish(),
        offset: [0.0, 0.0],
        shape: None,
        geom: None,
        brush: None,
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn percentile(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() as f64 - 1.0) * p).round() as usize]
}

struct Budget {
    name: &'static str,
    value: f64,
    limit: f64,
    unit: &'static str,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse::<usize>().ok())
    };
    let n = arg("--strokes").unwrap_or(100_000);
    let check = args.iter().any(|a| a == "--check");
    let mut budgets: Vec<Budget> = Vec::new();

    println!("rupok bench — {n} strokes\n");

    // ---------------------------------------------------------------- input
    let mut rng = Rng(0x0037_4e07);
    let t = Instant::now();
    // Strokes spread over a 16:9 slide grid of 40 pages' worth of area, like a
    // long class that keeps writing on an infinite board.
    let strokes: Vec<StrokeData> = (0..n)
        .map(|i| {
            let page = i % 40;
            let ox = (page % 8) as f32 * 2000.0;
            let oy = (page / 8) as f32 * 1200.0;
            let (cx, cy) = (ox + rng.next() * 1900.0, oy + rng.next() * 1050.0);
            synth_stroke(&mut rng, cx, cy, i as f64 * 400.0)
        })
        .collect();
    let pts: usize = strokes.iter().map(|s| s.points.len()).sum();
    println!(
        "input pipeline      {:>9.1} ms   ({} stored points, {:.1}/stroke)",
        ms(t.elapsed()),
        pts,
        pts as f64 / n as f64
    );

    curve_report(&mut budgets);

    // ------------------------------------------------------------- document
    let mut lesson = Lesson::new("Bench", 0.0);
    let page = lesson.page_id_at(0).unwrap();
    lesson
        .set_page_background(&page, rupok_core::Pattern::Grid, 0xffffffff)
        .unwrap();
    let _ = lesson.add_page(None, PageSpec::default());
    let t = Instant::now();
    let mut per_insert = Vec::with_capacity(n);
    for (i, s) in strokes.iter().enumerate() {
        let ti = Instant::now();
        lesson
            .add_item(&page, &ItemData::Stroke(s.clone()), i as f64)
            .unwrap();
        per_insert.push(ms(ti.elapsed()));
    }
    let insert_total = t.elapsed();
    let p99_insert = percentile(&mut per_insert, 0.99);
    println!(
        "doc insert          {:>9.1} ms   (p99 {:.3} ms per stroke commit)",
        ms(insert_total),
        p99_insert
    );
    budgets.push(Budget {
        name: "stroke commit p99",
        value: p99_insert,
        limit: 5.0,
        unit: "ms",
    });

    let t = Instant::now();
    let snap = lesson.save();
    let save_ms = ms(t.elapsed());
    println!(
        "doc snapshot        {:>9.1} ms   ({:.1} MB, {:.1} B/stroke)",
        save_ms,
        snap.len() as f64 / 1e6,
        snap.len() as f64 / n as f64
    );

    let t = Instant::now();
    let loaded = Lesson::load(&snap).unwrap();
    let load_ms = ms(t.elapsed());
    println!("doc load            {:>9.1} ms", load_ms);

    let t = Instant::now();
    let items = loaded.items(&page).unwrap();
    let read_ms = ms(t.elapsed());
    assert_eq!(items.len(), n);
    let t = Instant::now();
    let _dv = loaded.loro().get_deep_value();
    println!("  (raw loro deep value {:.1} ms)", ms(t.elapsed()));
    let t = Instant::now();
    let _ = loaded.items(&page).unwrap();
    println!(
        "doc read items      {:>9.1} ms   (warm re-read {:.1} ms)",
        read_ms,
        ms(t.elapsed())
    );
    budgets.push(Budget {
        name: "open lesson (load + read)",
        value: load_ms + read_ms,
        limit: 1000.0,
        unit: "ms",
    });

    let t = Instant::now();
    let pkg = Package::from_lesson(&lesson, "bench", 0.0);
    let bytes = pkg.write().unwrap();
    let wr = ms(t.elapsed());
    let t = Instant::now();
    let back = Package::read(&bytes).unwrap();
    let rd = ms(t.elapsed());
    assert_eq!(back.doc.len(), snap.len());
    println!(
        "package write/read  {:>9.1} / {:.1} ms   ({:.1} MB .rpk)",
        wr,
        rd,
        bytes.len() as f64 / 1e6
    );

    let t = Instant::now();
    let scene = rupok_core::Scene::from_items(items.clone());
    let scene_ms = ms(t.elapsed());
    let t = Instant::now();
    let mut hits = 0;
    for i in 0..1000 {
        let x = (i % 40) as f32 * 400.0;
        hits += scene.erase_hits([x, 500.0], [x + 30.0, 520.0], 8.0).len();
    }
    println!(
        "scene build         {:>9.1} ms   (1000 eraser queries {:.2} ms, {hits} hits)",
        scene_ms,
        ms(t.elapsed())
    );

    // ------------------------------------------------------------------ GPU
    let gpu = match Gpu::headless() {
        Ok(g) => g,
        Err(e) => {
            println!("\nGPU: unavailable ({e}) — skipping render budgets");
            report(&budgets, check);
            return;
        }
    };
    println!("\nGPU: {}", gpu.backend_name());
    let (w, h) = (1920u32, 1080u32);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench target"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let mut r = InkRenderer::new(&gpu.device, &gpu.queue, format);

    // Correctness: a single thick red stroke must produce red pixels.
    let probe = StrokeData {
        tool: "pen".into(),
        color: 0xff0000ff,
        width: 20.0,
        opacity: 1.0,
        points: vec![
            rupok_ink::StrokePoint {
                x: 100.0,
                y: 100.0,
                p: 1.0,
                dt: 0,
            },
            rupok_ink::StrokePoint {
                x: 300.0,
                y: 100.0,
                p: 1.0,
                dt: 10,
            },
        ],
        offset: [0.0, 0.0],
        shape: None,
        geom: None,
        brush: None,
    };
    r.add_stroke("probe", &probe);
    r.render(&gpu.device, &gpu.queue, &view, w, h, Camera::default(), 1.0);
    let px = read_pixel(&gpu, &target, 200, 100);
    let px_out = read_pixel(&gpu, &target, 200, 140);
    println!(
        "probe pixel         rgba{:?} inside, rgba{:?} outside",
        px, px_out
    );
    assert!(px[0] > 240 && px[3] > 240, "stroke did not render: {px:?}");
    assert!(
        px_out[3] == 0,
        "pixels outside the stroke are not transparent: {px_out:?}"
    );
    r.remove_stroke("probe");

    let t = Instant::now();
    for it in &items {
        if let ItemData::Stroke(s) = &it.data {
            r.add_stroke(&it.id, s);
        }
    }
    let add_ms = ms(t.elapsed());
    let t = Instant::now();
    r.compact();
    println!(
        "renderer load       {:>9.1} ms   (+ spatial sort {:.1} ms)",
        add_ms,
        ms(t.elapsed())
    );

    let time_frames = |r: &mut InkRenderer,
                       cam_fn: &dyn Fn(usize) -> Camera,
                       frames: usize,
                       wet: Option<Brush>|
     -> (f64, f64, u32) {
        let mut times = Vec::with_capacity(frames);
        let mut segs = 0;
        for f in 0..frames {
            if let Some(brush) = wet {
                let pts: Vec<_> = (0..(50 + f))
                    .map(|i| rupok_ink::StrokePoint {
                        x: 300.0 + i as f32 * 2.0,
                        y: 300.0 + (i as f32 * 0.2).sin() * 30.0,
                        p: 0.6,
                        dt: i as u32,
                    })
                    .collect();
                r.set_wet(&pts, 3.0, brush, 0x2563ebff, 1.0);
            }
            let t = Instant::now();
            let st = r.render(&gpu.device, &gpu.queue, &view, w, h, cam_fn(f), 1.0);
            gpu.device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            times.push(ms(t.elapsed()));
            segs = st.segments_drawn;
        }
        r.clear_wet();
        let p50 = percentile(&mut times.clone(), 0.5);
        let p99 = percentile(&mut times, 0.99);
        (p50, p99, segs)
    };

    // Warm-up (pipeline compile, uploads).
    time_frames(&mut r, &|_| Camera::default(), 5, None);

    let (p50, p99, segs) = time_frames(
        &mut r,
        &|f| Camera {
            x: f as f32 * 3.0,
            y: 0.0,
            zoom: 1.0,
        },
        120,
        None,
    );
    println!(
        "pan @100% zoom      p50 {:>6.2} ms  p99 {:>6.2} ms   ({segs} segments/frame)",
        p50, p99
    );
    budgets.push(Budget {
        name: "pan frame p99 @100%",
        value: p99,
        limit: 8.3,
        unit: "ms",
    });

    let (p50, p99, segs) = time_frames(
        &mut r,
        &|f| Camera {
            x: -100.0 + f as f32 * 5.0,
            y: -100.0,
            zoom: 0.11,
        },
        120,
        None,
    );
    println!(
        "pan, whole board    p50 {:>6.2} ms  p99 {:>6.2} ms   ({segs} segments/frame, everything visible)",
        p50, p99
    );

    let still = Camera {
        x: 0.0,
        y: 0.0,
        zoom: 1.0,
    };
    time_frames(&mut r, &|_| still, 2, None);
    let (p50, p99, segs) = time_frames(&mut r, &|_| still, 240, Some(Brush::Pen));
    println!(
        "writing (wet only)  p50 {:>6.2} ms  p99 {:>6.2} ms   ({segs} segments/frame, dry layer cached)",
        p50, p99
    );
    budgets.push(Budget {
        name: "wet frame p99 (pen latency)",
        value: p99,
        limit: 4.0,
        unit: "ms",
    });
    // Every brush on the same path (pen styles, docs/performance.md).
    let mut worst = (0.0f64, Brush::Pen);
    for brush in Brush::ALL {
        let (p50, p99, _) = time_frames(&mut r, &|_| still, 120, Some(brush));
        println!(
            "  wet {:<10} p50 {:>6.2} ms  p99 {:>6.2} ms",
            brush.as_str(),
            p50,
            p99
        );
        if p99 > worst.0 {
            worst = (p99, brush);
        }
    }
    println!(
        "writing, worst brush ({}) p99 {:.2} ms",
        worst.1.as_str(),
        worst.0
    );
    budgets.push(Budget {
        name: "wet frame p99, worst brush",
        value: worst.0,
        limit: 4.0,
        unit: "ms",
    });

    // The pen path through the engine: a 10 s stroke at 240 Hz (2400
    // samples), each pointer move rebuilding the wet stroke, then frames.
    let (move_p99, wet_frame_p99) = pen_path(&gpu, &view, w, h, format, Brush::Pen);
    println!(
        "pen path, 10 s stroke  move p99 {move_p99:>6.3} ms   wet frame at the end p99 {wet_frame_p99:>6.2} ms"
    );
    budgets.push(Budget {
        name: "pen move → wet update p99",
        value: move_p99,
        limit: 0.5,
        unit: "ms",
    });
    let mut worst_move = (0.0f64, Brush::Pen);
    for brush in Brush::ALL {
        let (m, f) = pen_path(&gpu, &view, w, h, format, brush);
        println!(
            "  pen path {:<10} move p99 {m:>6.3} ms   wet frame p99 {f:>6.2} ms",
            brush.as_str()
        );
        if m > worst_move.0 {
            worst_move = (m, brush);
        }
    }
    budgets.push(Budget {
        name: "pen move → wet update p99, worst brush",
        value: worst_move.0,
        limit: 0.5,
        unit: "ms",
    });

    // Wet and dry must look the same, or the stroke jumps at pen-up: for
    // every brush.
    let changed = wet_dry_diff(&gpu, &target, &view, w, h, format, Brush::Pen);
    println!("wet vs dry          {changed:>6.2} % of ink pixels differ");
    budgets.push(Budget {
        name: "wet vs dry pixels differ",
        value: changed,
        limit: 0.5,
        unit: "%",
    });
    let mut worst_diff = (0.0f64, Brush::Pen);
    for brush in Brush::ALL {
        let c = wet_dry_diff(&gpu, &target, &view, w, h, format, brush);
        println!("  wet vs dry {:<10} {c:>6.2} %", brush.as_str());
        if c > worst_diff.0 {
            worst_diff = (c, brush);
        }
    }
    budgets.push(Budget {
        name: "wet vs dry pixels differ, worst brush",
        value: worst_diff.0,
        limit: 0.5,
        unit: "%",
    });

    report(&budgets, check);
}

/// What the curve through sparse samples costs (`rupok_ink::curve`): the
/// same 13 600 px of loops written by a mouse at 34 Hz (a fast hand, 40 px
/// between samples), a touch screen and a pen at 240 Hz, each built with
/// the curve and with straight joins.
fn curve_report(budgets: &mut Vec<Budget>) {
    use rupok_ink::filter::OneEuroParams;
    const LENGTH: f32 = 13_600.0;
    // (ms between samples, px between samples)
    let run = |dt: f64, step: f32, smooth: bool| {
        let r = 115.0f32;
        let n = (LENGTH / step) as usize;
        let at = |i: usize| {
            let s = i as f32 * step;
            let a = s / r;
            InputPoint {
                // Loops that drift, so the line never retraces itself.
                x: 400.0 + s * 0.05 + r * a.cos(),
                y: 400.0 + r * (a * 1.03).sin(),
                pressure: 0.5,
                t_ms: i as f64 * dt,
            }
        };
        let cfg = BuilderConfig {
            filter: OneEuroParams::from_smoothing(0.0),
            smooth,
            ..Default::default()
        };
        let mut b = StrokeBuilder::begin(StrokeStyle::default(), cfg, at(0), false);
        let mut wet = Vec::new();
        let mut times = Vec::with_capacity(n);
        for i in 1..n {
            let t = Instant::now();
            b.push(at(i));
            b.wet(&[], &mut wet);
            times.push(ms(t.elapsed()));
        }
        let samples = b.points().len();
        let wet_points = wet.len();
        let t = Instant::now();
        let stored = b.finish().len();
        let finish = ms(t.elapsed());
        (
            samples,
            wet_points,
            stored,
            percentile(&mut times, 0.99),
            finish,
        )
    };
    println!("curve through the samples, one 13 600 px line (straight joins in brackets)");
    let mut per_span = 0.0;
    let mut dense_extra = 0.0;
    let mut sparse_stored = 0.0;
    let mut pen_stored = 1.0;
    for (name, dt, step) in [
        ("34 Hz mouse, 40 px apart", 1000.0 / 34.0, 40.0f32),
        ("60 Hz touch, 15 px apart", 1000.0 / 60.0, 15.0),
        ("240 Hz pen, 1.5 px apart", 1000.0 / 240.0, 1.5),
    ] {
        let (samples, wet, stored, p99, finish) = run(dt, step, true);
        let (_, wet0, stored0, p990, finish0) = run(dt, step, false);
        println!(
            "  {name}: {samples} samples → {wet} wet points [{wet0}], {stored} stored [{stored0}]; \
             push + wet p99 {p99:.3} ms [{p990:.3}], pen-up {finish:.2} ms [{finish0:.2}]"
        );
        if step == 40.0 {
            per_span = (wet - 1) as f64 / (samples - 1) as f64;
            sparse_stored = stored as f64;
        }
        if step == 1.5 {
            dense_extra = (wet as f64 / wet0 as f64 - 1.0) * 100.0;
            pen_stored = stored as f64;
        }
    }
    budgets.push(Budget {
        name: "curve pieces per 40 px span (34 Hz mouse)",
        value: per_span,
        limit: 8.0,
        unit: "",
    });
    budgets.push(Budget {
        name: "curve: extra wet points, 240 Hz pen",
        value: dense_extra,
        limit: 5.0,
        unit: "%",
    });
    // A mouse stroke is stored no more densely than a pen leaves the line.
    budgets.push(Budget {
        name: "curve: mouse stroke stored vs the pen's",
        value: sparse_stored / pen_stored * 100.0,
        limit: 110.0,
        unit: "%",
    });
}

/// A 10 s, 240 Hz pen stroke through the engine (rupok-wasm's `Engine`, the
/// board's own pointer path). Returns (pointer_move p99, wet frame p99) in ms.
fn pen_path(
    gpu: &Gpu,
    view: &wgpu::TextureView,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
    brush: Brush,
) -> (f64, f64) {
    use rupok_wasm::engine::{Engine, PointerKind};
    let mut e = Engine::new(
        InkRenderer::new(&gpu.device, &gpu.queue, format),
        "bench",
        0.0,
    );
    e.set_view(w as f32, h as f32);
    e.set_brush(brush);
    let at = |i: usize| {
        let t = i as f64 * (1000.0 / 240.0);
        let x = 100.0 + (i as f64 * 0.7) % 1600.0;
        let y = 500.0 + (t * 0.02).sin() * 200.0;
        [x, y, 0.6, t]
    };
    let s0 = at(0);
    e.pointer_down(
        1,
        s0[0] as f32,
        s0[1] as f32,
        0.6,
        s0[3],
        PointerKind::Pen,
        0,
        0.0,
    );
    let mut moves = Vec::with_capacity(2400);
    let mut frames = Vec::new();
    for i in 1..2400 {
        let s = at(i);
        let t = Instant::now();
        e.pointer_move(1, &s, &[], s[3]);
        moves.push(ms(t.elapsed()));
        if i > 2400 - 120 {
            let t = Instant::now();
            e.sink
                .render(&gpu.device, &gpu.queue, view, w, h, e.camera, 1.0);
            gpu.device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            frames.push(ms(t.elapsed()));
        }
    }
    e.pointer_up(1, 10_000.0);
    (percentile(&mut moves, 0.99), percentile(&mut frames, 0.99))
}

/// Render a dense wet stroke, then the same stroke dry (simplified, as on
/// pen-up), and return the % of ink pixels whose coverage differs.
fn wet_dry_diff(
    gpu: &Gpu,
    target: &wgpu::Texture,
    view: &wgpu::TextureView,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
    brush: Brush,
) -> f64 {
    let pts: Vec<_> = (0..1200)
        .map(|i| {
            let x = 200.0 + i as f32 * 0.5;
            rupok_ink::StrokePoint {
                x,
                y: 300.0 + (x * 0.05).sin() * 60.0,
                p: 0.5 + 0.3 * (i as f32 * 0.01).sin(),
                dt: i * 4,
            }
        })
        .collect();
    let cam = Camera::default();
    let region = (180u32, 220u32, 640u32, 160u32);
    let mut r = InkRenderer::new(&gpu.device, &gpu.queue, format);
    r.set_wet(&pts, 4.0, brush, 0x1c2024ff, 1.0);
    r.render(&gpu.device, &gpu.queue, view, w, h, cam, 1.0);
    let wet = read_region(gpu, target, region);
    let mut r = InkRenderer::new(&gpu.device, &gpu.queue, format);
    r.add_stroke(
        "dry",
        &StrokeData {
            tool: "pen".into(),
            color: 0x1c2024ff,
            width: 4.0,
            opacity: 1.0,
            // As `StrokeBuilder::finish` does.
            points: if brush.keeps_every_point() {
                pts.clone()
            } else {
                rupok_ink::simplify::simplify(&pts, 0.2, 0.06)
            },
            offset: [0.0, 0.0],
            shape: None,
            geom: None,
            brush: Some(brush.as_str().into()),
        },
    );
    r.render(&gpu.device, &gpu.queue, view, w, h, cam, 1.0);
    let dry = read_region(gpu, target, region);
    let inked = wet
        .iter()
        .zip(&dry)
        .filter(|(a, b)| **a > 0 || **b > 0)
        .count();
    let changed = wet
        .iter()
        .zip(&dry)
        .filter(|(a, b)| a.abs_diff(**b) > 64)
        .count();
    100.0 * changed as f64 / inked.max(1) as f64
}

/// Alpha of a `(x, y, w, h)` region of an Rgba8 texture.
fn read_region(gpu: &Gpu, tex: &wgpu::Texture, (x, y, w, h): (u32, u32, u32, u32)) -> Vec<u8> {
    let row = (w * 4).div_ceil(256) * 256;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback region"),
        size: u64::from(row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
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
        .flat_map(|yy| (0..w).map(move |xx| (yy * row + xx * 4 + 3) as usize))
        .map(|i| data[i])
        .collect()
}

fn read_pixel(gpu: &Gpu, tex: &wgpu::Texture, x: u32, y: u32) -> [u8; 4] {
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(1),
            },
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit(Some(enc.finish()));
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    let data = buf.slice(..).get_mapped_range().expect("mapped");
    [data[0], data[1], data[2], data[3]]
}

fn report(budgets: &[Budget], check: bool) {
    println!("\nbudgets");
    let mut failed = false;
    for b in budgets {
        let ok = b.value <= b.limit;
        failed |= !ok;
        println!(
            "  {} {:<30} {:>8.2} {} (limit {} {})",
            if ok { "PASS" } else { "FAIL" },
            b.name,
            b.value,
            b.unit,
            b.limit,
            b.unit
        );
    }
    if check && failed {
        std::process::exit(1);
    }
}
