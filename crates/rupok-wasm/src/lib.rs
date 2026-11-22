//! The rupok board engine and its JavaScript API.
//!
//! [`engine::Engine`] holds all board logic (tools, gestures, pages, undo,
//! replay) and is plain Rust. On `wasm32`, [`Board`] wraps it with a wgpu
//! surface bound to an HTML canvas and exposes a `wasm-bindgen` API that the
//! SolidJS UI (and the Tauri desktop shell, which loads the same UI) drives.

pub mod engine;
pub mod replay;
pub mod shapes;

#[cfg(target_arch = "wasm32")]
mod web {
    use rupok_core::{BoxGeom, Pattern};
    use rupok_render::{Gpu, InkRenderer};
    use wasm_bindgen::prelude::*;

    use crate::engine::{Engine, PointerKind, ToolKind, TouchMode};

    fn now() -> f64 {
        js_sys::Date::now()
    }

    fn err(e: impl std::fmt::Display) -> JsValue {
        JsValue::from_str(&e.to_string())
    }

    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
    }

    #[wasm_bindgen]
    pub struct Board {
        engine: Engine<InkRenderer>,
        gpu: Gpu,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        dpr: f32,
        css: [f32; 2],
        last_stats: rupok_render::RenderStats,
        frame_ms: f64,
        warned: bool,
    }

    #[wasm_bindgen]
    impl Board {
        /// Create a board on `canvas`. Prefers WebGPU, falls back to WebGL2
        /// (or forces WebGL2 when `force_webgl` is true).
        pub async fn create(
            canvas: web_sys::HtmlCanvasElement,
            force_webgl: bool,
        ) -> Result<Board, JsValue> {
            let css = [
                canvas.client_width().max(1) as f32,
                canvas.client_height().max(1) as f32,
            ];
            let (gpu, surface) = Gpu::for_canvas(canvas, force_webgl).await.map_err(err)?;
            let caps = surface.get_capabilities(&gpu.adapter);
            let format = caps
                .formats
                .iter()
                .copied()
                .find(|f| !f.is_srgb())
                .unwrap_or(caps.formats[0]);
            let alpha_mode = if caps
                .alpha_modes
                .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
            {
                wgpu::CompositeAlphaMode::PreMultiplied
            } else {
                caps.alpha_modes[0]
            };
            let mut config = surface
                .get_default_config(&gpu.adapter, css[0] as u32, css[1] as u32)
                .ok_or_else(|| err("surface not supported by adapter"))?;
            config.format = format;
            config.alpha_mode = alpha_mode;
            config.present_mode = wgpu::PresentMode::Fifo;
            config.desired_maximum_frame_latency = 1;
            surface.configure(&gpu.device, &config);
            web_sys::console::info_1(
                &format!(
                    "rupok: {} · format {:?} · alpha {:?} (supported {:?})",
                    gpu.backend_name(),
                    format,
                    alpha_mode,
                    caps.alpha_modes
                )
                .into(),
            );
            let renderer = InkRenderer::new(&gpu.device, format);
            let mut engine = Engine::new(renderer, "Untitled lesson", now());
            engine.set_view(css[0], css[1]);
            engine.fit_page();
            Ok(Board {
                engine,
                gpu,
                surface,
                config,
                dpr: 1.0,
                css,
                last_stats: Default::default(),
                frame_ms: 0.0,
                warned: false,
            })
        }

        pub fn backend(&self) -> String {
            self.gpu.backend_name()
        }

        /// Canvas size in CSS px and the device pixel ratio.
        pub fn resize(&mut self, css_w: f32, css_h: f32, dpr: f32) {
            let max = self.gpu.device.limits().max_texture_dimension_2d;
            self.css = [css_w.max(1.0), css_h.max(1.0)];
            self.dpr = dpr.max(0.5);
            self.config.width = ((self.css[0] * self.dpr).round() as u32).clamp(1, max);
            self.config.height = ((self.css[1] * self.dpr).round() as u32).clamp(1, max);
            self.surface.configure(&self.gpu.device, &self.config);
            self.engine.set_view(self.css[0], self.css[1]);
            self.engine.sink.invalidate();
        }

        /// Draw a frame if anything changed. Returns true when a frame was drawn.
        pub fn frame(&mut self) -> bool {
            let t0 = now();
            self.engine.tick(t0);
            let cam = self.engine.camera;
            let (w, h) = (self.config.width, self.config.height);
            let dirty = self.engine.take_dirty();
            if !dirty && !self.engine.sink.needs_redraw(cam, self.dpr, w, h) {
                return false;
            }
            let frame = match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(f)
                | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    self.surface.configure(&self.gpu.device, &self.config);
                    self.engine.sink.invalidate();
                    return false;
                }
                other => {
                    if !self.warned {
                        self.warned = true;
                        web_sys::console::warn_1(
                            &format!(
                                "rupok: surface not ready: {:?}",
                                std::mem::discriminant(&other)
                            )
                            .into(),
                        );
                    }
                    return false;
                }
            };
            let view = frame.texture.create_view(&Default::default());
            self.last_stats = self.engine.sink.render(
                &self.gpu.device,
                &self.gpu.queue,
                &view,
                w,
                h,
                cam,
                self.dpr,
            );
            self.gpu.queue.present(frame);
            self.frame_ms = now() - t0;
            true
        }

        /// Idle housekeeping (spatial re-sort of the stroke buffer).
        pub fn maintain(&mut self) -> bool {
            self.engine.sink.maintain()
        }

        pub fn has_animation(&self) -> bool {
            self.engine.has_animation()
        }

        // ------------------------------------------------------------ tools

        pub fn set_tool(&mut self, tool: &str) -> bool {
            match ToolKind::parse(tool) {
                Some(t) => {
                    self.engine.set_tool(t);
                    true
                }
                None => false,
            }
        }

        /// Colour as 0xRRGGBBAA.
        pub fn set_color(&mut self, rgba: u32) {
            self.engine.set_color(rgba);
        }

        pub fn set_width(&mut self, w: f32) {
            self.engine.set_width(w);
        }

        pub fn set_smoothing(&mut self, s: f32) {
            self.engine.set_smoothing(s);
        }

        pub fn set_pressure_gamma(&mut self, g: f32) {
            self.engine.set_pressure_gamma(g);
        }

        /// "auto" | "draw" | "navigate"
        pub fn set_touch_mode(&mut self, mode: &str) {
            self.engine.touch_mode = match mode {
                "draw" => TouchMode::Draw,
                "navigate" => TouchMode::Navigate,
                _ => TouchMode::Auto,
            };
        }

        // ------------------------------------------------------------ input

        #[allow(clippy::too_many_arguments)]
        pub fn pointer_down(
            &mut self,
            id: i32,
            x: f32,
            y: f32,
            pressure: f32,
            t: f64,
            kind: &str,
            button: i32,
        ) {
            self.engine.pointer_down(
                id,
                x,
                y,
                pressure,
                t,
                PointerKind::parse(kind),
                button,
                now(),
            );
        }

        /// `samples` and `predicted` are flat `[x, y, pressure, t, …]` arrays.
        pub fn pointer_move(&mut self, id: i32, samples: &[f64], predicted: &[f64]) {
            self.engine.pointer_move(id, samples, predicted, now());
        }

        pub fn pointer_up(&mut self, id: i32) {
            self.engine.pointer_up(id, now());
        }

        pub fn pointer_cancel(&mut self, id: i32) {
            self.engine.pointer_cancel(id, now());
        }

        pub fn pan(&mut self, dx: f32, dy: f32) {
            self.engine.pan_by(dx, dy);
        }

        pub fn zoom_at(&mut self, factor: f32, x: f32, y: f32) {
            self.engine.zoom_at(factor, x, y);
        }

        /// `[x, y, zoom]`
        pub fn camera(&self) -> Vec<f32> {
            let c = self.engine.camera;
            vec![c.x, c.y, c.zoom]
        }

        pub fn set_camera(&mut self, x: f32, y: f32, zoom: f32) {
            self.engine.set_camera(x, y, zoom);
        }

        pub fn fit_page(&mut self) {
            self.engine.fit_page();
        }

        /// Screen margins covered by floating UI: top, right, bottom, left (CSS px).
        pub fn set_insets(&mut self, top: f32, right: f32, bottom: f32, left: f32) {
            self.engine.set_insets(top, right, bottom, left);
        }

        // ------------------------------------------------------------ pages

        pub fn revision(&self) -> f64 {
            self.engine.revision() as f64
        }

        pub fn pages_json(&self) -> String {
            self.engine.pages_json()
        }

        pub fn page_index(&self) -> usize {
            self.engine.page_index()
        }

        pub fn goto_page(&mut self, idx: usize) -> bool {
            self.engine.goto_page(idx)
        }

        pub fn add_page(&mut self, pattern: &str) -> usize {
            self.engine.add_page(true, Pattern::parse(pattern))
        }

        pub fn delete_page(&mut self) {
            self.engine.delete_page();
        }

        pub fn move_page(&mut self, from: usize, to: usize) {
            self.engine.move_page(from, to);
        }

        pub fn set_background(&mut self, pattern: &str, rgba: u32) {
            self.engine.set_background(Pattern::parse(pattern), rgba);
        }

        pub fn clear_page(&mut self) {
            self.engine.clear_page();
        }

        // ------------------------------------------------------------ items

        pub fn dom_items_json(&self) -> String {
            self.engine.dom_items_json()
        }

        /// `[minX, minY, maxX, maxY]` in world units, or empty.
        pub fn selection_bounds(&self) -> Vec<f32> {
            self.engine
                .selection_bounds()
                .map(|b| b.to_vec())
                .unwrap_or_default()
        }

        pub fn selection_json(&self) -> String {
            serde_json::to_string(self.engine.selection()).unwrap_or_else(|_| "[]".into())
        }

        pub fn select_json(&mut self, ids_json: &str) {
            let ids: Vec<String> = serde_json::from_str(ids_json).unwrap_or_default();
            self.engine.select(ids);
        }

        pub fn select_all(&mut self) {
            self.engine.select_all();
        }

        pub fn delete_selection(&mut self) {
            self.engine.delete_selection();
        }

        pub fn delete_item(&mut self, id: &str) {
            self.engine.delete_item(id);
        }

        pub fn move_items_json(&mut self, ids_json: &str, dx: f64, dy: f64) {
            let ids: Vec<String> = serde_json::from_str(ids_json).unwrap_or_default();
            self.engine.move_items(&ids, dx, dy);
        }

        pub fn set_geom(&mut self, id: &str, x: f64, y: f64, w: f64, h: f64) {
            self.engine.set_geom(
                id,
                BoxGeom {
                    x,
                    y,
                    w,
                    h,
                    rot: 0.0,
                },
            );
        }

        pub fn bring_to_front(&mut self, id: &str) {
            self.engine.bring_to_front(id);
        }

        /// Place a Smart Object. Returns the item id; throws the validation
        /// errors as a string when the object is not allowed.
        pub fn add_object_html(
            &mut self,
            html: &str,
            title: &str,
            w: f64,
            h: f64,
            steps: u32,
        ) -> Result<String, JsValue> {
            self.engine
                .add_object_html(html, title, w, h, steps, now())
                .map_err(err)
        }

        pub fn object_html(&self, bundle: &str) -> Option<String> {
            self.engine.object_html(bundle)
        }

        pub fn set_object_step(&mut self, id: &str, step: u32) {
            self.engine.set_object_step(id, step);
        }

        pub fn set_object_state(&mut self, id: &str, state_json: &str) {
            self.engine.set_object_state(id, state_json);
        }

        pub fn add_text(&mut self, x: f32, y: f32, text: &str, size: f64) -> Option<String> {
            self.engine.add_text(x, y, text, size, now())
        }

        pub fn set_text(&mut self, id: &str, text: &str, h: f64) {
            self.engine.set_text(id, text, (h > 0.0).then_some(h));
        }

        pub fn add_image(&mut self, bytes: Vec<u8>, ext: &str, w: f64, h: f64) -> Option<String> {
            self.engine.add_image(bytes, ext, w, h, now())
        }

        pub fn asset(&self, name: &str) -> Option<Vec<u8>> {
            self.engine.asset(name).map(<[u8]>::to_vec)
        }

        pub fn undo(&mut self) -> bool {
            self.engine.undo()
        }

        pub fn redo(&mut self) -> bool {
            self.engine.redo()
        }

        pub fn can_undo(&self) -> bool {
            self.engine.lesson.can_undo()
        }

        pub fn can_redo(&self) -> bool {
            self.engine.lesson.can_redo()
        }

        // ------------------------------------------------------ persistence

        pub fn title(&self) -> String {
            self.engine.lesson.title()
        }

        pub fn set_title(&mut self, title: &str) {
            self.engine.lesson.set_title(title);
        }

        /// The lesson as `.rpk` bytes.
        pub fn save(&mut self) -> Result<Vec<u8>, JsValue> {
            self.engine.save(now()).map_err(err)
        }

        pub fn load(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
            self.engine.load(bytes).map_err(err)
        }

        pub fn new_lesson(&mut self, title: &str) {
            self.engine.new_lesson(title, now());
        }

        // ----------------------------------------------------------- replay

        pub fn replay_start(&mut self) -> f64 {
            self.engine.replay_start()
        }

        pub fn replay_seek(&mut self, t: f64) {
            self.engine.replay_seek(t, now());
        }

        pub fn replay_play(&mut self, playing: bool, speed: f64) {
            self.engine.replay_play(playing, speed, now());
        }

        /// `[t, duration, playing(0|1)]`, or empty when not replaying.
        pub fn replay_state(&self) -> Vec<f64> {
            self.engine
                .replay_state()
                .map(|(t, d, p)| vec![t, d, f64::from(u8::from(p))])
                .unwrap_or_default()
        }

        pub fn replay_stop(&mut self) {
            self.engine.replay_stop();
        }

        // ------------------------------------------------------------ stats

        pub fn stats_json(&self) -> String {
            let s = &self.last_stats;
            serde_json::json!({
                "backend": self.gpu.backend_name(),
                "strokes": s.strokes,
                "vertices": s.vertices,
                "dead": s.dead_vertices,
                "segments": s.segments_drawn,
                "drawCalls": s.draw_calls,
                "dryRedrawn": s.dry_redrawn,
                "frameMs": self.frame_ms,
                "zoom": self.engine.camera.zoom,
            })
            .to_string()
        }
    }
}
