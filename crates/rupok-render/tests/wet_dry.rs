//! Wet ink (dense, unsimplified points) and the same stroke dry (simplified)
//! must look the same, or the stroke visibly changes at pen-up
//! (docs/performance.md), for every brush. Needs a GPU adapter; skipped
//! without one.

use rupok_core::StrokeData;
use rupok_ink::simplify::simplify;
use rupok_ink::{Brush, StrokePoint};
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

#[test]
fn dense_wet_ink_matches_the_dry_stroke() {
    let Ok(gpu) = Gpu::headless() else {
        eprintln!("no GPU adapter: skipped");
        return;
    };
    let format = wgpu::TextureFormat::Rgba8Unorm;
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
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let cam = Camera::default();
    let pts = dense_wave();

    for (brush, width) in Brush::ALL.into_iter().flat_map(|b| [(b, 3.0f32), (b, 8.0)]) {
        // Wet: every sample.
        let mut r = InkRenderer::new(&gpu.device, &gpu.queue, format);
        r.set_wet(&pts, width, brush, 0x1c2024ff, 1.0);
        r.render(&gpu.device, &gpu.queue, &view, W, H, cam, 1.0);
        let wet = read_alpha(&gpu, &target);

        // Dry: simplified as on pen-up (`StrokeBuilder::finish`).
        let mut r = InkRenderer::new(&gpu.device, &gpu.queue, format);
        let s = StrokeData {
            tool: "pen".into(),
            color: 0x1c2024ff,
            width,
            opacity: 1.0,
            points: if brush.keeps_every_point() {
                pts.clone()
            } else {
                simplify(&pts, 0.2, 0.06)
            },
            offset: [0.0, 0.0],
            shape: None,
            geom: None,
            brush: Some(brush.as_str().into()),
        };
        r.add_stroke("s", &s);
        r.render(&gpu.device, &gpu.queue, &view, W, H, cam, 1.0);
        let dry = read_alpha(&gpu, &target);

        // RUPOK_WETDRY_DUMP=dir writes both as PGM images to look at.
        if let Ok(dir) = std::env::var("RUPOK_WETDRY_DUMP") {
            for (name, img) in [("wet", &wet), ("dry", &dry)] {
                let mut f = format!("P5 {W} {H} 255\n").into_bytes();
                f.extend_from_slice(img);
                let path = format!("{dir}/{}-{width}-{name}.pgm", brush.as_str());
                std::fs::write(path, f).expect("dump");
            }
        }
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
        assert!(inked > 500, "{brush:?}: stroke did not render ({inked} px)");
        eprintln!(
            "{:<10} width {width}: {changed} of {inked} px differ",
            brush.as_str()
        );
        assert!(
            changed * 200 <= inked,
            "{brush:?} width {width}: {changed} of {inked} px differ between wet and dry"
        );
    }
}
