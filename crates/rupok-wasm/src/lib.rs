//! The rupok board engine and its JavaScript API.
//!
//! [`engine::Engine`] holds all board logic (tools, gestures, pages, undo,
//! replay) and is plain Rust. On `wasm32`, [`Board`] wraps it with a wgpu
//! surface bound to an HTML canvas and exposes a `wasm-bindgen` API that the
//! SolidJS UI (and the Tauri desktop shell, which loads the same UI) drives.

mod b64;
pub mod engine;
pub mod export;
pub mod live;
pub mod math_ink;
pub mod replay;
pub mod shapes;
pub mod templates;

#[cfg(target_arch = "wasm32")]
mod web {
    use rupok_core::{BoxGeom, Pattern};
    use rupok_render::{Gpu, InkRenderer};
    use wasm_bindgen::prelude::*;

    use crate::engine::{
        Engine, EraserMode, PointerKind, SmartInk, SmartMode, ToolKind, TouchMode,
    };

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

        /// Opacity (0.1..1) of new highlighter strokes; stored per stroke.
        pub fn set_highlighter_opacity(&mut self, a: f32) {
            self.engine.set_highlighter_opacity(a);
        }

        pub fn set_width(&mut self, w: f32) {
            self.engine.set_width(w);
        }

        /// Eraser tool and palm duster: "partial" rubs out only the ink
        /// under the eraser (strokes split), "stroke" erases whole strokes.
        pub fn set_eraser_mode(&mut self, mode: &str) {
            self.engine.set_eraser_mode(EraserMode::parse(mode));
        }

