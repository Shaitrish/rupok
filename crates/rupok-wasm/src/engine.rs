//! The board engine: tools, gestures, selection, pages, undo and replay.
//!
//! Pure Rust with no GPU or browser dependency so it is unit-testable natively.
//! Visual output goes through [`InkSink`] (the GPU renderer in the app, a mock
//! in tests); DOM-rendered items (objects, text, images) are exposed as JSON
//! for the UI layer, which re-reads them whenever [`Engine::revision`] changes.

use std::collections::{HashMap, HashSet};

use rupok_core::{
    BoxGeom, Frame, ImageData, Item, ItemData, ItemId, Lesson, ObjectData, PageId, PageSpec,
    Pattern, Scene, StrokeData, TextData,
};
use rupok_format::{ObjectBundle, Package};
use rupok_ink::filter::OneEuroParams;
use rupok_ink::pressure::PressureCurve;
use rupok_ink::{BuilderConfig, InputPoint, StrokeBuilder, StrokePoint, StrokeStyle, Tool};
use rupok_render::Camera;
use serde::Serialize;

use crate::replay::Replay;
use crate::shapes::{ShapeKind, shape_points};

pub const GENERATOR: &str = concat!("rupok/", env!("CARGO_PKG_VERSION"));
const MIN_ZOOM: f32 = 0.05;
const MAX_ZOOM: f32 = 40.0;
const LASER_LIFETIME_MS: f64 = 1400.0;

/// Where visual ink goes. Implemented by the GPU renderer.
pub trait InkSink {
    fn add_stroke(&mut self, id: &str, s: &StrokeData);
    fn remove_stroke(&mut self, id: &str);
    fn clear(&mut self);
    /// Replace all transient strokes (wet ink, previews, laser, lasso).
    fn set_wet(&mut self, strokes: &[WetStroke]);
    /// Called after a bulk load; lets the renderer re-sort/compact.
    fn finish_load(&mut self);
}

#[derive(Clone, Debug, PartialEq)]
pub struct WetStroke {
    pub points: Vec<StrokePoint>,
    pub width: f32,
    pub color: u32,
    pub opacity: f32,
}

impl InkSink for rupok_render::InkRenderer {
    fn add_stroke(&mut self, id: &str, s: &StrokeData) {
        rupok_render::InkRenderer::add_stroke(self, id, s);
    }
    fn remove_stroke(&mut self, id: &str) {
        rupok_render::InkRenderer::remove_stroke(self, id);
    }
    fn clear(&mut self) {
        rupok_render::InkRenderer::clear(self);
    }
    fn set_wet(&mut self, strokes: &[WetStroke]) {
        if strokes.is_empty() {
            self.clear_wet();
        } else {
            self.set_wet_many(
                strokes
                    .iter()
                    .map(|w| (w.points.as_slice(), w.width, w.color, w.opacity)),
            );
        }
    }
    fn finish_load(&mut self) {
        self.compact();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Pen,
    Highlighter,
    Eraser,
    Lasso,
    Hand,
    Laser,
    Shape(ShapeKind),
}

impl ToolKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "pen" => ToolKind::Pen,
            "highlighter" => ToolKind::Highlighter,
            "eraser" => ToolKind::Eraser,
            "lasso" | "select" => ToolKind::Lasso,
            "hand" => ToolKind::Hand,
            "laser" => ToolKind::Laser,
            "rect" => ToolKind::Shape(ShapeKind::Rect),
            "ellipse" => ToolKind::Shape(ShapeKind::Ellipse),
            "line" => ToolKind::Shape(ShapeKind::Line),
            "arrow" => ToolKind::Shape(ShapeKind::Arrow),
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerKind {
    Pen,
    Touch,
    Mouse,
}

impl PointerKind {
    pub fn parse(s: &str) -> Self {
        match s {
            "pen" => PointerKind::Pen,
            "touch" => PointerKind::Touch,
            _ => PointerKind::Mouse,
        }
    }
}

/// How finger touches behave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchMode {
    /// Fingers draw until a pen is seen; then fingers only navigate (palm rejection).
    Auto,
    Draw,
    Navigate,
}

// The Ink variant is large, but gestures are few (one per active pointer)
// and short-lived, so boxing would only add an allocation on pen-down.
#[allow(clippy::large_enum_variant)]
enum Gesture {
    Ink {
        builder: StrokeBuilder,
        predicted: Vec<StrokePoint>,
    },
    Erase {
        last: [f32; 2],
        hits: Vec<ItemId>,
    },
    Lasso {
        poly: Vec<[f32; 2]>,
        start_screen: [f32; 2],
        moved: bool,
    },
    MoveSelection {
        last: [f32; 2],
        total: [f32; 2],
    },
    Shape {
        kind: ShapeKind,
        start: [f32; 2],
        end: [f32; 2],
    },
    Pan {
        last: [f32; 2],
    },
    Laser {
        points: Vec<(StrokePoint, f64)>,
    },
}

struct LaserTrail {
    points: Vec<(StrokePoint, f64)>,
}

#[derive(Clone, Copy)]
struct Pinch {
    centroid: [f32; 2],
    dist: f32,
}

#[derive(Serialize)]
pub struct DomItem<'a> {
    pub id: &'a str,
    pub kind: &'static str,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub rot: f64,
    pub z: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<&'a str>,
    pub selected: bool,
}

pub struct Engine<S: InkSink> {
    pub sink: S,
    pub pkg: Package,
    pub lesson: Lesson,
    page: PageId,
    scene: Scene,
    pub camera: Camera,
    view: [f32; 2],
    /// Screen area covered by floating UI chrome: [top, right, bottom, left].
    insets: [f32; 4],
    tool: ToolKind,
    pen: StrokeStyle,
    highlighter: StrokeStyle,
    shape_style: StrokeStyle,
    eraser_radius: f32,
    smoothing: f32,
    pressure_gamma: f32,
    pub touch_mode: TouchMode,
    pen_seen: bool,
    gestures: HashMap<i32, Gesture>,
    touches: HashMap<i32, [f32; 2]>,
    pinch: Option<Pinch>,
    selection: Vec<ItemId>,
    drag_offset: [f32; 2],
    lasers: Vec<LaserTrail>,
    replay: Option<Replay>,
    revision: u64,
    dirty: bool,
}

fn css_color(rgba: u32) -> String {
    format!("#{:08x}", rgba)
}

