//! GPU ink renderer for rupok.
//!
//! One wgpu code path runs in the webview (WebGPU, or WebGL2 where WebGPU is
//! missing) and natively (headless export, thumbnails, benchmarks).
//!
//! Performance model:
//! - all dry strokes live in one vertex buffer, drawn with a single instanced
//!   call per visible chunk range (frustum culling by chunk bounds);
//! - erasing never rebuilds the buffer: the stroke's points are tombstoned in
//!   place and the buffer is compacted lazily;
//! - dry ink is rendered into a cached layer that is reused while only the wet
//!   stroke changes, so pen latency is independent of how much ink exists.

mod ink;

pub use ink::{InkRenderer, RenderStats};

/// World → screen mapping. `screen_css = (world - (x, y)) * zoom`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

impl Camera {
    pub fn screen_to_world(&self, sx: f32, sy: f32) -> (f32, f32) {
        (self.x + sx / self.zoom, self.y + sy / self.zoom)
    }
    pub fn world_to_screen(&self, wx: f32, wy: f32) -> (f32, f32) {
        ((wx - self.x) * self.zoom, (wy - self.y) * self.zoom)
    }
    /// Zoom by `factor` keeping the screen point (sx, sy) fixed.
    pub fn zoom_at(&mut self, factor: f32, sx: f32, sy: f32, min: f32, max: f32) {
        let (wx, wy) = self.screen_to_world(sx, sy);
        self.zoom = (self.zoom * factor).clamp(min, max);
        self.x = wx - sx / self.zoom;
        self.y = wy - sy / self.zoom;
    }
    /// Visible world rectangle for a viewport of `w`×`h` CSS px.
    pub fn visible(&self, w: f32, h: f32) -> [f32; 4] {
        [
            self.x,
            self.y,
            self.x + w / self.zoom,
            self.y + h / self.zoom,
        ]
    }
}

/// Owns the wgpu device. Created from a canvas (web) or headless (native).
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    fn device_desc(adapter: &wgpu::Adapter) -> wgpu::DeviceDescriptor<'static> {
        wgpu::DeviceDescriptor {
            label: Some("rupok"),
            required_features: wgpu::Features::empty(),
            // WebGL2-compatible limits so the same renderer runs everywhere.
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::Performance,
            ..Default::default()
        }
    }

    pub fn backend_name(&self) -> String {
        let info = self.adapter.get_info();
        format!("{:?} · {}", info.backend, info.name)
    }

    /// Headless device for exports and benchmarks.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn headless() -> Result<Self, String> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU adapter: {e}"))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Self::device_desc(&adapter)))
                .map_err(|e| e.to_string())?;
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }

    /// Device + surface for an HTML canvas. Prefers WebGPU, falls back to WebGL2.
    #[cfg(target_arch = "wasm32")]
    pub async fn for_canvas(
        canvas: wgpu::web_sys::HtmlCanvasElement,
        force_webgl: bool,
    ) -> Result<(Self, wgpu::Surface<'static>), String> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = if force_webgl {
            wgpu::Backends::GL
        } else {
            wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL
        };
        let instance = wgpu::util::new_instance_with_webgpu_detection(desc).await;
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| format!("create_surface: {e}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("no GPU adapter: {e}"))?;
        let (device, queue) = adapter
            .request_device(&Self::device_desc(&adapter))
            .await
            .map_err(|e| e.to_string())?;
        Ok((
            Self {
                instance,
                adapter,
                device,
                queue,
            },
            surface,
        ))
    }
}

/// Pack 0xRRGGBBAA and an opacity into the Unorm8x4 vertex colour (RGBA bytes).
#[inline]
pub fn pack_color(rgba: u32, opacity: f32) -> [u8; 4] {
    let a = ((rgba & 0xff) as f32 * opacity.clamp(0.0, 1.0)).round() as u8;
    [(rgba >> 24) as u8, (rgba >> 16) as u8, (rgba >> 8) as u8, a]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_zoom_keeps_anchor() {
        let mut c = Camera {
            x: 100.0,
            y: 50.0,
            zoom: 1.0,
        };
        let before = c.screen_to_world(300.0, 200.0);
        c.zoom_at(2.5, 300.0, 200.0, 0.05, 40.0);
        let after = c.screen_to_world(300.0, 200.0);
        assert!((before.0 - after.0).abs() < 1e-3 && (before.1 - after.1).abs() < 1e-3);
        assert_eq!(c.zoom, 2.5);
    }

    #[test]
    fn colors_pack_as_rgba_bytes() {
        assert_eq!(pack_color(0x11223380, 0.5), [0x11, 0x22, 0x33, 0x40]);
    }
}