        /// Keep ink, shapes and placed items on slide pages (on by default).
        pub fn set_clip_ink(&mut self, on: bool) {
            self.engine.set_clip_ink(on);
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

        /// Smart Ink: `mode` is "off" | "auto" (snap on pen-up) | "hold"
        /// (hold the pen still to snap); `scratch` turns scratch-out to
        /// erase on while smart shapes are on. Pen only.
        pub fn set_smart_ink(&mut self, mode: &str, scratch: bool) {
            let families = self.engine.smart_ink().families;
            self.engine.set_smart_ink(SmartInk {
                mode: SmartMode::parse(mode),
                scratch,
                families,
            });
        }

        /// Smart Ink families that are on, comma-separated: `graphs`,
        /// `flowchart`, `gates`, `numberline`, `geometry`, `tables`,
        /// `science`.
        pub fn set_smart_families(&mut self, list: &str) {
            let s = self.engine.smart_ink();
            self.engine.set_smart_ink(SmartInk {
                families: rupok_ink::recognize::family::parse(list),
                ..s
            });
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

        /// Palm / hand eraser: erase the strokes under a circle of screen
        /// radius `r` swept from (x0, y0) to (x1, y1) in canvas CSS px.
        /// Strokes only; `area_erase_end` makes the rub one undo step.
        pub fn area_erase(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> u32 {
            self.engine.area_erase([x0, y0], [x1, y1], r) as u32
        }

        /// End a palm erase; returns how many strokes it erased.
        pub fn area_erase_end(&mut self) -> u32 {
            self.engine.area_erase_end() as u32
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

        /// The fit zoom: 100 % on the zoom chip. A slide zooms from fit to 5 × fit.
        pub fn fit_zoom(&self) -> f32 {
            self.engine.fit_zoom()
        }

        /// A pinch released below fit springs back instantly instead of animating.
        pub fn set_reduced_motion(&mut self, on: bool) {
            self.engine.set_reduced_motion(on);
        }

        /// Fit the page leaving `margin` (fraction of its size) around it.
        pub fn fit_page_margin(&mut self, margin: f32) {
            self.engine.fit_page_margin(margin);
        }

        /// Margin for `fit_page`, page changes and loads: 0.03 by default,
        /// 0 in Teach mode so the slide touches the screen edges.
        pub fn set_fit_margin(&mut self, margin: f32) {
            self.engine.set_fit_margin(margin);
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

        /// Copy the page at `index` (items, bundle and asset references)
        /// right after it and show the copy. Returns the copy's index.
        pub fn duplicate_page(&mut self, index: usize) -> Option<usize> {
            self.engine.duplicate_page(index)
        }

        /// Thumbnail drawing list for the page at `index`:
        /// `{frame, bg, pattern, bounds, items: [{kind: "stroke", color,
        /// width, opacity, pts: [x, y, …]} | {kind: "image", rect, asset} |
        /// {kind: "object", rect, title} | {kind: "text", rect, text, size,
        /// color}]}` in stacking order; `null` for a bad index.
        pub fn page_preview_json(&self, index: usize, max_points_per_stroke: usize) -> String {
            self.engine.page_preview_json(index, max_points_per_stroke)
        }

        /// Vector export data for the page at `index` (PDF export): items in
        /// stacking order, strokes as filled outline polygons (left out when
        /// `strokes` is false). Read-only. See
        /// [`crate::export::page_export_json`].
        pub fn page_export_json(&self, index: usize, strokes: bool) -> String {
            crate::export::page_export_json(&self.engine.lesson, &self.engine.pkg, index, strokes)
        }

        pub fn set_background(&mut self, pattern: &str, rgba: u32) {
            self.engine.set_background(Pattern::parse(pattern), rgba);
        }

        /// Add a page that looks like the current one (colour, pattern,
        /// background image, page branding), right after it or, with
        /// `at_end`, after the last page. Shows it; returns its index.
        pub fn add_page_like(&mut self, at_end: bool) -> usize {
            self.engine.add_page_like(at_end)
        }

        /// Insert a page that looks like page `src` right before (`before`)
        /// or after it, and show it. Returns its index.
        pub fn insert_page_like(&mut self, src: usize, before: bool) -> usize {
            self.engine.insert_page_like(src, before)
        }

        /// Delete several pages as one undo step (at least one page stays).
        /// Returns how many were deleted.
        pub fn delete_pages(&mut self, indices: Vec<u32>) -> usize {
            let idx: Vec<usize> = indices.into_iter().map(|i| i as usize).collect();
            self.engine.delete_pages(&idx)
        }

        /// Copy several pages, each right after its original, as one undo
        /// step. Returns the copies' indexes.
        pub fn duplicate_pages(&mut self, indices: Vec<u32>) -> Vec<u32> {
            let idx: Vec<usize> = indices.into_iter().map(|i| i as usize).collect();
            self.engine
                .duplicate_pages(&idx)
                .into_iter()
                .map(|i| i as u32)
                .collect()
        }

        /// Move several pages together to the gap before page `gap`
        /// (indexes before the move), keeping their order: one undo step.
        pub fn move_pages(&mut self, indices: Vec<u32>, gap: usize) {
            let idx: Vec<usize> = indices.into_iter().map(|i| i as usize).collect();
            self.engine.move_pages(&idx, gap);
        }

        /// Set the look of the current page, or with `all` of every page,
        /// as one undo step. `style_json`: `{pattern, bg_color, background:
        /// {image, fit, opacity, dim} | null}` (spec: Page design).
        pub fn set_page_style_json(&mut self, style_json: &str, all: bool) -> Result<(), JsValue> {
            let style: rupok_core::PageStyle =
                serde_json::from_str(style_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
            self.engine
                .set_page_style(&style, all)
                .map_err(|e| JsValue::from_str(&e))
        }

        /// Set the lesson's branding (`page_only` false) or the current
        /// page's own (`page_only` true), as one undo step. `"null"`
        /// removes it; `{"off": true}` hides branding on the page.
        pub fn set_branding_json(&mut self, json: &str, page_only: bool) -> Result<(), JsValue> {
            let b: Option<rupok_core::Branding> =
                serde_json::from_str(json).map_err(|e| JsValue::from_str(&e.to_string()))?;
            self.engine
                .set_branding(b.as_ref(), page_only)
                .map_err(|e| JsValue::from_str(&e))
        }

        /// The lesson's branding as JSON, or `null`.
        pub fn branding_json(&self) -> String {
            serde_json::to_string(&self.engine.lesson.branding()).unwrap_or_else(|_| "null".into())
        }

        /// Store an image as a package asset (deduplicated by content) and
        /// return its name, for a page background or a logo.
        pub fn add_asset(&mut self, bytes: Vec<u8>, ext: &str) -> Option<String> {
            self.engine.add_asset(bytes, ext)
        }

        /// Clear the undo stack (after giving a new lesson its default design).
        pub fn forget_history(&mut self) {
            self.engine.forget_history();
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

        /// Raise several items to the top, keeping their order (one undo step).
        pub fn bring_to_front_many(&mut self, ids: Vec<String>) {
            self.engine.bring_to_front_many(&ids);
        }

        /// Resize a text item: box and font size (one undo step).
        pub fn resize_text(&mut self, id: &str, x: f64, y: f64, w: f64, h: f64, size: f64) {
            self.engine.resize_text(
                id,
                BoxGeom {
                    x,
                    y,
                    w,
                    h,
                    rot: 0.0,
                },
                size,
            );
        }

        /// `{count, strokes, boxes, groups, grouped, shape}` for the
        /// selection toolbar; `shape` is `[x0, y0, x1, y1]` (geometric
        /// bounds, world units) when the selection is one resizable shape.
        pub fn selection_meta_json(&self) -> String {
            self.engine.selection_meta_json()
        }

        /// Resize a shape stroke so its geometric bounds become the box;
        /// the points are regenerated at the same line width. `commit:
        /// false` previews only; `true` is one undo step.
        pub fn resize_shape(
            &mut self,
            id: &str,
            x0: f64,
            y0: f64,
            x1: f64,
            y1: f64,
            commit: bool,
        ) -> bool {
            self.engine.resize_shape(id, [x0, y0, x1, y1], commit)
        }

        /// Recolour (0xRRGGBBAA) and/or re-thicken the selection: `width`
        /// sets every stroke's width, `width_scale` multiplies it. Text takes
        /// the colour. One undo step; false when nothing changed.
        pub fn restyle_selection(
            &mut self,
            color: Option<u32>,
            width: Option<f32>,
            width_scale: Option<f32>,
        ) -> bool {
            self.engine.restyle_selection(color, width, width_scale)
        }

        /// Scale the selection so its bounds become the box: ink, shapes
        /// (regenerated), text (font) and boxes. `commit: false` previews the
        /// ink only; `true` is one undo step.
        pub fn scale_selection(
            &mut self,
            x0: f64,
            y0: f64,
            x1: f64,
            y1: f64,
            commit: bool,
        ) -> bool {
            self.engine.scale_selection([x0, y0, x1, y1], commit)
        }

        /// Scale the selection about its centre (one undo step).
        pub fn scale_selection_by(&mut self, factor: f64) -> bool {
            self.engine.scale_selection_by(factor)
        }

        /// Drop a `scale_selection` preview.
        pub fn scale_cancel(&mut self) {
            self.engine.scale_cancel();
        }

        /// Resize the selected shapes and labels to the box (preview, or one
        /// undo step on `commit`).
        pub fn resize_shapes(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, commit: bool) -> bool {
            self.engine.resize_shapes([x0, y0, x1, y1], commit)
        }

        /// The handwriting around the last pen stroke (maths preview), or
        /// `null`.
        pub fn math_cluster_json(&self) -> String {
            self.engine.math_cluster_json()
        }

        /// The selected axes-and-curve graph for "Make it live", or `null`.
        pub fn graph_live_json(&self) -> String {
            self.engine.graph_live_json()
        }

        /// Replace items with a Smart Object at build step `step` with saved
        /// `state` (JSON), as one undo step. Returns the object's id.
        #[allow(clippy::too_many_arguments)]
        pub fn replace_with_object_html(
            &mut self,
            remove: Vec<String>,
            html: &str,
            title: &str,
            w: f64,
            h: f64,
            steps: u32,
            step: u32,
            state: Option<String>,
        ) -> Result<String, JsValue> {
            self.engine
                .replace_with_object_html(
                    &remove,
                    html,
                    title,
                    w,
                    h,
                    steps,
                    step,
                    state.as_deref(),
                    now(),
                )
                .map_err(err)
        }

        /// Drop a resize preview.
        pub fn resize_shape_cancel(&mut self, id: &str) {
            self.engine.resize_shape_cancel(id);
        }

        /// The selected ink in writing order for handwriting recognition:
        /// `{bounds, strokes: [{color, width, pts: [x, y, t, …]}]}`, or
        /// `null`.
        pub fn selection_ink_json(&self) -> String {
            self.engine.selection_ink_json()
        }

        /// Replace the selected ink with a text item (one undo step brings
        /// the ink back). Returns the text item's id.
        pub fn replace_selection_with_text(
            &mut self,
            text: &str,
            x: f64,
            y: f64,
            w: f64,
            size: f64,
        ) -> Option<String> {
            self.engine
                .replace_selection_with_text(text, x, y, w, size, now())
        }

        /// Replace the selected ink with typeset maths (LaTeX), one undo
        /// step. Returns the new item's id.
        pub fn replace_selection_with_latex(
            &mut self,
            latex: &str,
            x: f64,
            y: f64,
            w: f64,
            size: f64,
        ) -> Option<String> {
            self.engine
                .replace_selection_with_latex(latex, x, y, w, size, now())
        }

        /// Group the selection (2+ items). Returns the group id.
        pub fn group_selection(&mut self) -> Option<String> {
            self.engine.group_selection()
        }

        /// Dissolve the groups in the selection. Returns how many.
        pub fn ungroup_selection(&mut self) -> usize {
            self.engine.ungroup_selection()
        }

        /// Merge selected ink into one unit (moves and erases as one); a
        /// selection with box items is grouped instead. Returns
        /// `"merged"`, `"grouped"` or `"none"`.
        pub fn merge_selection(&mut self) -> String {
            self.engine.merge_selection().into()
        }

        // -------------------------------------------------------- clipboard

        /// Clipboard JSON for items on the current page. Self-contained:
        /// Smart Object bundles and image assets are embedded as base64, so
        /// it pastes into another lesson or tab. Starts with
        /// `{"rupok":"clipboard/v1"`.
        pub fn export_items_json(&self, ids: Vec<String>) -> String {
            self.engine.export_items_json(&ids)
        }

        /// Paste clipboard JSON onto the current page moved by `(dx, dy)`
        /// world units, as one undo step. The new items become the
        /// selection. Returns their ids; throws on invalid input.
        pub fn import_items_json(
            &mut self,
            json: &str,
            dx: f64,
            dy: f64,
        ) -> Result<Vec<String>, JsValue> {
            self.engine
                .import_items_json(json, dx, dy, now())
                .map_err(err)
        }

        /// Copy items in place moved by `(dx, dy)`; the copies become the
        /// selection. Returns their ids.
        pub fn duplicate_items(&mut self, ids: Vec<String>, dx: f64, dy: f64) -> Vec<String> {
            self.engine.duplicate_items(&ids, dx, dy, now())
        }

        // -------------------------------------------------------- view mode

        /// Student view: reject every document edit. Navigation, zoom,
        /// replay and opening lessons keep working.
        pub fn set_read_only(&mut self, read_only: bool) {
            self.engine.set_read_only(read_only);
        }

        pub fn read_only(&self) -> bool {
            self.engine.read_only()
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

        /// Recolour a text item, 0xRRGGBBAA (one undo step).
        pub fn set_text_color(&mut self, id: &str, rgba: u32) {
            self.engine.set_text_color(id, rgba);
        }

        pub fn set_text(&mut self, id: &str, text: &str, h: f64) {
            self.engine.set_text(id, text, (h > 0.0).then_some(h));
        }

        pub fn add_image(&mut self, bytes: Vec<u8>, ext: &str, w: f64, h: f64) -> Option<String> {
            self.engine.add_image(bytes, ext, w, h, now())
        }

        /// Place a media item (`media_json`: a `media` map, see
        /// spec/rupok-format.md "Media items") with its poster picture and,
        /// for video and audio, the clip. Returns the item id or throws why
        /// it was refused (a link off the allowlist, a file that is not
        /// media, too big).
        #[allow(clippy::too_many_arguments)]
        pub fn add_media(
            &mut self,
            media_json: &str,
            poster: Vec<u8>,
            poster_ext: &str,
            file: Option<Vec<u8>>,
            file_ext: &str,
            w: f64,
            h: f64,
        ) -> Result<String, JsValue> {
            let media = serde_json::from_str(media_json).map_err(err)?;
            self.engine
                .add_media(
                    media,
                    (poster, poster_ext),
                    file.map(|f| (f, file_ext)),
                    w,
                    h,
                    now(),
                )
                .map_err(err)
        }

        /// Change a media item's settings, and optionally its poster or
        /// clip, as one undo step.
        pub fn set_media(
            &mut self,
            id: &str,
            media_json: &str,
            poster: Option<Vec<u8>>,
            poster_ext: &str,
            file: Option<Vec<u8>>,
            file_ext: &str,
        ) -> Result<(), JsValue> {
            let media = serde_json::from_str(media_json).map_err(err)?;
            self.engine
                .set_media(
                    id,
                    media,
                    poster.map(|p| (p, poster_ext)),
                    file.map(|f| (f, file_ext)),
                )
                .map_err(err)
        }

        /// Place a 3D model (`.glb`, or `.gltf` with everything embedded) as
        /// the built-in viewer object. Returns the item id or throws.
        pub fn add_model(
            &mut self,
            bytes: &[u8],
            name: &str,
            title: &str,
            w: f64,
            h: f64,
        ) -> Result<String, JsValue> {
            self.engine
                .add_model(bytes, name, title, w, h, now())
                .map_err(err)
        }

        /// A link the board can embed, as JSON (`MediaLink`), or `"null"`
        /// when it is not on the allowlist.
        pub fn media_link_json(url: &str) -> String {
            rupok_format::media::parse_link(url)
                .and_then(|l| serde_json::to_string(&l).ok())
                .unwrap_or_else(|| "null".into())
        }

        /// The iframe `src` for a stored `media` map, rebuilt from the
        /// allowlist; `undefined` when it may not be embedded.
        pub fn media_embed_url(media_json: &str, origin: &str) -> Option<String> {
            let media = serde_json::from_str(media_json).ok()?;
            rupok_format::media::embed_url(&media, Some(origin))
        }

        /// A normalised `http(s)` link for "Open in browser", else `undefined`.
        pub fn open_url(url: &str) -> Option<String> {
            rupok_format::media::open_url(url)
        }

        /// The trusted embed sites (`Site` list) as JSON.
        pub fn media_sites_json() -> String {
            serde_json::to_string(rupok_format::media::SITES).unwrap_or_else(|_| "[]".into())
        }

        /// What kind of media file this is by content: `["video", "mp4"]`,
        /// or empty when it is not video or audio.
        pub fn sniff_media(bytes: &[u8]) -> Vec<String> {
            rupok_format::media::sniff_media(bytes)
                .map(|(k, e)| vec![k.to_string(), e.to_string()])
                .unwrap_or_default()
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

        // ------------------------------------------------- live agent bridge
        // See `crate::live` and `rupok_live`: agent ops run here as local,
        // attributed changes; Loro updates go to and come from the agent's mirror.

        /// Run an agent op (JSON). Returns the result JSON; throws the error JSON.
        pub fn live_apply(&mut self, op_json: &str, agent: &str) -> Result<String, JsValue> {
            crate::live::apply(&mut self.engine, op_json, agent, now())
                .map_err(|e| JsValue::from_str(&e))
        }

        // ------------------------------------------- AI handwriting (solve)
        // `write_handwriting` writes through `live_apply`; these set the
        // teacher's glyphs, preview without writing, and drive the reveal.

        /// The teacher's calibrated glyphs (see `crate::live::set_glyphs_json`).
        /// Returns how many characters were accepted.
        pub fn set_handwriting_glyphs(&mut self, json: &str) -> Result<u32, JsValue> {
            crate::live::set_glyphs_json(json)
                .map(|n| n as u32)
                .map_err(|e| JsValue::from_str(&e))
        }

        /// Synthesise lines (JSON array of strings) for a private preview.
        pub fn handwriting_preview_json(
            &self,
            lines_json: &str,
            x_height: f32,
            teacher: bool,
            speed: f32,
        ) -> String {
            crate::live::preview_json(lines_json, x_height, teacher, speed)
        }

        /// `[shown, total, waiting (0/1), lines released, lines]`, or empty
        /// when no handwriting is being revealed.
        pub fn reveal_state(&self) -> Vec<u32> {
            self.engine.reveal_state().map_or_else(Vec::new, |r| {
                vec![
                    r.shown as u32,
                    r.total as u32,
                    u32::from(r.waiting),
                    r.gate as u32,
                    r.lines as u32,
                ]
            })
        }

        /// Step mode: write the next line. False when nothing waits for it.
        pub fn reveal_next(&mut self) -> bool {
            self.engine.reveal_next(now())
        }

        /// Show the rest of the writing at once.
        pub fn reveal_finish(&mut self) {
            self.engine.reveal_finish();
        }

        /// The lesson's encoded Loro version vector.
        pub fn live_version(&self) -> Vec<u8> {
            crate::live::version(&self.engine)
        }

        /// Loro updates since encoded version `from` (empty: everything).
        pub fn live_export_updates(&self, from: &[u8]) -> Result<Vec<u8>, JsValue> {
            crate::live::export_updates(&self.engine, from).map_err(err)
        }

        /// Merge Loro updates from the agent's mirror.
        pub fn live_import_updates(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
            crate::live::import_updates(&mut self.engine, bytes).map_err(err)
        }

        /// `{key, title}` identifying the open lesson.
        pub fn live_doc_json(&self) -> String {
            crate::live::doc_info_json(&self.engine)
        }

        /// `{itemId: agentName}` for agent-made items on page `index`.
        pub fn live_attribution_json(&self, index: usize) -> String {
            crate::live::attribution_json(&self.engine, index)
        }

        // ------------------------------------------------------ persistence

        pub fn title(&self) -> String {
            self.engine.lesson.title()
        }

        pub fn set_title(&mut self, title: &str) {
            self.engine.set_title(title);
        }

        /// The lesson as `.rpk` bytes.
        pub fn save(&mut self) -> Result<Vec<u8>, JsValue> {
            self.engine.save(now()).map_err(err)
        }

        /// The lesson as `.rpk` bytes for an autosave: the readable summary
        /// inside is refreshed at most once a minute (explicit saves,
        /// closing and exports use `save`, which always refreshes it).
        pub fn save_autosave(&mut self) -> Result<Vec<u8>, JsValue> {
            self.engine.autosave(now()).map_err(err)
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