impl<S: InkSink> Engine<S> {
    pub fn new(sink: S, title: &str, now_ms: f64) -> Self {
        let lesson = Lesson::new(title, now_ms);
        let pkg = Package::from_lesson(&lesson, GENERATOR, now_ms);
        let page = lesson.page_id_at(0).expect("new lesson has a page");
        let mut e = Self {
            sink,
            pkg,
            lesson,
            page,
            scene: Scene::new(),
            camera: Camera::default(),
            view: [1280.0, 720.0],
            insets: [0.0; 4],
            tool: ToolKind::Pen,
            pen: StrokeStyle {
                tool: Tool::Pen,
                color: 0x1d2433ff,
                width: 3.0,
                opacity: 1.0,
            },
            highlighter: StrokeStyle {
                tool: Tool::Highlighter,
                color: 0xffd60aff,
                width: 22.0,
                opacity: 0.38,
            },
            shape_style: StrokeStyle {
                tool: Tool::Pen,
                color: 0x1d2433ff,
                width: 3.0,
                opacity: 1.0,
            },
            eraser_radius: 14.0,
            smoothing: 0.5,
            pressure_gamma: 0.8,
            touch_mode: TouchMode::Auto,
            pen_seen: false,
            gestures: HashMap::new(),
            touches: HashMap::new(),
            pinch: None,
            selection: Vec::new(),
            drag_offset: [0.0, 0.0],
            lasers: Vec::new(),
            replay: None,
            revision: 1,
            dirty: true,
        };
        e.load_page();
        e
    }

    // ---------------------------------------------------------------- state

    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn bump(&mut self) {
        self.revision += 1;
        self.dirty = true;
    }

    /// True when transient visuals changed and a frame should be drawn.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub fn has_animation(&self) -> bool {
        !self.lasers.is_empty() || self.replay.as_ref().is_some_and(|r| r.playing)
    }

    pub fn set_view(&mut self, w: f32, h: f32) {
        self.view = [w.max(1.0), h.max(1.0)];
        self.dirty = true;
    }

    /// Tell the engine which screen margins are covered by UI chrome, so
    /// "fit page" and new-item placement use the visible area.
    pub fn set_insets(&mut self, top: f32, right: f32, bottom: f32, left: f32) {
        self.insets = [top.max(0.0), right.max(0.0), bottom.max(0.0), left.max(0.0)];
    }

    /// Centre and size of the visible (unobstructed) area in screen px.
    fn visible_area(&self) -> ([f32; 2], [f32; 2]) {
        let [t, r, b, l] = self.insets;
        let w = (self.view[0] - l - r).max(120.0);
        let h = (self.view[1] - t - b).max(120.0);
        ([l + w / 2.0, t + h / 2.0], [w, h])
    }

    pub fn tool(&self) -> ToolKind {
        self.tool
    }

    pub fn set_tool(&mut self, t: ToolKind) {
        self.tool = t;
        if !matches!(t, ToolKind::Lasso) && !self.selection.is_empty() {
            self.selection.clear();
            self.bump();
        }
    }

    fn style_mut(&mut self) -> &mut StrokeStyle {
        match self.tool {
            ToolKind::Highlighter => &mut self.highlighter,
            ToolKind::Shape(_) => &mut self.shape_style,
            _ => &mut self.pen,
        }
    }

    pub fn set_color(&mut self, rgba: u32) {
        let keep_alpha = self.tool == ToolKind::Highlighter;
        let s = self.style_mut();
        s.color = if keep_alpha {
            (rgba & 0xffffff00) | 0xff
        } else {
            rgba
        };
        if matches!(self.tool, ToolKind::Pen) {
            // Shapes follow the pen colour unless styled separately.
            self.shape_style.color = rgba;
        }
    }

    pub fn set_width(&mut self, w: f32) {
        if self.tool == ToolKind::Eraser {
            self.eraser_radius = (w * 0.5).clamp(2.0, 200.0);
        } else {
            self.style_mut().width = w.clamp(0.5, 200.0);
        }
    }

    pub fn set_smoothing(&mut self, s: f32) {
        self.smoothing = s.clamp(0.0, 1.0);
    }

    pub fn set_pressure_gamma(&mut self, g: f32) {
        self.pressure_gamma = g.clamp(0.2, 3.0);
    }

    pub fn page_id(&self) -> &str {
        &self.page
    }

    pub fn page_index(&self) -> usize {
        self.lesson
            .pages()
            .iter()
            .position(|p| p.id == self.page)
            .unwrap_or(0)
    }

    pub fn selection(&self) -> &[ItemId] {
        &self.selection
    }

    // ---------------------------------------------------------------- pages

    fn load_page(&mut self) {
        let items = self.lesson.items(&self.page).unwrap_or_default();
        self.sink.clear();
        for it in &items {
            if let ItemData::Stroke(s) = &it.data {
                self.sink.add_stroke(&it.id, s);
            }
        }
        self.sink.finish_load();
        self.scene = Scene::from_items(items);
        self.selection.clear();
        self.gestures.clear();
        self.sink.set_wet(&[]);
        self.bump();
    }

    /// Bring scene + renderer in line with the document (after undo/redo/import).
    pub fn resync(&mut self) {
        if self.lesson.page(&self.page).is_err() {
            self.page = self.lesson.page_id_at(0).unwrap_or_default();
            self.load_page();
            return;
        }
        let items = self.lesson.items(&self.page).unwrap_or_default();
        let ids: HashSet<&str> = items.iter().map(|i| i.id.as_str()).collect();
        let stale: Vec<ItemId> = self
            .scene
            .ordered()
            .iter()
            .filter(|i| !ids.contains(i.id.as_str()))
            .map(|i| i.id.clone())
            .collect();
        for id in stale {
            self.scene.remove(&id);
            self.sink.remove_stroke(&id);
        }
        for it in items {
            if self.scene.get(&it.id) == Some(&it) {
                continue;
            }
            if let ItemData::Stroke(s) = &it.data {
                self.sink.add_stroke(&it.id, s);
            }
            self.scene.insert(it);
        }
        self.selection.retain(|id| self.scene.get(id).is_some());
        self.bump();
    }

    pub fn goto_page(&mut self, idx: usize) -> bool {
        let Ok(id) = self.lesson.page_id_at(idx) else {
            return false;
        };
        self.replay = None;
        self.page = id;
        self.load_page();
        self.fit_page();
        true
    }

    pub fn add_page(&mut self, after_current: bool, pattern: Pattern) -> usize {
        let idx = if after_current {
            Some(self.page_index() + 1)
        } else {
            None
        };
        let spec = PageSpec {
            pattern,
            ..Default::default()
        };
        let id = self.lesson.add_page(idx, spec).expect("add page");
        let i = self
            .lesson
            .pages()
            .iter()
            .position(|p| p.id == id)
            .unwrap_or(0);
        self.goto_page(i);
        i
    }

    pub fn delete_page(&mut self) {
        if self.lesson.page_count() <= 1 {
            return;
        }
        let idx = self.page_index();
        let _ = self.lesson.remove_page(&self.page.clone());
        self.goto_page(idx.min(self.lesson.page_count() - 1));
    }

    pub fn move_page(&mut self, from: usize, to: usize) {
        if self.lesson.move_page(from, to).is_ok() {
            self.bump();
        }
    }

    pub fn set_background(&mut self, pattern: Pattern, color: u32) {
        let _ = self
            .lesson
            .set_page_background(&self.page.clone(), pattern, color);
        self.bump();
    }

    pub fn fit_page(&mut self) {
        let frame = self
            .lesson
            .page(&self.page)
            .ok()
            .and_then(|p| p.frame)
            .unwrap_or(Frame::SLIDE_16_9);
        let (fw, fh) = (frame.w as f32, frame.h as f32);
        let (c, size) = self.visible_area();
        let zoom = (size[0] / (fw * 1.03))
            .min(size[1] / (fh * 1.03))
            .clamp(MIN_ZOOM, MAX_ZOOM);
        self.camera = Camera {
            x: fw / 2.0 - c[0] / zoom,
            y: fh / 2.0 - c[1] / zoom,
            zoom,
        };
        self.dirty = true;
    }

    // ----------------------------------------------------------- input

    fn world(&self, x: f32, y: f32) -> [f32; 2] {
        let (wx, wy) = self.camera.screen_to_world(x, y);
        [wx, wy]
    }

    fn builder_cfg(&self) -> BuilderConfig {
        BuilderConfig {
            curve: PressureCurve {
                gamma: self.pressure_gamma,
                ..Default::default()
            },
            filter: OneEuroParams::from_smoothing(self.smoothing),
            zoom: self.camera.zoom,
        }
    }

    fn touch_draws(&self) -> bool {
        match self.touch_mode {
            TouchMode::Draw => true,
            TouchMode::Navigate => false,
            TouchMode::Auto => !self.pen_seen,
        }
    }

    fn inside_selection(&self, w: [f32; 2]) -> bool {
        let pad = 8.0 / self.camera.zoom;
        self.scene.bounds_of(&self.selection).is_some_and(|b| {
            w[0] >= b[0] - pad && w[0] <= b[2] + pad && w[1] >= b[1] - pad && w[1] <= b[3] + pad
        })
    }

    /// Pointer down in canvas CSS px. `t` is the event timestamp (ms).
    #[allow(clippy::too_many_arguments)]
    pub fn pointer_down(
        &mut self,
        id: i32,
        x: f32,
        y: f32,
        pressure: f32,
        t: f64,
        kind: PointerKind,
        button: i32,
        now_ms: f64,
    ) {
        if self.replay.is_some() {
            return;
        }
        if kind == PointerKind::Pen {
            self.pen_seen = true;
        }
        // Middle/right button always pans.
        if button == 1 || button == 2 {
            self.gestures.insert(id, Gesture::Pan { last: [x, y] });
            return;
        }
        if kind == PointerKind::Touch
            && (!self.touch_draws() || !self.touches.is_empty() || self.tool == ToolKind::Hand)
        {
            self.touches.insert(id, [x, y]);
            // A second finger cancels any stroke a finger started: it is a pinch.
            let finger_ids: Vec<i32> = self.touches.keys().copied().collect();
            for fid in finger_ids {
                if let Some(g) = self.gestures.remove(&fid)
                    && let Gesture::Erase { hits, .. } = g
                {
                    self.commit_erase(hits);
                }
            }
            self.update_wet(now_ms);
            self.pinch = None;
            return;
        }
        let w = self.world(x, y);
        let g = match self.tool {
            ToolKind::Pen | ToolKind::Highlighter => {
                let style = if self.tool == ToolKind::Pen {
                    self.pen
                } else {
                    self.highlighter
                };
                let first = InputPoint {
                    x: w[0],
                    y: w[1],
                    pressure,
                    t_ms: t,
                };
                Gesture::Ink {
                    builder: StrokeBuilder::begin(
                        style,
                        self.builder_cfg(),
                        first,
                        kind == PointerKind::Pen,
                    ),
                    predicted: Vec::new(),
                }
            }
            ToolKind::Eraser => {
                let r = self.eraser_radius / self.camera.zoom;
                let hits = self.erase_at(w, w, r);
                Gesture::Erase { last: w, hits }
            }
            ToolKind::Lasso => {
                if self.inside_selection(w) {
                    Gesture::MoveSelection {
                        last: w,
                        total: [0.0, 0.0],
                    }
                } else {
                    Gesture::Lasso {
                        poly: vec![w],
                        start_screen: [x, y],
                        moved: false,
                    }
                }
            }
            ToolKind::Hand => Gesture::Pan { last: [x, y] },
            ToolKind::Laser => Gesture::Laser {
                points: vec![(
                    StrokePoint {
                        x: w[0],
                        y: w[1],
                        p: 1.0,
                        dt: 0,
                    },
                    now_ms,
                )],
            },
            ToolKind::Shape(kind) => Gesture::Shape {
                kind,
                start: w,
                end: w,
            },
        };
        self.gestures.insert(id, g);
        self.update_wet(now_ms);
    }

    /// Coalesced samples as flat `[x, y, pressure, t, ...]`; `predicted` likewise.
    pub fn pointer_move(&mut self, id: i32, samples: &[f64], predicted: &[f64], now_ms: f64) {
        if let Some(prev) = self.touches.get(&id).copied() {
            if let Some(last) = samples.as_chunks::<4>().0.iter().last() {
                let p = [last[0] as f32, last[1] as f32];
                self.touches.insert(id, p);
                self.navigate_touches(prev, id);
            }
            return;
        }
        let zoom = self.camera.zoom;
        let cam = self.camera;
        let Some(g) = self.gestures.get_mut(&id) else {
            return;
        };
        let to_world = |x: f64, y: f64| {
            let (wx, wy) = cam.screen_to_world(x as f32, y as f32);
            [wx, wy]
        };
        let mut erase_segments: Vec<([f32; 2], [f32; 2])> = Vec::new();
        let mut pan: Option<[f32; 2]> = None;
        match g {
            Gesture::Ink {
                builder,
                predicted: pred,
            } => {
                for s in samples.as_chunks::<4>().0 {
                    let w = to_world(s[0], s[1]);
                    builder.push(InputPoint {
                        x: w[0],
                        y: w[1],
                        pressure: s[2] as f32,
                        t_ms: s[3],
                    });
                }
                pred.clear();
                let last_p = builder.points().last().map(|p| p.p).unwrap_or(0.5);
                for s in predicted.as_chunks::<4>().0 {
                    let w = to_world(s[0], s[1]);
                    pred.push(StrokePoint {
                        x: w[0],
                        y: w[1],
                        p: last_p,
                        dt: 0,
                    });
                }
            }
            Gesture::Erase { last, .. } => {
                for s in samples.as_chunks::<4>().0 {
                    let w = to_world(s[0], s[1]);
                    erase_segments.push((*last, w));
                    *last = w;
                }
            }
            Gesture::Lasso {
                poly,
                start_screen,
                moved,
            } => {
                for s in samples.as_chunks::<4>().0 {
                    let w = to_world(s[0], s[1]);
                    if (s[0] as f32 - start_screen[0]).abs() + (s[1] as f32 - start_screen[1]).abs()
                        > 6.0
                    {
                        *moved = true;
                    }
                    let far = poly.last().is_none_or(|l| {
                        ((l[0] - w[0]).powi(2) + (l[1] - w[1]).powi(2)).sqrt() > 2.0 / zoom
                    });
                    if far {
                        poly.push(w);
                    }
                }
            }
            Gesture::MoveSelection { last, total } => {
                if let Some(s) = samples.as_chunks::<4>().0.iter().last() {
                    let w = to_world(s[0], s[1]);
                    total[0] += w[0] - last[0];
                    total[1] += w[1] - last[1];
                    *last = w;
                }
            }
            Gesture::Shape { end, .. } => {
                if let Some(s) = samples.as_chunks::<4>().0.iter().last() {
                    *end = to_world(s[0], s[1]);
                }
            }
            Gesture::Pan { last } => {
                if let Some(s) = samples.as_chunks::<4>().0.iter().last() {
                    let p = [s[0] as f32, s[1] as f32];
                    pan = Some([p[0] - last[0], p[1] - last[1]]);
                    *last = p;
                }
            }
            Gesture::Laser { points } => {
                for s in samples.as_chunks::<4>().0 {
                    let w = to_world(s[0], s[1]);
                    points.push((
                        StrokePoint {
                            x: w[0],
                            y: w[1],
                            p: 1.0,
                            dt: 0,
                        },
                        now_ms,
                    ));
                }
            }
        }
        if let Some(d) = pan {
            self.pan_by(d[0], d[1]);
        }
        if !erase_segments.is_empty() {
            let r = self.eraser_radius / zoom;
            let mut new_hits = Vec::new();
            for (a, b) in erase_segments {
                new_hits.extend(self.erase_at(a, b, r));
            }
            if let Some(Gesture::Erase { hits, .. }) = self.gestures.get_mut(&id) {
                hits.extend(new_hits);
            }
        }
        if let Some(Gesture::MoveSelection { total, .. }) = self.gestures.get(&id) {
            let t = *total;
            self.preview_move(t);
        }
        self.update_wet(now_ms);
    }

    pub fn pointer_up(&mut self, id: i32, now_ms: f64) {
        if self.touches.remove(&id).is_some() {
            self.pinch = None;
            return;
        }
        let Some(g) = self.gestures.remove(&id) else {
            return;
        };
        match g {
            Gesture::Ink { builder, .. } => {
                let style = builder.style;
                let pts = builder.finish();
                if !pts.is_empty() {
                    self.commit_stroke(
                        StrokeData {
                            tool: style.tool.as_str().into(),
                            color: style.color,
                            width: style.width,
                            opacity: style.opacity,
                            points: pts,
                            offset: [0.0, 0.0],
                            shape: None,
                        },
                        now_ms,
                    );
                }
            }
            Gesture::Erase { hits, .. } => self.commit_erase(hits),
            Gesture::Lasso {
                poly,
                start_screen,
                moved,
            } => {
                if moved && poly.len() >= 3 {
                    self.selection = self.scene.lasso(&poly);
                } else {
                    let w = self.world(start_screen[0], start_screen[1]);
                    self.selection = self
                        .scene
                        .hit_test(w[0], w[1], 6.0 / self.camera.zoom)
                        .map(|i| vec![i.id.clone()])
                        .unwrap_or_default();
                }
                self.bump();
            }
            Gesture::MoveSelection { total, .. } => {
                self.drag_offset = [0.0, 0.0];
                if total != [0.0, 0.0] {
                    let sel = self.selection.clone();
                    let _ = self.lesson.translate_items(
                        &self.page.clone(),
                        &sel,
                        f64::from(total[0]),
                        f64::from(total[1]),
                    );
                    self.resync();
                } else {
                    self.bump();
                }
            }
            Gesture::Shape { kind, start, end } => {
                let d = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();
                if d * self.camera.zoom > 3.0 {
                    let st = self.shape_style;
                    self.commit_stroke(
                        StrokeData {
                            tool: "pen".into(),
                            color: st.color,
                            width: st.width,
                            opacity: st.opacity,
                            points: shape_points(kind, start, end, st.width),
                            offset: [0.0, 0.0],
                            shape: Some(kind.as_str().into()),
                        },
                        now_ms,
                    );
                }
            }
            Gesture::Pan { .. } => {}
            Gesture::Laser { points } => self.lasers.push(LaserTrail { points }),
        }
        self.update_wet(now_ms);
    }

    pub fn pointer_cancel(&mut self, id: i32, now_ms: f64) {
        self.touches.remove(&id);
        self.pinch = None;
        if let Some(Gesture::Erase { hits, .. }) = self.gestures.remove(&id) {
            self.commit_erase(hits);
        }
        self.preview_move([0.0, 0.0]);
        self.update_wet(now_ms);
    }

    fn navigate_touches(&mut self, prev: [f32; 2], moved_id: i32) {
        if self.touches.len() == 1 {
            let now = self.touches[&moved_id];
            self.pan_by(now[0] - prev[0], now[1] - prev[1]);
            return;
        }
        let pts: Vec<[f32; 2]> = self.touches.values().take(2).copied().collect();
        let c = [(pts[0][0] + pts[1][0]) / 2.0, (pts[0][1] + pts[1][1]) / 2.0];
        let d = ((pts[0][0] - pts[1][0]).powi(2) + (pts[0][1] - pts[1][1]).powi(2))
            .sqrt()
            .max(1.0);
        if let Some(p) = self.pinch {
            self.pan_by(c[0] - p.centroid[0], c[1] - p.centroid[1]);
            self.camera
                .zoom_at(d / p.dist, c[0], c[1], MIN_ZOOM, MAX_ZOOM);
        }
        self.pinch = Some(Pinch {
            centroid: c,
            dist: d,
        });
        self.dirty = true;
    }

    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        self.camera.x -= dx / self.camera.zoom;
        self.camera.y -= dy / self.camera.zoom;
        self.dirty = true;
    }

    pub fn zoom_at(&mut self, factor: f32, x: f32, y: f32) {
        self.camera.zoom_at(factor, x, y, MIN_ZOOM, MAX_ZOOM);
        self.dirty = true;
    }

    pub fn set_camera(&mut self, x: f32, y: f32, zoom: f32) {
        self.camera = Camera {
            x,
            y,
            zoom: zoom.clamp(MIN_ZOOM, MAX_ZOOM),
        };
        self.dirty = true;
    }

    // ----------------------------------------------------------- edits

    fn commit_stroke(&mut self, s: StrokeData, now_ms: f64) {
        let page = self.page.clone();
        let data = ItemData::Stroke(s);
        if let Ok(id) = self.lesson.add_item(&page, &data, now_ms) {
            if let ItemData::Stroke(s) = &data {
                self.sink.add_stroke(&id, s);
            }
            let z = self.lesson.item(&page, &id).map(|i| i.z).unwrap_or(0.0);
            self.scene.insert(Item {
                id,
                z,
                created: now_ms,
                data,
            });
            self.revision += 1;
        }
    }

    /// Visually erase immediately; the document change is committed once per
    /// eraser gesture so a whole scrub is a single undo step.
    fn erase_at(&mut self, a: [f32; 2], b: [f32; 2], r: f32) -> Vec<ItemId> {
        let hits = self.scene.erase_hits(a, b, r);
        for id in &hits {
            self.scene.remove(id);
            self.sink.remove_stroke(id);
        }
        if !hits.is_empty() {
            self.dirty = true;
        }
        hits
    }

    fn commit_erase(&mut self, hits: Vec<ItemId>) {
        if hits.is_empty() {
            return;
        }
        let _ = self.lesson.remove_items(&self.page.clone(), &hits);
        self.selection.retain(|s| !hits.contains(s));
        self.bump();
    }

    fn preview_move(&mut self, total: [f32; 2]) {
        self.drag_offset = total;
        for id in self.selection.clone() {
            if let Some(Item {
                data: ItemData::Stroke(s),
                ..
            }) = self.scene.get(&id)
            {
                let mut moved = s.clone();
                moved.offset = [s.offset[0] + total[0], s.offset[1] + total[1]];
                self.sink.add_stroke(&id, &moved);
            }
        }
        self.bump();
    }

    pub fn select(&mut self, ids: Vec<ItemId>) {
        self.selection = ids
            .into_iter()
            .filter(|id| self.scene.get(id).is_some())
            .collect();
        self.bump();
    }

    pub fn select_all(&mut self) {
        self.selection = self.scene.ordered().iter().map(|i| i.id.clone()).collect();
        self.bump();
    }

    pub fn delete_selection(&mut self) {
        let sel = std::mem::take(&mut self.selection);
        if sel.is_empty() {
            return;
        }
        let _ = self.lesson.remove_items(&self.page.clone(), &sel);
        self.resync();
    }

    pub fn delete_item(&mut self, id: &str) {
        let _ = self
            .lesson
            .remove_items(&self.page.clone(), &[id.to_string()]);
        self.resync();
    }

    pub fn move_items(&mut self, ids: &[ItemId], dx: f64, dy: f64) {
        let _ = self.lesson.translate_items(&self.page.clone(), ids, dx, dy);
        self.resync();
    }

    pub fn set_geom(&mut self, id: &str, g: BoxGeom) {
        let _ = self.lesson.set_geom(&self.page.clone(), id, g);
        self.resync();
    }

    pub fn bring_to_front(&mut self, id: &str) {
        let _ = self.lesson.bring_to_front(&self.page.clone(), id);
        self.resync();
    }

    pub fn clear_page(&mut self) {
        let ids: Vec<ItemId> = self.scene.ordered().iter().map(|i| i.id.clone()).collect();
        let _ = self.lesson.remove_items(&self.page.clone(), &ids);
        self.resync();
    }

    /// Place a Smart Object (single-file HTML). Returns the item id, or the
    /// validation errors joined by newlines.
    pub fn add_object_html(
        &mut self,
        html: &str,
        title: &str,
        w: f64,
        h: f64,
        steps: u32,
        now_ms: f64,
    ) -> Result<ItemId, String> {
        let bundle = ObjectBundle::from_single_html(html, title, w, h, steps);
        let errors: Vec<String> = bundle
            .validate()
            .into_iter()
            .filter(|d| d.severity == rupok_format::Severity::Error)
            .map(|d| d.message)
            .collect();
        if !errors.is_empty() {
            return Err(errors.join("\n"));
        }
        let mut slug: String = title
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        slug.truncate(40);
        if slug.is_empty() {
            slug = "object".into();
        }
        let mut bundle_id = slug.clone();
        let mut n = 2;
        while self.pkg.objects.contains_key(&bundle_id) {
            bundle_id = format!("{slug}-{n}");
            n += 1;
        }
        self.pkg.objects.insert(bundle_id.clone(), bundle);
        let geom = self.centered_geom(w, h);
        let data = ItemData::Object(ObjectData {
            geom,
            bundle: bundle_id,
            title: title.into(),
            step: 0,
            state: None,
        });
        self.add_box_item(data, now_ms).map_err(|e| e.to_string())
    }

    fn centered_geom(&self, w: f64, h: f64) -> BoxGeom {
        let (c, _) = self.visible_area();
        let (cx, cy) = self.camera.screen_to_world(c[0], c[1]);
        BoxGeom {
            x: f64::from(cx) - w / 2.0,
            y: f64::from(cy) - h / 2.0,
            w,
            h,
            rot: 0.0,
        }
    }

    fn add_box_item(&mut self, data: ItemData, now_ms: f64) -> Result<ItemId, rupok_core::CoreError> {
        let id = self.lesson.add_item(&self.page.clone(), &data, now_ms)?;
        self.resync();
        self.selection = vec![id.clone()];
        Ok(id)
    }

    pub fn add_text(
        &mut self,
        x: f32,
        y: f32,
        text: &str,
        size: f64,
        now_ms: f64,
    ) -> Option<ItemId> {
        let w = self.world(x, y);
        let data = ItemData::Text(TextData {
            geom: BoxGeom {
                x: f64::from(w[0]),
                y: f64::from(w[1]),
                w: 600.0,
                h: size * 1.5,
                rot: 0.0,
            },
            text: text.into(),
            size,
            color: self.pen.color,
        });
        self.add_box_item(data, now_ms).ok()
    }

    pub fn set_text(&mut self, id: &str, text: &str, h: Option<f64>) {
        let page = self.page.clone();
        if text.trim().is_empty() {
            let _ = self.lesson.remove_items(&page, &[id.to_string()]);
        } else {
            let _ = self.lesson.set_text(&page, id, text);
            if let (
                Some(h),
                Ok(Item {
                    data: ItemData::Text(t),
                    ..
                }),
            ) = (h, self.lesson.item(&page, id))
            {
                let _ = self.lesson.set_geom(&page, id, BoxGeom { h, ..t.geom });
            }
        }
        self.resync();
    }

    pub fn add_image(
        &mut self,
        bytes: Vec<u8>,
        ext: &str,
        w: f64,
        h: f64,
        now_ms: f64,
    ) -> Option<ItemId> {
        let asset = self.pkg.add_asset(bytes, ext);
        let geom = self.centered_geom(w, h);
        self.add_box_item(ItemData::Image(ImageData { geom, asset }), now_ms)
            .ok()
    }

    pub fn set_object_step(&mut self, id: &str, step: u32) {
        let _ = self.lesson.set_object_step(&self.page.clone(), id, step);
        self.resync();
    }

    pub fn set_object_state(&mut self, id: &str, state: &str) {
        // State saves are frequent and not user actions; no revision bump needed
        // beyond the scene refresh.
        let _ = self.lesson.set_object_state(&self.page.clone(), id, state);
        if let Ok(it) = self.lesson.item(&self.page.clone(), id) {
            self.scene.insert(it);
        }
    }

    pub fn undo(&mut self) -> bool {
        let ok = self.lesson.undo().unwrap_or(false);
        if ok {
            self.resync();
        }
        ok
    }

    pub fn redo(&mut self) -> bool {
        let ok = self.lesson.redo().unwrap_or(false);
        if ok {
            self.resync();
        }
        ok
    }

    // ------------------------------------------------------- persistence

    pub fn save(&mut self, now_ms: f64) -> Result<Vec<u8>, String> {
        // Drop bundles no longer referenced by any page.
        let mut used = HashSet::new();
        for p in self.lesson.pages() {
            for it in self.lesson.items(&p.id).unwrap_or_default() {
                match it.data {
                    ItemData::Object(o) => {
                        used.insert(o.bundle);
                    }
                    ItemData::Image(i) => {
                        used.insert(i.asset);
                    }
                    _ => {}
                }
            }
        }
        self.pkg.objects.retain(|k, _| used.contains(k));
        self.pkg.assets.retain(|k, _| used.contains(k));
        self.pkg.update_lesson(&self.lesson, GENERATOR, now_ms);
        self.pkg.write().map_err(|e| e.to_string())
    }

    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        let pkg = Package::read(bytes).map_err(|e| e.to_string())?;
        let lesson = pkg.lesson().map_err(|e| e.to_string())?;
        self.pkg = pkg;
        self.lesson = lesson;
        self.replay = None;
        self.page = self.lesson.page_id_at(0).map_err(|e| e.to_string())?;
        self.load_page();
        self.fit_page();
        Ok(())
    }

    pub fn new_lesson(&mut self, title: &str, now_ms: f64) {
        self.lesson = Lesson::new(title, now_ms);
        self.pkg = Package::from_lesson(&self.lesson, GENERATOR, now_ms);
        self.replay = None;
        self.page = self.lesson.page_id_at(0).unwrap_or_default();
        self.load_page();
        self.fit_page();
    }

    // ------------------------------------------------------------- views

    /// Non-ink items for the DOM layer, in stacking order.
    pub fn dom_items_json(&self) -> String {
        let visible: Option<HashSet<&str>> = self.replay.as_ref().map(|r| r.visible_dom_ids());
        let sel: HashSet<&str> = self.selection.iter().map(String::as_str).collect();
        let items: Vec<DomItem> = self
            .scene
            .ordered()
            .into_iter()
            .filter(|it| !matches!(it.data, ItemData::Stroke(_)))
            .filter(|it| visible.as_ref().is_none_or(|v| v.contains(it.id.as_str())))
            .map(|it| {
                let selected = sel.contains(it.id.as_str());
                let off = if selected {
                    self.drag_offset
                } else {
                    [0.0, 0.0]
                };
                let base = |g: &BoxGeom, kind| DomItem {
                    id: &it.id,
                    kind,
                    x: g.x + f64::from(off[0]),
                    y: g.y + f64::from(off[1]),
                    w: g.w,
                    h: g.h,
                    rot: g.rot,
                    z: it.z,
                    title: None,
                    bundle: None,
                    step: None,
                    steps: None,
                    state: None,
                    text: None,
                    size: None,
                    color: None,
                    asset: None,
                    selected,
                };
                match &it.data {
                    ItemData::Object(o) => DomItem {
                        title: Some(&o.title),
                        bundle: Some(&o.bundle),
                        step: Some(o.step),
                        steps: self.pkg.objects.get(&o.bundle).map(|b| b.manifest.steps),
                        state: o.state.as_deref(),
                        ..base(&o.geom, "object")
                    },
                    ItemData::Text(t) => DomItem {
                        text: Some(&t.text),
                        size: Some(t.size),
                        color: Some(css_color(t.color)),
                        ..base(&t.geom, "text")
                    },
                    ItemData::Image(i) => DomItem {
                        asset: Some(&i.asset),
                        ..base(&i.geom, "image")
                    },
                    ItemData::Stroke(_) => unreachable!(),
                }
            })
            .collect();
        serde_json::to_string(&items).unwrap_or_else(|_| "[]".into())
    }

    pub fn pages_json(&self) -> String {
        serde_json::to_string(&self.lesson.pages()).unwrap_or_else(|_| "[]".into())
    }

    /// World-space bounds of the selection (with live drag offset), or None.
    pub fn selection_bounds(&self) -> Option<[f32; 4]> {
        self.scene.bounds_of(&self.selection).map(|b| {
            let o = self.drag_offset;
            [b[0] + o[0], b[1] + o[1], b[2] + o[0], b[3] + o[1]]
        })
    }

    pub fn object_html(&self, bundle: &str) -> Option<String> {
        self.pkg
            .objects
            .get(bundle)
            .and_then(|b| b.entry_html().map(str::to_string))
    }

    pub fn asset(&self, name: &str) -> Option<&[u8]> {
        self.pkg.assets.get(name).map(Vec::as_slice)
    }

    pub fn stroke_count(&self) -> usize {
        self.scene.len()
    }

    // ---------------------------------------------------------- transients

    /// Rebuild the wet layer: active strokes (+prediction), shape previews,
    /// lasso outline, fading laser trails, and replay's partial stroke.
    fn update_wet(&mut self, now_ms: f64) {
        let mut wet: Vec<WetStroke> = Vec::new();
        let zoom = self.camera.zoom;
        for g in self.gestures.values() {
            match g {
                Gesture::Ink { builder, predicted } => {
                    let mut pts = builder.points().to_vec();
                    pts.extend_from_slice(predicted);
                    let st = builder.style;
                    wet.push(WetStroke {
                        points: pts,
                        width: st.width,
                        color: st.color,
                        opacity: st.opacity,
                    });
                }
                Gesture::Lasso { poly, .. } if poly.len() > 1 => {
                    let mut pts: Vec<StrokePoint> = poly
                        .iter()
                        .map(|p| StrokePoint {
                            x: p[0],
                            y: p[1],
                            p: 0.0,
                            dt: 0,
                        })
                        .collect();
                    pts.push(pts[0]);
                    wet.push(WetStroke {
                        points: pts,
                        width: 2.4 / zoom,
                        color: 0x3b82f6ff,
                        opacity: 0.9,
                    });
                }
                Gesture::Shape { kind, start, end } => {
                    let st = self.shape_style;
                    wet.push(WetStroke {
                        points: shape_points(*kind, *start, *end, st.width),
                        width: st.width,
                        color: st.color,
                        opacity: st.opacity,
                    });
                }
                Gesture::Laser { points } => wet.push(laser_wet(points, now_ms, zoom)),
                _ => {}
            }
        }
        self.lasers.retain(|l| {
            l.points
                .last()
                .is_some_and(|(_, t)| now_ms - t < LASER_LIFETIME_MS)
        });
        for l in &self.lasers {
            wet.push(laser_wet(&l.points, now_ms, zoom));
        }
        if let Some(r) = &self.replay
            && let Some(w) = r.partial()
        {
            wet.push(w);
        }
        wet.retain(|w| !w.points.is_empty());
        self.sink.set_wet(&wet);
        self.dirty = true;
    }

    /// Per-frame tick for animations (laser fade, replay playback).
    pub fn tick(&mut self, now_ms: f64) {
        if let Some(r) = &mut self.replay
            && r.playing
        {
            let t = r.t + (now_ms - r.last_tick).clamp(0.0, 100.0) * r.speed;
            r.last_tick = now_ms;
            self.replay_seek(t, now_ms);
            if self.replay.as_ref().is_some_and(|r| r.t >= r.duration)
                && let Some(r) = &mut self.replay
            {
                r.playing = false;
            }
        }
        if !self.lasers.is_empty()
            || self
                .gestures
                .values()
                .any(|g| matches!(g, Gesture::Laser { .. }))
        {
            self.update_wet(now_ms);
        }
    }

    // ------------------------------------------------------------- replay

    pub fn replay_start(&mut self) -> f64 {
        let items = self.lesson.items(&self.page).unwrap_or_default();
        let r = Replay::new(items);
        let d = r.duration;
        self.replay = Some(r);
        self.sink.clear();
        self.selection.clear();
        self.gestures.clear();
        self.bump();
        d
    }

    pub fn replay_seek(&mut self, t: f64, now_ms: f64) {
        let Some(r) = &mut self.replay else { return };
        let before = r.visible_dom_count();
        r.seek(t, &mut self.sink);
        let changed = r.visible_dom_count() != before;
        if changed {
            self.revision += 1;
        }
        self.update_wet(now_ms);
    }

    pub fn replay_play(&mut self, playing: bool, speed: f64, now_ms: f64) {
        if let Some(r) = &mut self.replay {
            if playing && r.t >= r.duration {
                r.t = 0.0;
                r.reset(&mut self.sink);
            }
            r.playing = playing;
            r.speed = speed.clamp(0.25, 16.0);
            r.last_tick = now_ms;
        }
    }

    pub fn replay_state(&self) -> Option<(f64, f64, bool)> {
        self.replay.as_ref().map(|r| (r.t, r.duration, r.playing))
    }

    pub fn replay_stop(&mut self) {
        if self.replay.take().is_some() {
            self.load_page();
        }
    }
}

fn laser_wet(points: &[(StrokePoint, f64)], now_ms: f64, zoom: f32) -> WetStroke {
    let pts: Vec<StrokePoint> = points
        .iter()
        .filter(|(_, t)| now_ms - t < LASER_LIFETIME_MS)
        .map(|(p, t)| StrokePoint {
            p: (1.0 - ((now_ms - t) / LASER_LIFETIME_MS) as f32).clamp(0.0, 1.0),
            ..*p
        })
        .collect();
    WetStroke {
        points: pts,
        width: 9.0 / zoom,
        color: 0xff2d55ff,
        opacity: 0.85,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[derive(Default)]
    pub struct MockSink {
        pub strokes: HashMap<String, StrokeData>,
        pub wet: Vec<WetStroke>,
        pub loads: usize,
    }

    impl InkSink for MockSink {
        fn add_stroke(&mut self, id: &str, s: &StrokeData) {
            self.strokes.insert(id.into(), s.clone());
        }
        fn remove_stroke(&mut self, id: &str) {
            self.strokes.remove(id);
        }
        fn clear(&mut self) {
            self.strokes.clear();
        }
        fn set_wet(&mut self, strokes: &[WetStroke]) {
            self.wet = strokes.to_vec();
        }
        fn finish_load(&mut self) {
            self.loads += 1;
        }
    }

    fn engine() -> Engine<MockSink> {
        let mut e = Engine::new(MockSink::default(), "Test", 0.0);
        e.set_view(1280.0, 720.0);
        e
    }

    fn draw(e: &mut Engine<MockSink>, id: i32, kind: PointerKind, from: [f32; 2], to: [f32; 2]) {
        e.pointer_down(id, from[0], from[1], 0.5, 0.0, kind, 0, 0.0);
        let mut s = Vec::new();
        for i in 1..=20 {
            let t = i as f32 / 20.0;
            s.extend_from_slice(&[
                f64::from(from[0] + (to[0] - from[0]) * t),
                f64::from(from[1] + (to[1] - from[1]) * t),
                0.5,
                f64::from(i) * 8.0,
            ]);
        }
        e.pointer_move(id, &s, &[], 1.0);
        e.pointer_up(id, 2.0);
    }

    #[test]
    fn pen_stroke_commits_to_doc_scene_and_renderer() {
        let mut e = engine();
        draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [400.0, 120.0]);
        assert_eq!(e.sink.strokes.len(), 1);
        assert_eq!(e.stroke_count(), 1);
        assert_eq!(e.lesson.items(&e.page.clone()).unwrap().len(), 1);
        assert!(e.sink.wet.is_empty(), "wet layer cleared after pen up");
        assert!(e.undo());
        assert_eq!(e.sink.strokes.len(), 0);
        assert!(e.redo());
        assert_eq!(e.sink.strokes.len(), 1);
    }

    #[test]
    fn eraser_scrub_is_one_undo_step() {
        let mut e = engine();
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 100.0],
            [400.0, 100.0],
        );
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 200.0],
            [400.0, 200.0],
        );
        e.set_tool(ToolKind::Eraser);
        draw(&mut e, 2, PointerKind::Mouse, [250.0, 50.0], [250.0, 250.0]);
        assert_eq!(e.sink.strokes.len(), 0);
        assert_eq!(e.lesson.items(&e.page.clone()).unwrap().len(), 0);
        assert!(e.undo());
        assert_eq!(e.sink.strokes.len(), 2, "both strokes return with one undo");
    }

    #[test]
    fn palm_rejection_after_pen_seen() {
        let mut e = engine();
        draw(
            &mut e,
            1,
            PointerKind::Touch,
            [100.0, 100.0],
            [300.0, 100.0],
        );
        assert_eq!(e.stroke_count(), 1, "finger draws on touch-only boards");
        draw(&mut e, 2, PointerKind::Pen, [100.0, 300.0], [300.0, 300.0]);
        assert_eq!(e.stroke_count(), 2);
        let cam = e.camera;
        draw(
            &mut e,
            3,
            PointerKind::Touch,
            [100.0, 400.0],
            [300.0, 400.0],
        );
        assert_eq!(
            e.stroke_count(),
            2,
            "palm/finger must not draw once a pen is in use"
        );
        assert_ne!(e.camera, cam, "finger pans instead");
    }

    #[test]
    fn two_finger_pinch_zooms() {
        let mut e = engine();
        e.touch_mode = TouchMode::Navigate;
        e.pointer_down(1, 500.0, 300.0, 0.0, 0.0, PointerKind::Touch, 0, 0.0);
        e.pointer_down(2, 700.0, 300.0, 0.0, 0.0, PointerKind::Touch, 0, 0.0);
        let z0 = e.camera.zoom;
        e.pointer_move(1, &[450.0, 300.0, 0.0, 10.0], &[], 0.0);
        e.pointer_move(2, &[750.0, 300.0, 0.0, 10.0], &[], 0.0);
        assert!(e.camera.zoom > z0);
    }

    #[test]
    fn lasso_select_and_move() {
        let mut e = engine();
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 100.0],
            [200.0, 100.0],
        );
        let z = e.camera.zoom;
        e.set_tool(ToolKind::Lasso);
        e.pointer_down(1, 50.0, 50.0, 0.5, 0.0, PointerKind::Mouse, 0, 0.0);
        e.pointer_move(
            1,
            &[
                250.0, 50.0, 0.5, 1.0, 250.0, 150.0, 0.5, 2.0, 50.0, 150.0, 0.5, 3.0,
            ],
            &[],
            0.0,
        );
        e.pointer_up(1, 0.0);
        assert_eq!(e.selection().len(), 1);
        let id = e.selection()[0].clone();
        e.pointer_down(1, 150.0, 100.0, 0.5, 0.0, PointerKind::Mouse, 0, 0.0);
        e.pointer_move(1, &[250.0, 100.0, 0.5, 1.0], &[], 0.0);
        e.pointer_up(1, 0.0);
        match &e.lesson.item(&e.page.clone(), &id).unwrap().data {
            ItemData::Stroke(s) => assert!((s.offset[0] - 100.0 / z).abs() < 1e-3),
            _ => panic!(),
        }
    }

    #[test]
    fn shapes_and_objects_and_save_load() {
        let mut e = engine();
        e.set_tool(ToolKind::Shape(ShapeKind::Arrow));
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 100.0],
            [300.0, 100.0],
        );
        let id = e
            .add_object_html("<h1>Hello</h1>", "Hello World", 640.0, 360.0, 0, 5.0)
            .unwrap();
        assert!(
            e.add_object_html(
                r#"<script src="https://cdn.example.com/x.js"></script>"#,
                "Bad",
                640.0,
                360.0,
                0,
                6.0
            )
            .is_err()
        );
        let dom: serde_json::Value = serde_json::from_str(&e.dom_items_json()).unwrap();
        assert_eq!(dom.as_array().unwrap().len(), 1);
        assert_eq!(dom[0]["bundle"], "hello-world");
        e.set_object_step(&id, 2);
        let bytes = e.save(10.0).unwrap();
        let mut e2 = engine();
        e2.load(&bytes).unwrap();
        assert_eq!(e2.stroke_count(), 2);
        assert_eq!(e2.object_html("hello-world").unwrap(), "<h1>Hello</h1>");
        let dom: serde_json::Value = serde_json::from_str(&e2.dom_items_json()).unwrap();
        assert_eq!(dom[0]["step"], 2);
    }

    #[test]
    fn replay_rebuilds_progressively() {
        let mut e = engine();
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 100.0],
            [300.0, 100.0],
        );
        e.pointer_down(1, 100.0, 200.0, 0.5, 1000.0, PointerKind::Mouse, 0, 5000.0);
        e.pointer_move(1, &[300.0, 200.0, 0.5, 1400.0], &[], 5000.0);
        e.pointer_up(1, 5000.0);
        let d = e.replay_start();
        assert!(d > 0.0);
        assert!(e.sink.strokes.is_empty());
        e.replay_seek(d, 0.0);
        assert_eq!(e.sink.strokes.len(), 2);
        e.replay_seek(0.0, 0.0);
        assert!(e.sink.strokes.len() <= 1);
        e.replay_stop();
        assert_eq!(e.sink.strokes.len(), 2);
    }
}
