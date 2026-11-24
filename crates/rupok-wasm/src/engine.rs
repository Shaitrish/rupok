//! The board engine: tools, gestures, selection, pages, undo and replay.
//!
//! Pure Rust with no GPU or browser dependency so it is unit-testable natively.
//! Visual output goes through [`InkSink`] (the GPU renderer in the app, a mock
//! in tests); DOM-rendered items (objects, text, images) are exposed as JSON
//! for the UI layer, which re-reads them whenever [`Engine::revision`] changes.

use std::collections::{BTreeMap, HashMap, HashSet};

use rupok_core::{
    BoxGeom, Branding, Frame, ImageData, Item, ItemData, ItemId, Lesson, MediaData, NewItem,
    ObjectData, PageBackground, PageId, PageSpec, PageStyle, Pattern, Scene, ShapeGeom, StrokeData,
    TextData,
};
use rupok_format::package::OUTLINE_MAX_AGE_MS;
use rupok_format::{ObjectBundle, ObjectManifest, Package};
use rupok_ink::filter::OneEuroParams;
use rupok_ink::pressure::PressureCurve;
use rupok_ink::{Brush, BuilderConfig, InputPoint, StrokeBuilder, StrokePoint, StrokeStyle, Tool};
use rupok_render::Camera;
use serde::{Deserialize, Serialize};

use crate::b64;
use crate::replay::Replay;
use crate::shapes::{ShapeKind, shape_points};

#[cfg(test)]
mod brush_tests;
mod clip;
mod duster;
mod restyle;
mod reveal;
mod smart;
#[cfg(test)]
mod ux_tests;
pub use duster::EraserMode;
pub use smart::{SmartInk, SmartMode};

pub const GENERATOR: &str = concat!("rupok/", env!("CARGO_PKG_VERSION"));
/// Zoom range on an infinite board (a page without a frame).
const MIN_ZOOM: f32 = 0.05;
const MAX_ZOOM: f32 = 40.0;
/// On a slide, zoom runs from fit (the smallest it gets: ink stays on the
/// page, so there is nothing to see around it) to this many times fit.
const MAX_ZOOM_OVER_FIT: f32 = 5.0;
/// A touch pinch may stretch the slide below fit to this fraction of it;
/// lifting the fingers springs it back over [`SETTLE_MS`].
const PINCH_UNDER_FIT: f32 = 0.7;
const SETTLE_MS: f64 = 220.0;
const LASER_LIFETIME_MS: f64 = 1400.0;
/// Magic ink stays this long after the pen lifts, then fades out.
pub const MAGIC_HOLD_MS: f64 = 2600.0;
pub const MAGIC_FADE_MS: f64 = 900.0;
const DEFAULT_FIT_MARGIN: f32 = 0.03;
/// Clipboard format marker, the first key of every clipboard JSON.
pub const CLIP_MARKER: &str = "clipboard/v1";
/// Clipboard JSON comes from other tabs and the system clipboard: bound it.
const MAX_CLIP_BYTES: usize = 192 << 20;
const MAX_CLIP_ITEMS: usize = 20_000;
/// Smart Object state limit (spec/smart-object.md).
const MAX_STATE_BYTES: usize = 256 * 1024;
const READ_ONLY: &str = "this lesson is open read-only";

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
    pub brush: Brush,
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
                    .map(|w| (w.points.as_slice(), w.width, w.brush, w.color, w.opacity)),
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
        /// Wall-clock time of pen-down: the stroke's `created`.
        started: f64,
        /// Smart Ink hold-to-snap state.
        hold: smart::Hold,
    },
    Erase {
        last: [f32; 2],
        hits: Vec<ItemId>,
    },
    Lasso {
        poly: Vec<[f32; 2]>,
        start_screen: [f32; 2],
        moved: bool,
        /// Tap tolerance, screen px (a finger needs more than a pen).
        tol: f32,
    },
    MoveSelection {
        last: [f32; 2],
        total: [f32; 2],
    },
    Shape {
        kind: ShapeKind,
        start: [f32; 2],
        end: [f32; 2],
        started: f64,
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

/// Magic ink after pen-up: drawn exactly like its last wet frame (same
/// points, brush and page clip), then faded out. Never saved.
struct MagicTrail {
    stroke: WetStroke,
    clip: Option<[f32; 4]>,
    lifted: f64,
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
    /// A text item whose text is LaTeX maths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latex: Option<bool>,
    /// An image item that is the poster of a video, audio clip or embed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<&'a MediaData>,
    /// A text item that is a classroom stamp (`stamp` key).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stamp: Option<&'a str>,
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
    /// Margin `fit_page` leaves around the page (fraction of its size).
    fit_margin: f32,
    /// A pinch released below fit springs back: (zoom at release, start ms).
    settle: Option<(f32, f64)>,
    reduced_motion: bool,
    tool: ToolKind,
    pen: StrokeStyle,
    highlighter: StrokeStyle,
    shape_style: StrokeStyle,
    eraser_radius: f32,
    /// Partial (rub out what the eraser covers) or whole strokes.
    eraser_mode: EraserMode,
    /// The partial eraser's pending edit (`duster.rs`): committed as one
    /// undo step when the rub ends.
    cut: Option<duster::Cut>,
    cut_seq: u64,
    /// Keep ink, shapes and placed items inside a slide page (`clip.rs`).
    clip_ink: bool,
    /// The page rect of the gesture in progress, when ink is kept inside.
    ink_clip: Option<[f32; 4]>,
    smoothing: f32,
    pressure_gamma: f32,
    pub touch_mode: TouchMode,
    pen_seen: bool,
    gestures: HashMap<i32, Gesture>,
    touches: HashMap<i32, [f32; 2]>,
    /// The finger driving a tool gesture and its last screen position: a
    /// second finger turns both into a pinch.
    touch_gesture: Option<(i32, [f32; 2])>,
    pinch: Option<Pinch>,
    selection: Vec<ItemId>,
    drag_offset: [f32; 2],
    lasers: Vec<LaserTrail>,
    magic: Vec<MagicTrail>,
    replay: Option<Replay>,
    /// AI handwriting being written stroke by stroke (`reveal.rs`).
    reveal: Option<reveal::Reveal>,
    revision: u64,
    dirty: bool,
    /// Student view: every document edit is rejected.
    read_only: bool,
    /// Smart Ink settings, the last pen stroke and scratch-out flashes.
    smart: SmartInk,
    last_ink: Option<smart::LastInk>,
    flashes: Vec<smart::Flash>,
    /// The last stroke and when to read it again (deferred recognisers).
    pending: Option<(ItemId, f64)>,
    /// The last plain pen stroke committed (maths live preview).
    last_pen: Option<ItemId>,
}

fn css_color(rgba: u32) -> String {
    format!("#{:08x}", rgba)
}

/// Lowercase ASCII slug (`[a-z0-9-]`, at most 40 chars) for bundle ids.
fn slug(s: &str, fallback: &str) -> String {
    let mut out = s
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
    out.truncate(40);
    if out.is_empty() { fallback.into() } else { out }
}

#[derive(Serialize)]
struct Preview<'a> {
    frame: Option<Frame>,
    bg: u32,
    pattern: &'static str,
    /// The page's background image.
    #[serde(skip_serializing_if = "Option::is_none")]
    background: Option<PageBackground>,
    /// The branding the page shows (its own, else the lesson's).
    #[serde(skip_serializing_if = "Option::is_none")]
    branding: Option<Branding>,
    /// Union bounds of all items (for infinite boards), world units.
    bounds: Option<[f32; 4]>,
    items: Vec<PreviewItem<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum PreviewItem<'a> {
    Stroke {
        color: String,
        width: f32,
        opacity: f32,
        /// Flat `[x0, y0, x1, y1, …]`.
        pts: Vec<f64>,
        /// Pen style, when not the plain pen.
        #[serde(skip_serializing_if = "Option::is_none")]
        brush: Option<&'static str>,
    },
    Text {
        rect: [f64; 4],
        text: &'a str,
        size: f64,
        color: String,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        latex: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        stamp: Option<&'a str>,
    },
    Image {
        rect: [f64; 4],
        asset: &'a str,
    },
    Object {
        rect: [f64; 4],
        title: &'a str,
    },
}

/// Copied items with everything they need to be pasted elsewhere.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clip {
    /// Item data and group in stacking order (bottom first).
    pub items: Vec<(ItemData, Option<String>)>,
    pub bundles: BTreeMap<String, ObjectBundle>,
    pub assets: BTreeMap<String, Vec<u8>>,
    /// World bounds of the copied items where they were copied from.
    pub bounds: Option<[f32; 4]>,
}

/// Clipboard JSON. `rupok` is the marker and comes first, so the UI can
/// recognise a Rupok clipboard in `text/plain` without parsing it.
#[derive(Serialize, Deserialize)]
struct ClipJson {
    rupok: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bounds: Option<[f32; 4]>,
    items: Vec<ClipEntryJson>,
    #[serde(default)]
    bundles: BTreeMap<String, ClipBundleJson>,
    /// Asset name → base64 bytes.
    #[serde(default)]
    assets: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize)]
struct ClipEntryJson {
    #[serde(flatten)]
    item: ClipItemJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum ClipItemJson {
    Stroke(ClipStrokeJson),
    Text(TextData),
    Image(ImageData),
    Object(ObjectData),
}

#[derive(Serialize, Deserialize)]
struct ClipStrokeJson {
    tool: String,
    color: u32,
    width: f32,
    opacity: f32,
    /// Point codec v1 bytes, base64.
    pts: String,
    #[serde(default)]
    ox: f32,
    #[serde(default)]
    oy: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shape: Option<String>,
    /// Editable shape geometry (`geom`), so pasted shapes stay resizable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    geom: Option<ShapeGeom>,
    /// Pen style (`brush`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    brush: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ClipBundleJson {
    manifest: ObjectManifest,
    /// Relative path → base64 bytes.
    files: BTreeMap<String, String>,
}

fn finite_geom(g: &BoxGeom) -> bool {
    [g.x, g.y, g.w, g.h, g.rot].iter().all(|v| v.is_finite()) && g.w >= 0.0 && g.h >= 0.0
}

impl Clip {
    pub fn to_json(&self) -> String {
        let items = self
            .items
            .iter()
            .map(|(d, group)| ClipEntryJson {
                group: group.clone(),
                item: match d {
                    ItemData::Stroke(s) => ClipItemJson::Stroke(ClipStrokeJson {
                        tool: s.tool.clone(),
                        color: s.color,
                        width: s.width,
                        opacity: s.opacity,
                        pts: b64::encode(&rupok_ink::encode_points(&s.points)),
                        ox: s.offset[0],
                        oy: s.offset[1],
                        shape: s.shape.clone(),
                        geom: s.geom.clone(),
                        brush: s.brush.clone(),
                    }),
                    ItemData::Text(t) => ClipItemJson::Text(t.clone()),
                    ItemData::Image(i) => ClipItemJson::Image(i.clone()),
                    ItemData::Object(o) => ClipItemJson::Object(o.clone()),
                },
            })
            .collect();
        let doc = ClipJson {
            rupok: CLIP_MARKER.into(),
            bounds: self.bounds,
            items,
            bundles: self
                .bundles
                .iter()
                .map(|(k, b)| {
                    (
                        k.clone(),
                        ClipBundleJson {
                            manifest: b.manifest.clone(),
                            files: b
                                .files
                                .iter()
                                .map(|(p, bytes)| (p.clone(), b64::encode(bytes)))
                                .collect(),
                        },
                    )
                })
                .collect(),
            assets: self
                .assets
                .iter()
                .map(|(k, v)| (k.clone(), b64::encode(v)))
                .collect(),
        };
        serde_json::to_string(&doc).unwrap_or_default()
    }

    /// Parse and validate clipboard JSON. Never panics on bad input.
    pub fn from_json(json: &str) -> Result<Self, String> {
        if json.len() > MAX_CLIP_BYTES {
            return Err("clipboard data is too large".into());
        }
        let doc: ClipJson =
            serde_json::from_str(json).map_err(|e| format!("not a Rupok clipboard: {e}"))?;
        if doc.rupok != CLIP_MARKER {
            return Err(format!("unsupported clipboard format {:?}", doc.rupok));
        }
        if doc.items.len() > MAX_CLIP_ITEMS {
            return Err("too many items on the clipboard".into());
        }
        let mut clip = Clip {
            bounds: doc.bounds,
            ..Default::default()
        };
        for entry in doc.items {
            let group = entry.group.filter(|g| !g.is_empty() && g.len() <= 256);
            clip.items.push((
                match entry.item {
                    ClipItemJson::Stroke(s) => {
                        let bytes = b64::decode(&s.pts)?;
                        let points = rupok_ink::decode_points(&bytes).map_err(|e| e.to_string())?;
                        if points.is_empty() {
                            continue;
                        }
                        if !(s.width.is_finite() && s.opacity.is_finite())
                            || !(s.ox.is_finite() && s.oy.is_finite())
                        {
                            return Err("invalid stroke".into());
                        }
                        ItemData::Stroke(StrokeData {
                            tool: s.tool,
                            color: s.color,
                            width: s.width.clamp(0.1, 1000.0),
                            opacity: s.opacity.clamp(0.0, 1.0),
                            points,
                            offset: [s.ox, s.oy],
                            shape: s.shape,
                            // Pasted connectors start free: their links name
                            // the originals' shapes.
                            geom: s.geom.filter(ShapeGeom::is_valid).map(|g| ShapeGeom {
                                from: None,
                                to: None,
                                ..g
                            }),
                            brush: s.brush.filter(|b| b.len() <= 32),
                        })
                    }
                    ClipItemJson::Text(t) if finite_geom(&t.geom) && t.size.is_finite() => {
                        ItemData::Text(t)
                    }
                    ClipItemJson::Image(i) if finite_geom(&i.geom) => ItemData::Image(i),
                    ClipItemJson::Object(mut o) if finite_geom(&o.geom) => {
                        if o.state
                            .as_ref()
                            .is_some_and(|st| st.len() > MAX_STATE_BYTES)
                        {
                            o.state = None;
                        }
                        ItemData::Object(o)
                    }
                    _ => return Err("invalid item geometry".into()),
                },
                group,
            ));
        }
        for (id, b) in doc.bundles {
            let mut files = BTreeMap::new();
            for (path, data) in b.files {
                if !rupok_format::safe_path(&path) {
                    return Err(format!("unsafe path in object bundle: {path}"));
                }
                files.insert(path, b64::decode(&data)?);
            }
            clip.bundles.insert(
                id,
                ObjectBundle {
                    manifest: b.manifest,
                    files,
                },
            );
        }
        for (name, data) in doc.assets {
            clip.assets.insert(name, b64::decode(&data)?);
        }
        Ok(clip)
    }
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
            fit_margin: DEFAULT_FIT_MARGIN,
            settle: None,
            reduced_motion: false,
            tool: ToolKind::Pen,
            pen: StrokeStyle {
                tool: Tool::Pen,
                color: 0x1d2433ff,
                width: 3.0,
                opacity: 1.0,
                brush: Brush::Pen,
            },
            highlighter: StrokeStyle {
                tool: Tool::Highlighter,
                color: 0xffd60aff,
                width: 22.0,
                opacity: 0.38,
                brush: Brush::Pen,
            },
            shape_style: StrokeStyle {
                tool: Tool::Pen,
                color: 0x1d2433ff,
                width: 3.0,
                opacity: 1.0,
                brush: Brush::Pen,
            },
            eraser_radius: 32.0,
            eraser_mode: EraserMode::Partial,
            cut: None,
            cut_seq: 0,
            clip_ink: true,
            ink_clip: None,
            smoothing: 0.5,
            pressure_gamma: 0.8,
            touch_mode: TouchMode::Auto,
            pen_seen: false,
            gestures: HashMap::new(),
            touches: HashMap::new(),
            touch_gesture: None,
            pinch: None,
            selection: Vec::new(),
            drag_offset: [0.0, 0.0],
            lasers: Vec::new(),
            magic: Vec::new(),
            replay: None,
            reveal: None,
            revision: 1,
            dirty: true,
            read_only: false,
            smart: SmartInk::default(),
            last_ink: None,
            flashes: Vec::new(),
            pending: None,
            last_pen: None,
        };
        e.load_page();
        e
    }

    // ---------------------------------------------------------------- state

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Student view: reject every document edit (drawing, erasing, moving,
    /// adding, deleting, page changes, undo). Navigation, zoom, replay and
    /// opening a lesson still work.
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        if read_only {
            self.gestures.clear();
            self.selection.clear();
            self.sink.set_wet(&[]);
            self.bump();
        }
    }

    pub fn read_only(&self) -> bool {
        self.read_only
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
        !self.lasers.is_empty()
            || !self.magic.is_empty()
            || !self.flashes.is_empty()
            || self.replay.as_ref().is_some_and(|r| r.playing)
            || self.reveal_animating()
            || self.settle.is_some()
    }

    /// The board refits a fitted slide on resize (controller.ts); here the
    /// camera only comes back inside the slide's bounds.
    pub fn set_view(&mut self, w: f32, h: f32) {
        self.view = [w.max(1.0), h.max(1.0)];
        self.clamp_camera();
        self.dirty = true;
    }

    /// Tell the engine which screen margins are covered by UI chrome, so
    /// "fit page" and new-item placement use the visible area.
    pub fn set_insets(&mut self, top: f32, right: f32, bottom: f32, left: f32) {
        self.insets = [top.max(0.0), right.max(0.0), bottom.max(0.0), left.max(0.0)];
        self.clamp_camera();
        self.dirty = true;
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

    /// Highlighter opacity for new strokes; each stroke stores its own.
    pub fn set_highlighter_opacity(&mut self, a: f32) {
        if a.is_finite() {
            self.highlighter.opacity = a.clamp(0.1, 1.0);
        }
    }

    /// Pen style of the current tool's new strokes (pen, highlighter or the
    /// shape tools; each remembers its own). Magic ink is for the pen only.
    pub fn set_brush(&mut self, b: Brush) {
        let b = if b == Brush::Magic && self.tool != ToolKind::Pen {
            Brush::Pen
        } else {
            b
        };
        self.style_mut().brush = b;
    }

    pub fn brush(&mut self) -> Brush {
        self.style_mut().brush
    }

    /// Pen style and opacity of one tool's new strokes, whatever tool is
    /// active: `"pen"`, `"highlighter"` or `"shape"`. The highlighter keeps
    /// its own opacity (`set_highlighter_opacity`); magic ink is pen-only.
    pub fn set_tool_brush(&mut self, kind: &str, b: Brush, opacity: f32) {
        let opacity = if opacity.is_finite() {
            opacity.clamp(0.1, 1.0)
        } else {
            1.0
        };
        match kind {
            "pen" => {
                self.pen.brush = b;
                self.pen.opacity = opacity;
            }
            "highlighter" => {
                self.highlighter.brush = if b == Brush::Magic { Brush::Pen } else { b }
            }
            "shape" => {
                self.shape_style.brush = if b == Brush::Magic { Brush::Pen } else { b };
                self.shape_style.opacity = opacity;
            }
            _ => {}
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
        // A page change shows any writing still being revealed in full.
        self.reveal = None;
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
        self.cut = None;
        self.magic.clear();
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
        if self.read_only {
            return self.page_index();
        }
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
        if self.read_only {
            return;
        }
        if self.lesson.page_count() <= 1 {
            return;
        }
        let idx = self.page_index();
        let _ = self.lesson.remove_page(&self.page.clone());
        self.goto_page(idx.min(self.lesson.page_count() - 1));
    }

    pub fn move_page(&mut self, from: usize, to: usize) {
        if self.read_only {
            return;
        }
        if self.lesson.move_page(from, to).is_ok() {
            self.bump();
        }
    }

    pub fn set_background(&mut self, pattern: Pattern, color: u32) {
        if self.read_only {
            return;
        }
        let _ = self
            .lesson
            .set_page_background(&self.page.clone(), pattern, color);
        self.bump();
    }

    /// Add a page that looks like the current one (colour, pattern,
    /// background image, page branding): right after it, or at the end.
    /// Shows the new page and returns its index.
    pub fn add_page_like(&mut self, at_end: bool) -> usize {
        if self.read_only {
            return self.page_index();
        }
        let idx = (!at_end).then(|| self.page_index() + 1);
        let Ok(id) = self.lesson.add_page_like(idx, &self.page.clone()) else {
            return self.page_index();
        };
        let i = self
            .lesson
            .pages()
            .iter()
            .position(|p| p.id == id)
            .unwrap_or(0);
        self.goto_page(i);
        i
    }

    /// Insert a page that looks like page `src` right before or after it,
    /// and show it. Returns its index.
    pub fn insert_page_like(&mut self, src: usize, before: bool) -> usize {
        if self.read_only {
            return self.page_index();
        }
        let Ok(src_id) = self.lesson.page_id_at(src) else {
            return self.page_index();
        };
        let at = if before { src } else { src + 1 };
        let Ok(id) = self.lesson.add_page_like(Some(at), &src_id) else {
            return self.page_index();
        };
        let i = self
            .lesson
            .pages()
            .iter()
            .position(|p| p.id == id)
            .unwrap_or(0);
        self.goto_page(i);
        i
    }

    fn page_ids_at(&self, indices: &[usize]) -> Vec<PageId> {
        indices
            .iter()
            .filter_map(|&i| self.lesson.page_id_at(i).ok())
            .collect()
    }

    /// Delete the pages at `indices` (one undo step); at least one page
    /// stays. Shows the page after the first deleted one. Returns how many
    /// were deleted.
    pub fn delete_pages(&mut self, indices: &[usize]) -> usize {
        if self.read_only {
            return 0;
        }
        let ids = self.page_ids_at(indices);
        let first = indices.iter().copied().min().unwrap_or(0);
        let current = self.page.clone();
        let Ok(n) = self.lesson.remove_pages(&ids) else {
            return 0;
        };
        let at = if ids.contains(&current) {
            first.min(self.lesson.page_count().saturating_sub(1))
        } else {
            self.lesson
                .pages()
                .iter()
                .position(|p| p.id == current)
                .unwrap_or(0)
        };
        self.goto_page(at);
        n
    }

    /// Copy the pages at `indices`, each right after its original (one undo
    /// step). Returns the copies' indexes.
    pub fn duplicate_pages(&mut self, indices: &[usize]) -> Vec<usize> {
        if self.read_only {
            return Vec::new();
        }
        let ids = self.page_ids_at(indices);
        let Ok(copies) = self.lesson.duplicate_pages(&ids) else {
            return Vec::new();
        };
        let pages = self.lesson.pages();
        let out: Vec<usize> = copies
            .iter()
            .filter_map(|c| pages.iter().position(|p| &p.id == c))
            .collect();
        self.bump();
        out
    }

    /// Move the pages at `indices` together to the gap before page `gap`
    /// (indexes before the move), in order, as one undo step.
    pub fn move_pages(&mut self, indices: &[usize], gap: usize) {
        if self.read_only {
            return;
        }
        let ids = self.page_ids_at(indices);
        if self.lesson.move_pages(&ids, gap).is_ok() {
            self.bump();
        }
    }

    /// Give the current page, or every page, the look `style` (one undo
    /// step).
    pub fn set_page_style(&mut self, style: &PageStyle, all: bool) -> Result<(), String> {
        if self.read_only {
            return Ok(());
        }
        let ids: Vec<PageId> = if all {
            self.lesson.pages().into_iter().map(|p| p.id).collect()
        } else {
            vec![self.page.clone()]
        };
        self.lesson
            .set_page_style(&ids, style)
            .map_err(|e| e.to_string())?;
        self.bump();
        Ok(())
    }

    /// Set or remove (`None`) the lesson's branding, or, with `page_only`,
    /// the current page's own branding (one undo step).
    pub fn set_branding(&mut self, b: Option<&Branding>, page_only: bool) -> Result<(), String> {
        if self.read_only {
            return Ok(());
        }
        let r = if page_only {
            self.lesson.set_page_branding(&self.page.clone(), b)
        } else {
            self.lesson.set_branding(b)
        };
        r.map_err(|e| e.to_string())?;
        self.bump();
        Ok(())
    }

    /// Store bytes as a package asset (deduplicated by BLAKE3) and return
    /// its name, for a background image or a logo. Nothing references it
    /// until the document does; unreferenced assets are not saved.
    pub fn add_asset(&mut self, bytes: Vec<u8>, ext: &str) -> Option<String> {
        if self.read_only {
            return None;
        }
        Some(self.pkg.add_asset(bytes, ext))
    }

    /// Forget the undo stack (a new lesson's default design is not an
    /// action to undo).
    pub fn forget_history(&mut self) {
        self.lesson.forget_history();
        self.bump();
    }

    /// Fit the page into the visible area with the current fit margin
    /// (3% by default; Teach mode sets 0). Page changes and loads use it too.
    pub fn fit_page(&mut self) {
        self.fit_page_margin(self.fit_margin);
    }

    /// The margin [`Engine::fit_page`] leaves around the page, as a fraction
    /// of its size: 0.03 by default, 0 for a slide that touches the screen.
    pub fn set_fit_margin(&mut self, margin: f32) {
        self.fit_margin = if margin.is_finite() {
            margin.clamp(0.0, 1.0)
        } else {
            DEFAULT_FIT_MARGIN
        };
    }

    /// Fit the page into the visible area leaving `margin` (a fraction of the
    /// page size) around it. With 0 the page touches both edges of the
    /// limiting axis and is centred on the other.
    pub fn fit_page_margin(&mut self, margin: f32) {
        self.settle = None;
        self.camera = self.fit_camera(margin);
        self.dirty = true;
    }

    fn fit_camera(&self, margin: f32) -> Camera {
        let m = 1.0
            + if margin.is_finite() {
                margin.clamp(0.0, 1.0)
            } else {
                0.0
            };
        let frame = self
            .lesson
            .page(&self.page)
            .ok()
            .and_then(|p| p.frame)
            .unwrap_or(Frame::SLIDE_16_9);
        let (fw, fh) = (frame.w as f32, frame.h as f32);
        let (c, size) = self.visible_area();
        let zoom = (size[0] / (fw * m))
            .min(size[1] / (fh * m))
            .clamp(MIN_ZOOM, MAX_ZOOM);
        Camera {
            x: fw / 2.0 - c[0] / zoom,
            y: fh / 2.0 - c[1] / zoom,
            zoom,
        }
    }

    /// The zoom [`Engine::fit_page`] gives: 100 % on the zoom chip.
    pub fn fit_zoom(&self) -> f32 {
        self.fit_camera(self.fit_margin).zoom
    }

    /// The slide frame, or `None` on an infinite board.
    fn slide_frame(&self) -> Option<Frame> {
        self.lesson.page(&self.page).ok().and_then(|p| p.frame)
    }

    /// Zoom range: fit to [`MAX_ZOOM_OVER_FIT`] × fit on a slide.
    fn zoom_range(&self) -> (f32, f32) {
        if self.slide_frame().is_some() {
            let f = self.fit_zoom();
            (f, f * MAX_ZOOM_OVER_FIT)
        } else {
            (MIN_ZOOM, MAX_ZOOM)
        }
    }

    /// Keep the slide on screen: a slide smaller than the visible area is
    /// centred in it; a larger one cannot leave a gap at either edge.
    fn clamp_pan(&mut self) {
        let Some(f) = self.slide_frame() else {
            return;
        };
        let (c, s) = self.visible_area();
        let z = self.camera.zoom;
        let axis = |x: f32, len: f32, c: f32, s: f32| {
            if len * z <= s {
                len / 2.0 - c / z
            } else {
                x.clamp(-(c - s / 2.0) / z, len - (c + s / 2.0) / z)
            }
        };
        self.camera.x = axis(self.camera.x, f.w as f32, c[0], s[0]);
        self.camera.y = axis(self.camera.y, f.h as f32, c[1], s[1]);
    }

    /// Bring the camera into the slide's zoom range and pan bounds.
    fn clamp_camera(&mut self) {
        let (lo, hi) = self.zoom_range();
        let (c, _) = self.visible_area();
        self.camera.zoom_at(1.0, c[0], c[1], lo, hi);
        self.clamp_pan();
    }

    /// Spring back a pinch released below fit (no animation with reduced motion).
    pub fn set_reduced_motion(&mut self, on: bool) {
        self.reduced_motion = on;
    }

    fn settle_tick(&mut self, now_ms: f64) -> bool {
        let Some((from, t0)) = self.settle else {
            return false;
        };
        let lo = self.zoom_range().0;
        let t = ((now_ms - t0) / SETTLE_MS).clamp(0.0, 1.0) as f32;
        let ease = 1.0 - (1.0 - t).powi(3);
        let (c, _) = self.visible_area();
        let z = from + (lo - from) * ease;
        self.camera
            .zoom_at(z / self.camera.zoom, c[0], c[1], MIN_ZOOM, MAX_ZOOM);
        if t >= 1.0 {
            self.settle = None;
            self.clamp_camera();
        } else {
            self.clamp_pan();
        }
        self.dirty = true;
        true
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
        // A second finger while one finger drives a tool gesture: cancel that
        // gesture (an unfinished stroke is dropped) and pinch with both.
        if kind == PointerKind::Touch
            && let Some((fid, pos)) = self.touch_gesture.take()
            && fid != id
        {
            match self.gestures.remove(&fid) {
                Some(Gesture::Erase { hits, .. }) => self.commit_erase(hits),
                Some(Gesture::MoveSelection { .. }) => self.preview_move([0.0, 0.0]),
                _ => {}
            }
            self.touches.insert(fid, pos);
        }
        // Fingers navigate when palm rejection says so, except in Select,
        // where one finger selects and moves (two still pinch).
        if kind == PointerKind::Touch
            && (self.read_only
                || (!self.touch_draws()
                    && !matches!(self.tool, ToolKind::Lasso | ToolKind::Eraser))
                || !self.touches.is_empty()
                || self.tool == ToolKind::Hand)
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
        if self.read_only {
            self.gestures.insert(id, Gesture::Pan { last: [x, y] });
            return;
        }
        if !matches!(self.tool, ToolKind::Lasso | ToolKind::Hand) && !self.selection.is_empty() {
            // Starting to draw dismisses a selection left over from placing an item.
            self.selection.clear();
            self.bump();
        }
        let w = self.world(x, y);
        // On a slide page ink stays on the page: a stroke or shape that
        // starts outside it draws nothing.
        self.ink_clip = self.ink_bounds();
        if self.starts_off_page(w) {
            self.update_wet(now_ms);
            return;
        }
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
                    started: now_ms,
                    hold: smart::Hold::new(w, now_ms),
                }
            }
            ToolKind::Eraser => {
                let r = self.eraser_radius / self.camera.zoom;
                let hits = self.erase_at(w, w, r);
                Gesture::Erase { last: w, hits }
            }
            ToolKind::Lasso => {
                let tol = match kind {
                    PointerKind::Touch => 18.0,
                    PointerKind::Pen => 10.0,
                    PointerKind::Mouse => 6.0,
                };
                if self.inside_selection(w) {
                    Gesture::MoveSelection {
                        last: w,
                        total: [0.0, 0.0],
                    }
                } else if let Some(hit) = self.press_target(w, tol) {
                    // One gesture: press on an item selects it (with its
                    // group) and dragging moves it right away.
                    self.selection = self.expand_groups(&[hit]);
                    self.bump();
                    Gesture::MoveSelection {
                        last: w,
                        total: [0.0, 0.0],
                    }
                } else {
                    Gesture::Lasso {
                        poly: vec![w],
                        start_screen: [x, y],
                        moved: false,
                        tol,
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
                started: now_ms,
            },
        };
        if kind == PointerKind::Touch {
            self.touch_gesture = Some((id, [x, y]));
        }
        self.gestures.insert(id, g);
        self.update_wet(now_ms);
    }

    /// The item a press in Select grabs: the topmost item under the pointer,
    /// unless it is a page-sized box (an imported PDF slide, a full-slide
    /// object). Those would swallow every lasso drawn over them, so a drag
    /// there lassos and a plain click still selects them.
    fn press_target(&self, w: [f32; 2], tol: f32) -> Option<ItemId> {
        let it = self.scene.hit_test(w[0], w[1], tol / self.camera.zoom)?;
        if !matches!(it.data, ItemData::Stroke(_))
            && let Some(f) = self.lesson.page(&self.page).ok().and_then(|p| p.frame)
        {
            let b = it.data.aabb();
            let area = f64::from((b[2] - b[0]).max(0.0)) * f64::from((b[3] - b[1]).max(0.0));
            if area >= 0.85 * f.w * f.h {
                return None;
            }
        }
        Some(it.id.clone())
    }

    /// Grow a set of items to whole groups, keeping order and without repeats.
    fn expand_groups(&self, ids: &[ItemId]) -> Vec<ItemId> {
        let groups: HashSet<&str> = ids
            .iter()
            .filter_map(|id| self.scene.get(id)?.group.as_deref())
            .collect();
        let mut seen: HashSet<&str> = HashSet::new();
        let mut out: Vec<ItemId> = Vec::with_capacity(ids.len());
        for id in ids {
            if self.scene.get(id).is_some() && seen.insert(id.as_str()) {
                out.push(id.clone());
            }
        }
        if !groups.is_empty() {
            for it in self.scene.ordered() {
                if it.group.as_deref().is_some_and(|g| groups.contains(g))
                    && seen.insert(it.id.as_str())
                {
                    out.push(it.id.clone());
                }
            }
        }
        out
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
        if let Some((fid, pos)) = &mut self.touch_gesture
            && *fid == id
            && let Some(last) = samples.as_chunks::<4>().0.iter().last()
        {
            *pos = [last[0] as f32, last[1] as f32];
        }
        let zoom = self.camera.zoom;
        let cam = self.camera;
        let hold_mode = self.smart.mode == SmartMode::Hold;
        let clip = self.ink_clip;
        let shape_width = self.shape_style.width;
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
                hold,
                ..
            } => {
                for s in samples.as_chunks::<4>().0 {
                    let w = to_world(s[0], s[1]);
                    builder.push(InputPoint {
                        x: w[0],
                        y: w[1],
                        pressure: s[2] as f32,
                        t_ms: s[3],
                    });
                    if hold_mode && builder.style.brush != Brush::Magic {
                        Self::hold_move(hold, w, now_ms, zoom);
                    }
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
                ..
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
                    *end = clip::clamp_point(to_world(s[0], s[1]), clip, shape_width);
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
            let t = self.clamp_move(*total);
            self.preview_move(t);
        }
        self.update_wet(now_ms);
    }

    pub fn pointer_up(&mut self, id: i32, now_ms: f64) {
        if self.touch_gesture.is_some_and(|(fid, _)| fid == id) {
            self.touch_gesture = None;
        }
        if self.touches.remove(&id).is_some() {
            self.pinch = None;
            if self.touches.is_empty() && self.camera.zoom < self.zoom_range().0 {
                self.settle = Some((self.camera.zoom, now_ms));
                if self.reduced_motion {
                    self.settle_tick(f64::INFINITY);
                }
            }
            return;
        }
        let Some(g) = self.gestures.remove(&id) else {
            return;
        };
        match g {
            Gesture::Ink {
                builder,
                started,
                hold,
                ..
            } => {
                let style = builder.style;
                let pts = builder.finish();
                if style.brush == Brush::Magic {
                    // Disappearing ink: kept on screen as drawn, never saved.
                    if !pts.is_empty() {
                        self.magic.push(MagicTrail {
                            stroke: WetStroke {
                                points: pts,
                                width: style.width,
                                color: style.color,
                                opacity: style.opacity,
                                brush: Brush::Magic,
                            },
                            clip: self.ink_clip,
                            lifted: now_ms,
                        });
                    }
                } else if !pts.is_empty() {
                    self.finish_clipped(
                        StrokeData {
                            tool: style.tool.as_str().into(),
                            color: style.color,
                            width: style.width,
                            opacity: style.opacity,
                            points: pts,
                            offset: [0.0, 0.0],
                            shape: None,
                            geom: None,
                            brush: style.brush.key().map(Into::into),
                        },
                        started,
                        hold.snapped,
                        now_ms,
                    );
                }
            }
            Gesture::Erase { hits, .. } => self.commit_erase(hits),
            Gesture::Lasso {
                poly,
                start_screen,
                moved,
                tol,
            } => {
                let picked = if moved && poly.len() >= 3 {
                    self.scene.lasso(&poly)
                } else {
                    let w = self.world(start_screen[0], start_screen[1]);
                    self.scene
                        .hit_test(w[0], w[1], tol / self.camera.zoom)
                        .map(|i| vec![i.id.clone()])
                        .unwrap_or_default()
                };
                self.selection = self.expand_groups(&picked);
                self.bump();
            }
            Gesture::MoveSelection { total, .. } => {
                self.drag_offset = [0.0, 0.0];
                let total = self.clamp_move(total);
                if total != [0.0, 0.0] {
                    let sel = self.selection.clone();
                    self.translate_with_connectors(&sel, f64::from(total[0]), f64::from(total[1]));
                } else {
                    self.bump();
                }
            }
            Gesture::Shape {
                kind,
                start,
                end,
                started,
            } => {
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
                            geom: Some(crate::shapes::shape_geom(kind, start, end)),
                            brush: st.brush.key().map(Into::into),
                        },
                        started,
                    );
                }
            }
            Gesture::Pan { .. } => {}
            Gesture::Laser { points } => self.lasers.push(LaserTrail { points }),
        }
        self.update_wet(now_ms);
    }

    pub fn pointer_cancel(&mut self, id: i32, now_ms: f64) {
        if self.touch_gesture.is_some_and(|(fid, _)| fid == id) {
            self.touch_gesture = None;
        }
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
            let (lo, hi) = self.zoom_range();
            let lo = if self.slide_frame().is_some() {
                lo * PINCH_UNDER_FIT
            } else {
                lo
            };
            self.camera.zoom_at(d / p.dist, c[0], c[1], lo, hi);
            self.clamp_pan();
        }
        self.pinch = Some(Pinch {
            centroid: c,
            dist: d,
        });
        self.dirty = true;
    }

    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        self.settle = None;
        self.camera.x -= dx / self.camera.zoom;
        self.camera.y -= dy / self.camera.zoom;
        self.clamp_pan();
        self.dirty = true;
    }

    pub fn zoom_at(&mut self, factor: f32, x: f32, y: f32) {
        self.settle = None;
        let (lo, hi) = self.zoom_range();
        self.camera.zoom_at(factor, x, y, lo, hi);
        self.clamp_pan();
        self.dirty = true;
    }

    /// Set the camera as given (no slide bounds): restoring a saved view,
    /// or the desktop overlay pinning its own.
    pub fn set_camera(&mut self, x: f32, y: f32, zoom: f32) {
        self.settle = None;
        self.camera = Camera {
            x,
            y,
            zoom: zoom.clamp(MIN_ZOOM, MAX_ZOOM),
        };
        self.dirty = true;
    }

    // ----------------------------------------------------------- edits

    /// `created` is the pen-down wall-clock time (spec § Replay).
    fn commit_stroke(&mut self, s: StrokeData, created: f64) -> Option<ItemId> {
        let page = self.page.clone();
        let plain_pen = s.tool == "pen" && s.shape.is_none();
        let data = ItemData::Stroke(s);
        let id = self.lesson.add_item(&page, &data, created).ok()?;
        if plain_pen {
            self.last_pen = Some(id.clone());
        }
        if let ItemData::Stroke(s) = &data {
            self.sink.add_stroke(&id, s);
        }
        let z = self.lesson.item(&page, &id).map(|i| i.z).unwrap_or(0.0);
        self.scene.insert(Item {
            id: id.clone(),
            z,
            created,
            group: None,
            data,
        });
        self.revision += 1;
        Some(id)
    }

    /// Visually erase immediately; the document change is committed once per
    /// eraser gesture so a whole scrub is a single undo step.
    fn erase_at(&mut self, a: [f32; 2], b: [f32; 2], r: f32) -> Vec<ItemId> {
        if self.eraser_mode == EraserMode::Partial {
            // Rub out only what the eraser covers (`duster.rs`).
            self.cut_at(a, b, r);
            return Vec::new();
        }
        let mut hits = self.scene.erase_hits(a, b, r);
        // Grouped (merged) strokes erase as one; box items in the group stay.
        if hits
            .iter()
            .any(|h| self.scene.get(h).is_some_and(|i| i.group.is_some()))
        {
            hits = self
                .expand_groups(&hits)
                .into_iter()
                .filter(|id| {
                    self.scene
                        .get(id)
                        .is_some_and(|i| matches!(i.data, ItemData::Stroke(_)))
                })
                .collect();
        }
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
        // The partial eraser commits once every rub has ended.
        if self.cut.is_some()
            && !self
                .gestures
                .values()
                .any(|g| matches!(g, Gesture::Erase { .. }))
        {
            self.commit_cut();
        }
        if self.read_only {
            return;
        }
        if hits.is_empty() {
            return;
        }
        let _ = self.lesson.remove_items(&self.page.clone(), &hits);
        self.selection.retain(|s| !hits.contains(s));
        self.bump();
    }

    /// Gesture slot of the area eraser (real pointer ids are never negative).
    const AREA_ERASE_ID: i32 = i32::MIN;

    /// Area erase for the palm / hand eraser (the board's gesture layer):
    /// erase the strokes under a circle of screen radius `r` swept from
    /// screen point `a` to `b`. Only strokes are erased, never objects,
    /// images or text. Strokes vanish at once; `area_erase_end` commits them,
    /// so a whole rub is one undo step. Returns the strokes this call erased.
    pub fn area_erase(&mut self, a: [f32; 2], b: [f32; 2], r: f32) -> usize {
        let finite = a.iter().chain(&b).all(|v| v.is_finite()) && r.is_finite();
        if self.read_only || self.replay.is_some() || !finite || r <= 0.0 {
            return 0;
        }
        let r = r.min(400.0) / self.camera.zoom;
        let (wa, wb) = (self.world(a[0], a[1]), self.world(b[0], b[1]));
        let cut_before = self.cut_touched();
        let new_hits = self.erase_at(wa, wb, r);
        let n = new_hits.len() + self.cut_touched() - cut_before;
        if let Gesture::Erase { last, hits } = self
            .gestures
            .entry(Self::AREA_ERASE_ID)
            .or_insert_with(|| Gesture::Erase {
                last: wb,
                hits: Vec::new(),
            })
        {
            *last = wb;
            hits.extend(new_hits);
        }
        n
    }

    /// End an area erase: commit its strokes as one undo step. Returns how
    /// many strokes the whole rub erased.
    pub fn area_erase_end(&mut self) -> usize {
        match self.gestures.remove(&Self::AREA_ERASE_ID) {
            Some(Gesture::Erase { hits, .. }) => {
                let n = hits.len() + self.cut_touched();
                self.commit_erase(hits);
                n
            }
            _ => 0,
        }
    }

    fn preview_move(&mut self, total: [f32; 2]) {
        self.drag_offset = total;
        let sel = self.selection.clone();
        for id in &sel {
            if let Some(Item {
                data: ItemData::Stroke(s),
                ..
            }) = self.scene.get(id)
            {
                let mut moved = s.clone();
                moved.offset = [s.offset[0] + total[0], s.offset[1] + total[1]];
                self.sink.add_stroke(id, &moved);
            }
        }
        // Attached connectors follow (or let go) as the selection moves.
        if sel.len() <= 200 {
            for (id, s) in
                self.connectors_for_move(&sel, [f64::from(total[0]), f64::from(total[1])])
            {
                self.sink.add_stroke(&id, &s);
            }
        }
        self.bump();
    }

    pub fn select(&mut self, ids: Vec<ItemId>) {
        self.selection = self.expand_groups(&ids);
        self.bump();
    }

    // ------------------------------------------------------------ groups

    /// Group the selection (two or more items) into one new group; nested
    /// groups are flattened. Returns the group id.
    pub fn group_selection(&mut self) -> Option<String> {
        if self.read_only || self.selection.len() < 2 {
            return None;
        }
        let sel = self.selection.clone();
        let g = self.lesson.group_items(&self.page.clone(), &sel).ok()?;
        self.resync();
        Some(g)
    }

    /// Dissolve every group in the selection; the items stay selected.
    /// Returns how many groups were dissolved.
    pub fn ungroup_selection(&mut self) -> usize {
        if self.read_only {
            return 0;
        }
        let groups: Vec<String> = self
            .selection
            .iter()
            .filter_map(|id| self.scene.get(id)?.group.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let page = self.page.clone();
        for g in &groups {
            let _ = self.lesson.ungroup(&page, g);
        }
        if !groups.is_empty() {
            self.resync();
        }
        groups.len()
    }

    /// Merge the selected ink into one unit that selects, moves, copies and
    /// erases as one. A selection with box items is grouped instead.
    /// Returns `"merged"`, `"grouped"` or `"none"`.
    pub fn merge_selection(&mut self) -> &'static str {
        let strokes_only = self.selection.iter().all(|id| {
            self.scene
                .get(id)
                .is_some_and(|i| matches!(i.data, ItemData::Stroke(_)))
        });
        match self.group_selection() {
            Some(_) if strokes_only => "merged",
            Some(_) => "grouped",
            None => "none",
        }
    }

    /// What the selection holds, for the selection toolbar:
    /// `{count, strokes, boxes, groups, grouped}` where `grouped` is true
    /// when the whole selection is exactly one group.
    pub fn selection_meta_json(&self) -> String {
        let items: Vec<&Item> = self
            .selection
            .iter()
            .filter_map(|id| self.scene.get(id))
            .collect();
        let strokes = items
            .iter()
            .filter(|i| matches!(i.data, ItemData::Stroke(_)))
            .count();
        let groups: HashSet<&str> = items.iter().filter_map(|i| i.group.as_deref()).collect();
        let grouped = groups.len() == 1 && items.iter().all(|i| i.group.is_some());
        serde_json::json!({
            "count": items.len(),
            "strokes": strokes,
            "boxes": items.len() - strokes,
            "groups": groups.len(),
            "grouped": grouped,
            // Resizable shapes (Smart Ink templates with their labels): the
            // bounds and the first shape's name.
            "shape": self.selected_shapes(),
            "shapeName": items.iter().find_map(|it| match &it.data {
                ItemData::Stroke(s) if s.geom.is_some() => s.shape.clone(),
                _ => None,
            }),
            // An axes-and-curve graph that "Make it live" can replace.
            "live": self.graph_live_json() != "null",
            // Shared colour (#rrggbbaa) and stroke width, when uniform, and
            // whether all the ink is highlighter (`restyle.rs`).
            "style": self.selection_style(),
        })
        .to_string()
    }

    pub fn select_all(&mut self) {
        self.selection = self.scene.ordered().iter().map(|i| i.id.clone()).collect();
        self.bump();
    }

    pub fn delete_selection(&mut self) {
        if self.read_only {
            return;
        }
        let sel = std::mem::take(&mut self.selection);
        if sel.is_empty() {
            return;
        }
        let _ = self.lesson.remove_items(&self.page.clone(), &sel);
        self.resync();
    }

    pub fn delete_item(&mut self, id: &str) {
        if self.read_only {
            return;
        }
        let _ = self
            .lesson
            .remove_items(&self.page.clone(), &[id.to_string()]);
        self.resync();
    }

    pub fn move_items(&mut self, ids: &[ItemId], dx: f64, dy: f64) {
        if self.read_only {
            return;
        }
        self.translate_with_connectors(ids, dx, dy);
    }

    /// Move items, and in the same change the connectors attached to them
    /// (Smart Ink): one undo step.
    fn translate_with_connectors(&mut self, ids: &[ItemId], dx: f64, dy: f64) {
        let page = self.page.clone();
        let conn = self.connectors_for_move(ids, [dx, dy]);
        if conn.is_empty() {
            let _ = self.lesson.translate_items(&page, ids, dx, dy);
        } else {
            let moved: HashSet<&ItemId> = conn.iter().map(|c| &c.0).collect();
            let e = rupok_core::Edits {
                translate: ids
                    .iter()
                    .filter(|id| !moved.contains(id))
                    .map(|id| (id.clone(), dx, dy))
                    .collect(),
                strokes: conn,
                ..Default::default()
            };
            let _ = self.lesson.apply_edits(&page, &e);
        }
        self.resync();
    }

    pub fn set_geom(&mut self, id: &str, g: BoxGeom) {
        if self.read_only {
            return;
        }
        let _ = self.lesson.set_geom(&self.page.clone(), id, g);
        self.resync();
    }

    pub fn bring_to_front(&mut self, id: &str) {
        if self.read_only {
            return;
        }
        let _ = self.lesson.bring_to_front(&self.page.clone(), id);
        self.resync();
    }

    pub fn clear_page(&mut self) {
        if self.read_only {
            return;
        }
        let ids: Vec<ItemId> = self.scene.ordered().iter().map(|i| i.id.clone()).collect();
        let _ = self.lesson.remove_items(&self.page.clone(), &ids);
        self.resync();
    }

    /// Raise several items above everything else, keeping their relative
    /// order; one undo step.
    pub fn bring_to_front_many(&mut self, ids: &[ItemId]) {
        if self.read_only || ids.is_empty() {
            return;
        }
        let _ = self.lesson.bring_items_to_front(&self.page.clone(), ids);
        self.resync();
    }

    /// Resize a text item: box and font size together (one undo step).
    pub fn resize_text(&mut self, id: &str, g: BoxGeom, size: f64) {
        if self.read_only || !(size.is_finite() && size > 0.0) {
            return;
        }
        let _ = self
            .lesson
            .resize_text(&self.page.clone(), id, g, size.clamp(4.0, 2000.0));
        self.resync();
    }

    pub fn set_title(&mut self, title: &str) {
        if self.read_only {
            return;
        }
        self.lesson.set_title(title);
        self.bump();
    }

    /// Copy the page at `index` (with all items and their bundle/asset
    /// references) right after it and show the copy. Returns its index.
    pub fn duplicate_page(&mut self, index: usize) -> Option<usize> {
        if self.read_only {
            return None;
        }
        let src = self.lesson.page_id_at(index).ok()?;
        let id = self.lesson.duplicate_page(&src).ok()?;
        let i = self.lesson.pages().iter().position(|p| p.id == id)?;
        self.goto_page(i);
        Some(i)
    }

    // --------------------------------------------------------- clipboard

    /// Self-contained copy of items on the current page: item data plus the
    /// Smart Object bundles and image assets they reference.
    pub fn export_items(&self, ids: &[ItemId]) -> Clip {
        let want: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let mut clip = Clip::default();
        for it in self.scene.ordered() {
            if !want.contains(it.id.as_str()) {
                continue;
            }
            match &it.data {
                ItemData::Object(o) => {
                    if let Some(b) = self.pkg.objects.get(&o.bundle) {
                        clip.bundles.insert(o.bundle.clone(), b.clone());
                    }
                }
                ItemData::Image(i) => {
                    let src = i.media.as_ref().and_then(MediaData::src_asset);
                    for name in std::iter::once(i.asset.as_str()).chain(src) {
                        if let Some(a) = self.pkg.assets.get(name) {
                            clip.assets.insert(name.to_string(), a.clone());
                        }
                    }
                }
                _ => {}
            }
            clip.items.push((it.data.clone(), it.group.clone()));
        }
        clip.bounds = self.scene.bounds_of(ids);
        clip
    }

    /// [`Engine::export_items`] as clipboard JSON (bundle files and assets
    /// base64-encoded), so a paste works in another lesson or tab.
    pub fn export_items_json(&self, ids: &[ItemId]) -> String {
        self.export_items(ids).to_json()
    }

    /// Add a clip's items to the current page, moved by `(dx, dy)`, as one
    /// undo step. Bundles and assets are merged into the package (equal
    /// bundles are shared, clashing ids renamed). The new items become the
    /// selection; returns their ids.
    pub fn import_items(
        &mut self,
        clip: Clip,
        dx: f64,
        dy: f64,
        now_ms: f64,
    ) -> Result<Vec<ItemId>, String> {
        if self.read_only {
            return Err(READ_ONLY.into());
        }
        if !(dx.is_finite() && dy.is_finite()) {
            return Err("invalid offset".into());
        }
        let mut bundle_ids: HashMap<String, String> = HashMap::new();
        for (id, bundle) in clip.bundles {
            let errors: Vec<String> = bundle
                .validate()
                .into_iter()
                .filter(|d| d.severity == rupok_format::Severity::Error)
                .map(|d| d.message)
                .collect();
            if !errors.is_empty() {
                return Err(errors.join("\n"));
            }
            let target = if self.pkg.objects.get(&id) == Some(&bundle) {
                id.clone()
            } else if let Some((k, _)) = self.pkg.objects.iter().find(|(_, b)| **b == bundle) {
                k.clone()
            } else {
                let base = slug(&id, "object");
                let mut k = base.clone();
                let mut n = 2;
                while self.pkg.objects.contains_key(&k) {
                    k = format!("{base}-{n}");
                    n += 1;
                }
                self.pkg.objects.insert(k.clone(), bundle);
                k
            };
            bundle_ids.insert(id, target);
        }
        let mut asset_names: HashMap<String, String> = HashMap::new();
        for (name, bytes) in clip.assets {
            let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
            asset_names.insert(name.clone(), self.pkg.add_asset(bytes, ext));
        }
        // Copies form their own groups, apart from the originals.
        let mut groups: HashMap<String, String> = HashMap::new();
        let mut list: Vec<NewItem> = Vec::with_capacity(clip.items.len());
        for (i, (mut data, group)) in clip.items.into_iter().enumerate() {
            let keep = match &mut data {
                ItemData::Stroke(s) => {
                    s.offset = [s.offset[0] + dx as f32, s.offset[1] + dy as f32];
                    true
                }
                ItemData::Text(t) => {
                    t.geom.x += dx;
                    t.geom.y += dy;
                    true
                }
                ItemData::Image(img) => {
                    img.geom.x += dx;
                    img.geom.y += dy;
                    if let Some(n) = asset_names.get(&img.asset) {
                        img.asset = n.clone();
                    }
                    if let Some(m) = img.media.as_mut().filter(|m| m.src_asset().is_some()) {
                        if let Some(n) = asset_names.get(&m.src) {
                            m.src = n.clone();
                        }
                        if !self.pkg.assets.contains_key(&m.src) {
                            img.media = None;
                        }
                    }
                    self.pkg.assets.contains_key(&img.asset)
                }
                ItemData::Object(o) => {
                    o.geom.x += dx;
                    o.geom.y += dy;
                    if let Some(b) = bundle_ids.get(&o.bundle) {
                        o.bundle = b.clone();
                    }
                    self.pkg.objects.contains_key(&o.bundle)
                }
            };
            if keep {
                let group = group.map(|g| {
                    groups
                        .entry(g)
                        .or_insert_with(|| self.lesson.new_group_id())
                        .clone()
                });
                // 1 ms apart keeps the clip's order in the replay timeline.
                list.push(NewItem {
                    data,
                    created: now_ms + i as f64,
                    group,
                    z: None,
                });
            }
        }
        let ids = self
            .lesson
            .add_items(&self.page.clone(), &list)
            .map_err(|e| e.to_string())?;
        self.resync();
        self.selection = ids.clone();
        self.bump();
        Ok(ids)
    }

    /// Parse clipboard JSON (untrusted: from the system clipboard or another
    /// tab) and [`Engine::import_items`] it.
    pub fn import_items_json(
        &mut self,
        json: &str,
        dx: f64,
        dy: f64,
        now_ms: f64,
    ) -> Result<Vec<ItemId>, String> {
        if self.read_only {
            return Err(READ_ONLY.into());
        }
        let clip = Clip::from_json(json)?;
        self.import_items(clip, dx, dy, now_ms)
    }

    /// Copy items in place, offset by `(dx, dy)`; the copies become the
    /// selection. One undo step.
    pub fn duplicate_items(
        &mut self,
        ids: &[ItemId],
        dx: f64,
        dy: f64,
        now_ms: f64,
    ) -> Vec<ItemId> {
        let clip = self.export_items(ids);
        if clip.items.is_empty() {
            return Vec::new();
        }
        self.import_items(clip, dx, dy, now_ms).unwrap_or_default()
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
        if self.read_only {
            return Err(READ_ONLY.into());
        }
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
        let slug = slug(title, "object");
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

    fn add_box_item(
        &mut self,
        data: ItemData,
        now_ms: f64,
    ) -> Result<ItemId, rupok_core::CoreError> {
        if self.read_only {
            return Err(rupok_core::CoreError::Corrupt(READ_ONLY.into()));
        }
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
        let w = self.place_on_page(self.world(x, y), text, size);
        let data = ItemData::Text(TextData {
            latex: false,
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
            stamp: None,
        });
        self.add_box_item(data, now_ms).ok()
    }

    /// Place a classroom stamp centred on the screen point (`x`, `y`): a
    /// text item with the `stamp` key, `w` × `h` world units, holding
    /// `text` (the emoji or label older readers show). Kept on the page
    /// like ink; not selected, so stamps can be tapped down one after
    /// another.
    #[allow(clippy::too_many_arguments)]
    pub fn add_stamp(
        &mut self,
        kind: &str,
        text: &str,
        x: f32,
        y: f32,
        w: f64,
        h: f64,
        color: u32,
        now_ms: f64,
    ) -> Option<ItemId> {
        let ok = |v: f64| v.is_finite() && v > 0.0 && v < 1e5;
        if self.read_only || kind.is_empty() || kind.len() > 32 || !ok(w) || !ok(h) {
            return None;
        }
        let c = self.world(x, y);
        let (mut cx, mut cy) = (f64::from(c[0]), f64::from(c[1]));
        if let Some(r) = self.ink_bounds() {
            // Keep the stamp on a slide page, as ink is.
            let half = |a: f32, b: f32, s: f64| {
                let (a, b) = (f64::from(a), f64::from(b));
                (a + s / 2.0).min((a + b) / 2.0)..=(b - s / 2.0).max((a + b) / 2.0)
            };
            let (rx, ry) = (half(r[0], r[2], w), half(r[1], r[3], h));
            cx = cx.clamp(*rx.start(), *rx.end());
            cy = cy.clamp(*ry.start(), *ry.end());
        }
        let data = ItemData::Text(TextData {
            latex: false,
            geom: BoxGeom {
                x: cx - w / 2.0,
                y: cy - h / 2.0,
                w,
                h,
                rot: 0.0,
            },
            text: text.chars().take(64).collect(),
            size: (h * 0.7).max(4.0),
            color,
            stamp: Some(kind.to_ascii_lowercase()),
        });
        let id = self.add_box_item(data, now_ms).ok()?;
        self.selection.clear();
        self.bump();
        Some(id)
    }

    /// Move, resize and rotate a stamp (one undo step): the box and its
    /// rotation in radians; the fallback text follows the size.
    pub fn set_stamp_geom(&mut self, id: &str, g: BoxGeom) {
        if !finite_geom(&g) || g.h <= 0.0 {
            return;
        }
        self.resize_text(id, g, (g.h * 0.7).max(4.0));
    }

    pub fn set_text_color(&mut self, id: &str, rgba: u32) {
        if self.read_only {
            return;
        }
        let page = self.page.clone();
        if self.lesson.set_text_color(&page, id, rgba).is_ok() {
            self.resync();
        }
    }

    pub fn set_text(&mut self, id: &str, text: &str, h: Option<f64>) {
        if self.read_only {
            return;
        }
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
        if self.read_only {
            return None;
        }
        let asset = self.pkg.add_asset(bytes, ext);
        let geom = self.centered_geom(w, h);
        self.add_box_item(
            ItemData::Image(ImageData {
                geom,
                asset,
                media: None,
            }),
            now_ms,
        )
        .ok()
    }

    /// Place a media item (video, audio clip, YouTube video or web embed)
    /// centred in the view, as one undo step. `poster` is the picture shown
    /// until it plays (and in exports); `file` is the clip for `video` and
    /// `audio`, stored in the package (its asset name becomes `src`).
    pub fn add_media(
        &mut self,
        mut media: MediaData,
        poster: (Vec<u8>, &str),
        file: Option<(Vec<u8>, &str)>,
        w: f64,
        h: f64,
        now_ms: f64,
    ) -> Result<ItemId, String> {
        if self.read_only {
            return Err(READ_ONLY.into());
        }
        if media.src_asset().is_some() && file.is_none() {
            return Err("a video or audio item needs its file".into());
        }
        self.check_media(&mut media, file.as_ref().map(|f| f.0.as_slice()))?;
        if !(w.is_finite() && h.is_finite() && w >= 1.0 && h >= 1.0) {
            return Err("invalid size".into());
        }
        if let Some((bytes, ext)) = file {
            media.src = self.pkg.add_asset(bytes, ext);
        }
        let asset = self.pkg.add_asset(poster.0, poster.1);
        let geom = self.centered_geom(w, h);
        self.add_box_item(
            ItemData::Image(ImageData {
                geom,
                asset,
                media: Some(media),
            }),
            now_ms,
        )
        .map_err(|e| e.to_string())
    }

    /// Change what a media item plays (title, times, mute, loop, autoplay,
    /// or a new link), and optionally its poster or clip, as one undo step.
    pub fn set_media(
        &mut self,
        id: &str,
        mut media: MediaData,
        poster: Option<(Vec<u8>, &str)>,
        file: Option<(Vec<u8>, &str)>,
    ) -> Result<(), String> {
        if self.read_only {
            return Err(READ_ONLY.into());
        }
        let page = self.page.clone();
        let Ok(Item {
            data: ItemData::Image(old),
            ..
        }) = self.lesson.item(&page, id)
        else {
            return Err("not a media item".into());
        };
        let has_file = file.is_some();
        self.check_media(&mut media, file.as_ref().map(|f| f.0.as_slice()))?;
        if let Some((bytes, ext)) = file {
            media.src = self.pkg.add_asset(bytes, ext);
        } else if media.src_asset().is_some() {
            // Same clip as before unless a new file came with the change.
            match old.media.as_ref().and_then(MediaData::src_asset) {
                Some(src) if !has_file => media.src = src.to_string(),
                _ => return Err("a video or audio item needs its file".into()),
            }
        }
        let poster = poster.map(|(b, ext)| self.pkg.add_asset(b, ext));
        self.lesson
            .set_media(&page, id, &media, poster.as_deref())
            .map_err(|e| e.to_string())?;
        self.resync();
        Ok(())
    }

    /// Validate a media item before it is stored: a known kind, links only
    /// from the allowlist (stored in canonical form), files that really are
    /// video or audio and not too big.
    fn check_media(&self, media: &mut MediaData, file: Option<&[u8]>) -> Result<(), String> {
        use rupok_format::media;
        media.title = media
            .title
            .chars()
            .filter(|c| !c.is_control())
            .take(200)
            .collect();
        match media.kind.as_str() {
            "youtube" | "web" => {
                let link = media::parse_link(&media.src)
                    .filter(|l| l.kind == media.kind)
                    .ok_or("this link is not on the list of sites Rupok can embed")?;
                media.src = link.url;
                media.site = link.site.into();
            }
            "video" | "audio" => {
                media.site.clear();
                if let Some(bytes) = file {
                    if bytes.len() > media::MAX_MEDIA_BYTES {
                        return Err(format!(
                            "the file is too big (limit {} MB)",
                            media::MAX_MEDIA_BYTES >> 20
                        ));
                    }
                    let (kind, _) = media::sniff_media(bytes).ok_or("not a video or audio file")?;
                    // A video file may carry only sound (e.g. .webm audio).
                    if kind == "audio" && media.kind == "video" {
                        media.kind = "audio".into();
                    }
                }
                media.src = "pending".into();
            }
            _ => return Err(format!("unknown media type `{}`", media.kind)),
        }
        if media
            .end
            .is_some_and(|e| media.start.is_some_and(|s| e <= s))
        {
            media.end = None;
        }
        if media.is_valid() {
            Ok(())
        } else {
            Err("invalid media settings".into())
        }
    }

    /// Place a 3D model (glTF 2.0: `.glb`, or `.gltf` with everything
    /// embedded) as a built-in viewer Smart Object. Returns the item id.
    pub fn add_model(
        &mut self,
        bytes: &[u8],
        name: &str,
        title: &str,
        w: f64,
        h: f64,
        now_ms: f64,
    ) -> Result<ItemId, String> {
        if self.read_only {
            return Err(READ_ONLY.into());
        }
        let bundle = rupok_format::model3d::model_bundle(bytes, name, title, w, h)
            .map_err(|e| e.to_string())?;
        let title = bundle.manifest.title.clone();
        let base = slug(&title, "model");
        let mut bundle_id = base.clone();
        let mut n = 2;
        while self.pkg.objects.contains_key(&bundle_id) {
            bundle_id = format!("{base}-{n}");
            n += 1;
        }
        self.pkg.objects.insert(bundle_id.clone(), bundle);
        let geom = self.centered_geom(w, h);
        let data = ItemData::Object(ObjectData {
            geom,
            bundle: bundle_id,
            title,
            step: 0,
            state: None,
        });
        self.add_box_item(data, now_ms).map_err(|e| e.to_string())
    }

    pub fn set_object_step(&mut self, id: &str, step: u32) {
        if self.read_only {
            return;
        }
        let _ = self.lesson.set_object_step(&self.page.clone(), id, step);
        self.resync();
    }

    pub fn set_object_state(&mut self, id: &str, state: &str) {
        if self.read_only {
            return;
        }
        // State saves are frequent and not user actions; no revision bump needed
        // beyond the scene refresh.
        let _ = self.lesson.set_object_state(&self.page.clone(), id, state);
        if let Ok(it) = self.lesson.item(&self.page.clone(), id) {
            self.scene.insert(it);
        }
    }

    pub fn undo(&mut self) -> bool {
        if self.read_only {
            return false;
        }
        let ok = self.lesson.undo().unwrap_or(false);
        if ok {
            self.resync();
        }
        ok
    }

    pub fn redo(&mut self) -> bool {
        if self.read_only {
            return false;
        }
        let ok = self.lesson.redo().unwrap_or(false);
        if ok {
            self.resync();
        }
        ok
    }

    // ------------------------------------------------------- persistence

    pub fn save(&mut self, now_ms: f64) -> Result<Vec<u8>, String> {
        self.save_with(now_ms, f64::NEG_INFINITY)
    }

    /// [`Engine::save`] for the board's autosave: reuses the readable
    /// outline (`README.md`/`content.md`/`content.json`) for up to
    /// [`OUTLINE_MAX_AGE_MS`] instead of rebuilding it on every save.
    pub fn autosave(&mut self, now_ms: f64) -> Result<Vec<u8>, String> {
        self.save_with(now_ms, OUTLINE_MAX_AGE_MS)
    }

    fn save_with(&mut self, now_ms: f64, outline_max_age_ms: f64) -> Result<Vec<u8>, String> {
        // Leave out bundles and assets no page references (items, background
        // images, branding logos).
        let mut used = HashSet::new();
        if let Some(logo) = self.lesson.branding().and_then(|b| b.logo) {
            used.insert(logo);
        }
        for p in self.lesson.pages() {
            if let Some(bg) = &p.background {
                used.insert(bg.image.clone());
            }
            if let Some(logo) = p.branding.as_ref().and_then(|b| b.logo.clone()) {
                used.insert(logo);
            }
            for it in self.lesson.items(&p.id).unwrap_or_default() {
                match it.data {
                    ItemData::Object(o) => {
                        used.insert(o.bundle);
                    }
                    ItemData::Image(i) => {
                        if let Some(src) = i.media.as_ref().and_then(MediaData::src_asset) {
                            used.insert(src.to_string());
                        }
                        used.insert(i.asset);
                    }
                    _ => {}
                }
            }
        }
        // Kept in memory, not written: an undo or redo can bring a reference
        // back (a removed image, a replaced logo, a deleted video, an undone
        // Smart Object) after an autosave.
        let (objects, unused_objects): (BTreeMap<_, _>, BTreeMap<_, _>) =
            std::mem::take(&mut self.pkg.objects)
                .into_iter()
                .partition(|(k, _)| used.contains(k));
        let (assets, unused_assets): (BTreeMap<_, _>, BTreeMap<_, _>) =
            std::mem::take(&mut self.pkg.assets)
                .into_iter()
                .partition(|(k, _)| used.contains(k));
        self.pkg.objects = objects;
        self.pkg.assets = assets;
        self.pkg
            .update_lesson_throttled(&self.lesson, GENERATOR, now_ms, outline_max_age_ms);
        let out = self.pkg.write().map_err(|e| e.to_string());
        self.pkg.objects.extend(unused_objects);
        self.pkg.assets.extend(unused_assets);
        out
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
                    latex: None,
                    media: None,
                    stamp: None,
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
                        latex: t.latex.then_some(true),
                        stamp: t.stamp.as_deref(),
                        ..base(&t.geom, "text")
                    },
                    ItemData::Image(i) => DomItem {
                        asset: Some(&i.asset),
                        media: i.media.as_ref(),
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

    /// A light drawing list for the page thumbnail at `index`: stroke
    /// polylines decimated to at most `max_points` points (offset applied),
    /// plus box item rects, in stacking order. `"null"` for a bad index.
    pub fn page_preview_json(&self, index: usize, max_points: usize) -> String {
        let Ok(id) = self.lesson.page_id_at(index) else {
            return "null".into();
        };
        let Ok(info) = self.lesson.page(&id) else {
            return "null".into();
        };
        let items = self.lesson.items(&id).unwrap_or_default();
        let max_points = max_points.max(2);
        let r1 = |v: f64| (v * 10.0).round() / 10.0;
        let mut bounds: Option<[f32; 4]> = None;
        let mut out = Vec::with_capacity(items.len());
        for it in &items {
            let b = it.data.aabb();
            bounds = Some(match bounds {
                None => b,
                Some(a) => [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[2].max(b[2]),
                    a[3].max(b[3]),
                ],
            });
            let rect = |g: &BoxGeom| [r1(g.x), r1(g.y), r1(g.w), r1(g.h)];
            out.push(match &it.data {
                ItemData::Stroke(s) => {
                    let n = s.points.len();
                    let step = n.div_ceil(max_points).max(1);
                    let mut pts = Vec::with_capacity((n / step + 2) * 2);
                    let mut push = |p: &StrokePoint| {
                        pts.push(r1(f64::from(p.x + s.offset[0])));
                        pts.push(r1(f64::from(p.y + s.offset[1])));
                    };
                    for p in s.points.iter().step_by(step) {
                        push(p);
                    }
                    if n > 1 && (n - 1) % step != 0 {
                        push(&s.points[n - 1]);
                    }
                    PreviewItem::Stroke {
                        color: css_color(s.color),
                        width: s.width,
                        opacity: s.opacity,
                        pts,
                        brush: s.brush().key(),
                    }
                }
                ItemData::Text(t) => PreviewItem::Text {
                    rect: rect(&t.geom),
                    text: &t.text,
                    size: t.size,
                    color: css_color(t.color),
                    latex: t.latex,
                    stamp: t.stamp.as_deref(),
                },
                ItemData::Image(i) => PreviewItem::Image {
                    rect: rect(&i.geom),
                    asset: &i.asset,
                },
                ItemData::Object(o) => PreviewItem::Object {
                    rect: rect(&o.geom),
                    title: &o.title,
                },
            });
        }
        let branding = self.lesson.branding_for(&info);
        serde_json::to_string(&Preview {
            frame: info.frame,
            bg: info.bg_color,
            pattern: info.pattern.as_str(),
            background: info.background.clone(),
            branding,
            bounds,
            items: out,
        })
        .unwrap_or_else(|_| "null".into())
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
                Gesture::Ink {
                    builder,
                    hold:
                        smart::Hold {
                            snapped: Some(h), ..
                        },
                    ..
                } => wet.extend(Self::held_wet(builder, h)),
                Gesture::Ink {
                    builder, predicted, ..
                } => {
                    // Smoothed body + raw tip + the browser's prediction:
                    // the ink reaches the pen (rupok_ink::StrokeBuilder::tip).
                    let mut pts = Vec::with_capacity(builder.points().len() + 1 + predicted.len());
                    pts.extend_from_slice(builder.points());
                    pts.extend(builder.tip());
                    pts.extend_from_slice(predicted);
                    let st = builder.style;
                    clip::push_wet(
                        &mut wet,
                        WetStroke {
                            points: pts,
                            width: st.width,
                            color: st.color,
                            opacity: st.opacity,
                            brush: st.brush,
                        },
                        self.ink_clip,
                    );
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
                        brush: Brush::Pen,
                    });
                }
                Gesture::Shape {
                    kind, start, end, ..
                } => {
                    let st = self.shape_style;
                    wet.push(WetStroke {
                        points: shape_points(*kind, *start, *end, st.width),
                        width: st.width,
                        color: st.color,
                        opacity: st.opacity,
                        brush: st.brush,
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
        self.magic
            .retain(|m| now_ms - m.lifted < MAGIC_HOLD_MS + MAGIC_FADE_MS);
        for m in &self.magic {
            let k = magic_fade(now_ms - m.lifted);
            clip::push_wet(
                &mut wet,
                WetStroke {
                    opacity: m.stroke.opacity * k,
                    ..m.stroke.clone()
                },
                m.clip,
            );
        }
        self.flash_wet(now_ms, &mut wet);
        if let Some(r) = &self.replay
            && let Some(w) = r.partial()
        {
            wet.push(w);
        }
        if let Some(w) = self.reveal_partial() {
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
        let revealing = self.reveal_tick(now_ms) | self.settle_tick(now_ms);
        let snapped = self.hold_tick(now_ms) | self.deferred_tick(now_ms);
        if snapped
            || revealing
            || !self.lasers.is_empty()
            || !self.magic.is_empty()
            || !self.flashes.is_empty()
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
        self.reveal_finish();
        let items = self.lesson.items(&self.page).unwrap_or_default();
        let r = Replay::new(items);
        let d = r.duration;
        self.replay = Some(r);
        self.magic.clear();
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
        brush: Brush::Pen,
    }
}

/// Opacity factor of magic ink `age` ms after pen-up: full while it holds,
/// then an ease-out fade to nothing.
pub fn magic_fade(age: f64) -> f32 {
    let t = ((age - MAGIC_HOLD_MS) / MAGIC_FADE_MS).clamp(0.0, 1.0) as f32;
    (1.0 - t) * (1.0 - t)
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

    impl Engine<MockSink> {
        pub(crate) fn can_undo_for_tests(&self) -> bool {
            self.lesson.can_undo()
        }
    }

    pub(crate) fn engine() -> Engine<MockSink> {
        let mut e = Engine::new(MockSink::default(), "Test", 0.0);
        e.set_view(1280.0, 720.0);
        // Screen = world: most tests draw in world coordinates.
        e.set_camera(0.0, 0.0, 1.0);
        e
    }

    pub(crate) fn draw(
        e: &mut Engine<MockSink>,
        id: i32,
        kind: PointerKind,
        from: [f32; 2],
        to: [f32; 2],
    ) {
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

    /// Wet ink reaches the pen at any smoothing, and the dry stroke ends
    /// where the last wet frame did: pen-up moves nothing (docs/performance.md).
    #[test]
    fn wet_ink_reaches_the_pen_and_dry_matches_it() {
        let mut e = engine();
        e.set_smoothing(1.0);
        e.pointer_down(1, 100.0, 100.0, 0.5, 0.0, PointerKind::Pen, 0, 0.0);
        let s: Vec<f64> = (1..=40)
            .flat_map(|i| [100.0 + f64::from(i) * 6.0, 100.0, 0.5, f64::from(i) * 4.0])
            .collect();
        e.pointer_move(1, &s, &[], 1.0);
        let wet_tip = *e.sink.wet[0].points.last().unwrap();
        assert_eq!((wet_tip.x, wet_tip.y), (340.0, 100.0), "wet tip at the pen");
        e.pointer_up(1, 2.0);
        let dry = e.sink.strokes.values().next().unwrap();
        let end = dry.points.last().unwrap();
        assert_eq!((end.x, end.y, end.p), (wet_tip.x, wet_tip.y, wet_tip.p));
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
        // Whole-stroke erasing (the partial eraser: duster.rs).
        e.set_eraser_mode(EraserMode::Stroke);
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
    fn area_erase_is_one_undo_step_and_spares_boxes() {
        let mut e = engine();
        // Whole-stroke erasing (the partial eraser: duster.rs).
        e.set_eraser_mode(EraserMode::Stroke);
        draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [400.0, 100.0]);
        draw(&mut e, 1, PointerKind::Pen, [100.0, 200.0], [400.0, 200.0]);
        draw(&mut e, 1, PointerKind::Pen, [100.0, 600.0], [400.0, 600.0]);
        let id = e
            .add_object_html("<p>x</p>", "t", 200.0, 200.0, 0, 0.0)
            .unwrap();
        let (cx, cy) = {
            let g = item_geom(&e, &id);
            let s = screen_of(&e, &g, 0.5, 0.5);
            (s[0], s[1])
        };
        // A rub across the first two strokes, in two moves of a 30 px palm.
        assert_eq!(e.area_erase([250.0, 80.0], [250.0, 150.0], 30.0), 1);
        assert_eq!(e.area_erase([250.0, 150.0], [250.0, 220.0], 30.0), 1);
        // Rubbing over the object never erases it.
        assert_eq!(e.area_erase([cx, cy], [cx + 10.0, cy], 60.0), 0);
        assert_eq!(e.sink.strokes.len(), 1, "strokes vanish while rubbing");
        assert_eq!(e.area_erase_end(), 2);
        assert_eq!(e.area_erase_end(), 0, "ending twice is harmless");
        assert_eq!(
            e.lesson.items(&e.page.clone()).unwrap().len(),
            2,
            "one stroke and the object stay"
        );
        assert!(e.undo());
        assert_eq!(e.sink.strokes.len(), 3, "the whole rub is one undo step");
        // Read-only boards never erase.
        e.set_read_only(true);
        assert_eq!(e.area_erase([250.0, 80.0], [250.0, 220.0], 30.0), 0);
        assert_eq!(e.area_erase([f32::NAN, 0.0], [0.0, 0.0], 30.0), 0);
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
            [300.0, 400.0],
            [100.0, 400.0],
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
        // Autosaves reuse the readable outline for a minute; the doc is current.
        let outline_at = e.pkg.outline_at;
        e.set_object_step(&id, 3);
        let auto = e.autosave(20.0).unwrap();
        assert_eq!(e.pkg.outline_at, outline_at);
        let mut e3 = engine();
        e3.load(&auto).unwrap();
        let dom: serde_json::Value = serde_json::from_str(&e3.dom_items_json()).unwrap();
        assert_eq!(dom[0]["step"], 3);
        e.autosave(10.0 + OUTLINE_MAX_AGE_MS).unwrap();
        assert_eq!(e.pkg.outline_at, 10.0 + OUTLINE_MAX_AGE_MS);
        e.save(10.0 + OUTLINE_MAX_AGE_MS + 1.0).unwrap();
        assert_eq!(e.pkg.outline_at, 10.0 + OUTLINE_MAX_AGE_MS + 1.0);
    }

    pub(crate) fn select_rect(e: &mut Engine<MockSink>, a: [f32; 2], b: [f32; 2]) {
        e.set_tool(ToolKind::Lasso);
        e.pointer_down(9, a[0], a[1], 0.5, 0.0, PointerKind::Mouse, 0, 0.0);
        e.pointer_move(
            9,
            &[
                f64::from(b[0]),
                f64::from(a[1]),
                0.5,
                1.0,
                f64::from(b[0]),
                f64::from(b[1]),
                0.5,
                2.0,
                f64::from(a[0]),
                f64::from(b[1]),
                0.5,
                3.0,
            ],
            &[],
            0.0,
        );
        e.pointer_up(9, 0.0);
    }

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nnot-really-an-image";

    #[test]
    fn stroke_created_is_pen_down_time() {
        let mut e = engine();
        e.pointer_down(1, 100.0, 100.0, 0.5, 0.0, PointerKind::Pen, 0, 1_000.0);
        e.pointer_move(1, &[200.0, 120.0, 0.5, 400.0], &[], 1_400.0);
        e.pointer_up(1, 3_000.0);
        e.set_tool(ToolKind::Shape(ShapeKind::Rect));
        e.pointer_down(2, 100.0, 300.0, 0.5, 0.0, PointerKind::Mouse, 0, 5_000.0);
        e.pointer_move(2, &[300.0, 400.0, 0.5, 10.0], &[], 5_500.0);
        e.pointer_up(2, 6_000.0);
        let items = e.lesson.items(&e.page.clone()).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].created, 1_000.0, "pen-down, not pen-up");
        assert_eq!(items[1].created, 5_000.0, "shapes too");
        assert_eq!(e.scene.get(&items[0].id).unwrap().created, 1_000.0);
    }

    #[test]
    fn drawing_dismisses_a_leftover_selection() {
        let mut e = engine();
        e.add_image(PNG.to_vec(), "png", 200.0, 100.0, 1.0).unwrap();
        assert_eq!(e.selection().len(), 1, "a placed item is selected");
        draw(&mut e, 1, PointerKind::Pen, [10.0, 10.0], [60.0, 10.0]);
        assert!(e.selection().is_empty());
    }

    #[test]
    fn copy_paste_round_trips_into_another_lesson() {
        let mut a = engine();
        draw(
            &mut a,
            1,
            PointerKind::Mouse,
            [100.0, 100.0],
            [300.0, 140.0],
        );
        let obj = a
            .add_object_html("<h1>Quiz</h1>", "Quiz", 400.0, 300.0, 3, 5.0)
            .unwrap();
        a.set_object_step(&obj, 2);
        let img = a.add_image(PNG.to_vec(), "png", 200.0, 100.0, 6.0).unwrap();
        a.select_all();
        let ids = a.selection().to_vec();
        assert_eq!(ids.len(), 3);
        let json = a.export_items_json(&ids);
        assert!(json.starts_with(r#"{"rupok":"clipboard/v1""#));

        let mut b = engine();
        let pasted = b.import_items_json(&json, 24.0, 24.0, 100.0).unwrap();
        assert_eq!(pasted.len(), 3);
        assert_eq!(
            b.selection(),
            pasted.as_slice(),
            "pasted items are selected"
        );
        assert_eq!(b.sink.strokes.len(), 1);
        let src = a.lesson.items(&a.page.clone()).unwrap();
        let dst = b.lesson.items(&b.page.clone()).unwrap();
        assert_eq!(dst.len(), 3);
        for (s, d) in src.iter().zip(&dst) {
            let (sb, db) = (s.data.aabb(), d.data.aabb());
            assert!((db[0] - sb[0] - 24.0).abs() < 1e-3 && (db[1] - sb[1] - 24.0).abs() < 1e-3);
            assert_eq!(s.data.kind(), d.data.kind(), "stacking order kept");
        }
        match (&src[0].data, &dst[0].data) {
            (ItemData::Stroke(x), ItemData::Stroke(y)) => assert_eq!(x.points, y.points),
            _ => panic!("stroke first"),
        }
        let ItemData::Object(o) = &dst[1].data else {
            panic!()
        };
        assert_eq!(o.step, 2);
        assert_eq!(b.object_html(&o.bundle).unwrap(), "<h1>Quiz</h1>");
        let ItemData::Image(i) = &dst[2].data else {
            panic!()
        };
        assert_eq!(b.asset(&i.asset).unwrap(), PNG);
        assert!(dst.windows(2).all(|w| w[0].created < w[1].created));
        // One undo step removes the whole paste.
        assert!(b.undo());
        assert!(b.lesson.items(&b.page.clone()).unwrap().is_empty());
        // The pasted lesson saves and reloads with its bundle and asset.
        assert!(b.redo());
        let mut c = engine();
        c.load(&b.save(200.0).unwrap()).unwrap();
        assert_eq!(c.lesson.items(&c.page.clone()).unwrap().len(), 3);
        assert!(c.asset(&i.asset).is_some());
        let _ = img;
    }

    #[test]
    fn paste_shares_equal_bundles_and_renames_clashes() {
        let mut a = engine();
        let id = a
            .add_object_html("<p>one</p>", "Widget", 400.0, 300.0, 0, 1.0)
            .unwrap();
        let json = a.export_items_json(std::slice::from_ref(&id));
        // Same lesson: the equal bundle is reused.
        a.import_items_json(&json, 24.0, 24.0, 2.0).unwrap();
        assert_eq!(a.pkg.objects.len(), 1);
        // Another lesson already has a different "widget" bundle.
        let mut b = engine();
        b.add_object_html("<p>two</p>", "Widget", 400.0, 300.0, 0, 1.0)
            .unwrap();
        let ids = b.import_items_json(&json, 0.0, 0.0, 2.0).unwrap();
        assert_eq!(b.pkg.objects.len(), 2);
        let ItemData::Object(o) = b.lesson.item(&b.page.clone(), &ids[0]).unwrap().data else {
            panic!()
        };
        assert_eq!(o.bundle, "widget-2");
        assert_eq!(b.object_html("widget-2").unwrap(), "<p>one</p>");
        assert_eq!(b.object_html("widget").unwrap(), "<p>two</p>");
    }

    #[test]
    fn paste_rejects_bad_clipboards_without_panicking() {
        let mut e = engine();
        for bad in [
            "",
            "not json",
            "{}",
            r#"{"rupok":"clipboard/v9","items":[]}"#,
            r#"{"rupok":"clipboard/v1","items":[{"kind":"stroke","tool":"pen","color":1,"width":2,"opacity":1,"pts":"!!"}]}"#,
            r#"{"rupok":"clipboard/v1","items":[{"kind":"stroke","tool":"pen","color":1,"width":2,"opacity":1,"pts":"YWJj"}]}"#,
            r#"{"rupok":"clipboard/v1","items":[{"kind":"teleporter"}]}"#,
            r#"{"rupok":"clipboard/v1","items":[],"bundles":{"x":{"manifest":{"schema":1,"title":"x","size":{"w":1,"h":1}},"files":{"../evil":"YQ=="}}}}"#,
        ] {
            assert!(e.import_items_json(bad, 0.0, 0.0, 1.0).is_err(), "{bad}");
        }
        // A bundle that fails validation (remote script) is refused.
        let mut a = engine();
        let id = a
            .add_object_html("<p>ok</p>", "Ok", 400.0, 300.0, 0, 1.0)
            .unwrap();
        let json = a.export_items_json(&[id]).replace(
            &b64::encode(b"<p>ok</p>"),
            &b64::encode(br#"<script src="https://cdn.example.com/x.js"></script>"#),
        );
        assert!(e.import_items_json(&json, 0.0, 0.0, 1.0).is_err());
        assert!(e.lesson.items(&e.page.clone()).unwrap().is_empty());
        assert!(!e.can_undo_for_tests());
    }

    #[test]
    fn duplicate_items_offsets_and_selects_the_copies() {
        let mut e = engine();
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 100.0],
            [300.0, 100.0],
        );
        let obj = e
            .add_object_html("<p>x</p>", "X", 400.0, 300.0, 0, 1.0)
            .unwrap();
        let sel = vec![obj.clone()];
        let copies = e.duplicate_items(&sel, 24.0, 24.0, 10.0);
        assert_eq!(copies.len(), 1);
        assert_eq!(e.selection(), copies.as_slice());
        let (ItemData::Object(a), ItemData::Object(b)) = (
            e.lesson.item(&e.page.clone(), &obj).unwrap().data,
            e.lesson.item(&e.page.clone(), &copies[0]).unwrap().data,
        ) else {
            panic!()
        };
        assert_eq!((b.geom.x - a.geom.x, b.geom.y - a.geom.y), (24.0, 24.0));
        assert_eq!(a.bundle, b.bundle, "bundle shared, not copied");
        assert_eq!(e.pkg.objects.len(), 1);
        assert!(
            e.duplicate_items(&["missing".into()], 24.0, 24.0, 11.0)
                .is_empty()
        );
    }

    #[test]
    fn page_preview_decimates_strokes_and_lists_boxes() {
        let mut e = engine();
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 100.0],
            [600.0, 300.0],
        );
        e.add_image(PNG.to_vec(), "png", 200.0, 100.0, 1.0).unwrap();
        e.add_object_html("<p>x</p>", "Lab", 400.0, 300.0, 0, 2.0)
            .unwrap();
        let sid = e.lesson.items(&e.page.clone()).unwrap()[0].id.clone();
        e.move_items(&[sid], 10.0, 0.0);
        let v: serde_json::Value = serde_json::from_str(&e.page_preview_json(0, 4)).unwrap();
        assert_eq!(v["frame"]["w"], 1920.0);
        assert_eq!(v["bg"], 0xffffffffu32);
        let items = v["items"].as_array().unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0]["kind"], "stroke");
        let pts = items[0]["pts"].as_array().unwrap();
        assert!(
            pts.len() >= 4 && pts.len() <= 10,
            "decimated: {}",
            pts.len()
        );
        let ItemData::Stroke(s) = &e.lesson.items(&e.page.clone()).unwrap()[0].data else {
            panic!()
        };
        let first = s.points[0].x + s.offset[0];
        assert!(
            (pts[0].as_f64().unwrap() - f64::from(first)).abs() < 0.06,
            "offset applied"
        );
        let last = s.points.last().unwrap();
        let n = pts.len();
        assert!((pts[n - 2].as_f64().unwrap() - f64::from(last.x + s.offset[0])).abs() < 0.06);
        assert_eq!(items[1]["kind"], "image");
        assert!(items[1]["asset"].as_str().unwrap().ends_with(".png"));
        assert_eq!(items[1]["rect"][2], 200.0);
        assert_eq!(items[2]["kind"], "object");
        assert_eq!(items[2]["title"], "Lab");
        assert!(v["bounds"].is_array());
        assert_eq!(e.page_preview_json(7, 4), "null");
    }

    fn mp4() -> Vec<u8> {
        let mut v = vec![0, 0, 0, 0x18];
        v.extend_from_slice(b"ftypisom-some-video-bytes");
        v
    }

    #[test]
    fn media_items_add_save_copy_and_undo() {
        let mut e = engine();
        let yt = MediaData {
            kind: "youtube".into(),
            src: "https://youtu.be/dQw4w9WgXcQ".into(),
            title: "Plants".into(),
            start: Some(30.0),
            ..MediaData::default()
        };
        let a = e
            .add_media(yt, (PNG.to_vec(), "png"), None, 640.0, 360.0, 1.0)
            .unwrap();
        let clip = MediaData {
            kind: "video".into(),
            ..MediaData::default()
        };
        let b = e
            .add_media(
                clip.clone(),
                (PNG.to_vec(), "png"),
                Some((mp4(), "mp4")),
                640.0,
                360.0,
                2.0,
            )
            .unwrap();
        let dom: serde_json::Value = serde_json::from_str(&e.dom_items_json()).unwrap();
        assert_eq!(dom[0]["media"]["type"], "youtube");
        assert_eq!(
            dom[0]["media"]["src"], "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "stored canonical"
        );
        assert_eq!(dom[0]["media"]["site"], "youtube");
        let src = dom[1]["media"]["src"].as_str().unwrap().to_string();
        assert!(src.ends_with(".mp4") && e.asset(&src).is_some());
        // Links off the allowlist, and files that are not media, are refused.
        let bad = MediaData {
            kind: "web".into(),
            src: "https://evil.example/".into(),
            ..MediaData::default()
        };
        assert!(
            e.add_media(bad, (PNG.to_vec(), "png"), None, 9.0, 9.0, 3.0)
                .is_err()
        );
        assert!(
            e.add_media(
                clip.clone(),
                (PNG.to_vec(), "png"),
                Some((b"<html>".to_vec(), "mp4")),
                9.0,
                9.0,
                3.0
            )
            .is_err()
        );
        assert!(
            e.add_media(clip, (PNG.to_vec(), "png"), None, 9.0, 9.0, 3.0)
                .is_err()
        );
        // Settings change in one undo step and keep the clip.
        let mut m: MediaData = serde_json::from_value(dom[1]["media"].clone()).unwrap();
        m.looped = true;
        m.src = "ignored".into();
        e.set_media(&b, m, None, None).unwrap();
        let dom: serde_json::Value = serde_json::from_str(&e.dom_items_json()).unwrap();
        assert_eq!(dom[1]["media"]["loop"], true);
        assert_eq!(dom[1]["media"]["src"], src.as_str());
        assert!(e.undo());
        let dom: serde_json::Value = serde_json::from_str(&e.dom_items_json()).unwrap();
        assert!(dom[1]["media"].get("loop").is_none());
        // Save keeps the clip (it is referenced by `media.src`, not `asset`).
        let mut f = engine();
        f.load(&e.save(10.0).unwrap()).unwrap();
        assert!(f.asset(&src).is_some());
        // Copy and paste into another lesson brings the clip along.
        let clip_json = e.export_items_json(&[a.clone(), b.clone()]);
        let mut g = engine();
        let ids = g
            .import_items(Clip::from_json(&clip_json).unwrap(), 10.0, 10.0, 11.0)
            .unwrap();
        assert_eq!(ids.len(), 2);
        assert!(g.asset(&src).is_some());
    }

    #[test]
    fn undo_save_redo_keeps_bundles_and_clips() {
        // Regression: a save while an item was undone dropped its bundle (or
        // its video) from memory, so redo brought back a broken item.
        let mut e = engine();
        e.add_object_html("<p>x</p>", "X", 400.0, 300.0, 0, 1.0)
            .unwrap();
        let clip = MediaData {
            kind: "video".into(),
            ..MediaData::default()
        };
        e.add_media(
            clip,
            (PNG.to_vec(), "png"),
            Some((mp4(), "mp4")),
            64.0,
            36.0,
            2.0,
        )
        .unwrap();
        assert!(e.undo() && e.undo());
        let saved = e.save(3.0).unwrap();
        let mut f = engine();
        f.load(&saved).unwrap();
        assert!(
            f.pkg.objects.is_empty() && f.pkg.assets.is_empty(),
            "unused things are not saved"
        );
        assert!(e.redo() && e.redo());
        let mut g = engine();
        g.load(&e.save(4.0).unwrap()).unwrap();
        assert_eq!(g.object_html("x").as_deref(), Some("<p>x</p>"));
        assert_eq!(g.pkg.assets.len(), 2, "poster and clip");
    }

    #[test]
    fn model_becomes_a_viewer_object() {
        let mut e = engine();
        let mut glb = b"glTF".to_vec();
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&[0; 12]);
        let id = e
            .add_model(&glb, "cell.glb", "", 1280.0, 720.0, 1.0)
            .unwrap();
        let dom: serde_json::Value = serde_json::from_str(&e.dom_items_json()).unwrap();
        assert_eq!(dom[0]["id"], id.as_str());
        assert_eq!(dom[0]["title"], "Cell");
        let html = e.object_html(dom[0]["bundle"].as_str().unwrap()).unwrap();
        assert!(html.contains("GLTFLoader"));
        assert!(e.add_model(b"nope", "x.glb", "", 9.0, 9.0, 2.0).is_err());
    }

    #[test]
    fn duplicate_page_copies_items_and_keeps_bundles() {
        let mut e = engine();
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 100.0],
            [300.0, 100.0],
        );
        e.add_object_html("<p>x</p>", "X", 400.0, 300.0, 0, 1.0)
            .unwrap();
        e.add_image(PNG.to_vec(), "png", 200.0, 100.0, 2.0).unwrap();
        e.add_page(true, Pattern::Plain);
        assert_eq!(e.duplicate_page(0), Some(1));
        assert_eq!(e.page_index(), 1, "shows the copy");
        assert_eq!(e.lesson.page_count(), 3);
        assert_eq!(e.stroke_count(), 3);
        assert_eq!(e.sink.strokes.len(), 1);
        let dom: serde_json::Value = serde_json::from_str(&e.dom_items_json()).unwrap();
        assert_eq!(dom[0]["bundle"], "x");
        // Delete the original page: the copy still saves with its bundle and asset.
        e.goto_page(0);
        e.delete_page();
        let mut f = engine();
        f.load(&e.save(10.0).unwrap()).unwrap();
        assert_eq!(f.stroke_count(), 3);
        assert_eq!(f.object_html("x").unwrap(), "<p>x</p>");
        assert_eq!(f.pkg.assets.len(), 1);
        assert_eq!(e.duplicate_page(9), None);
    }

    #[test]
    fn read_only_rejects_every_edit_but_allows_viewing() {
        let mut e = engine();
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 100.0],
            [300.0, 100.0],
        );
        let obj = e
            .add_object_html("<p>x</p>", "X", 400.0, 300.0, 2, 1.0)
            .unwrap();
        e.add_page(true, Pattern::Plain);
        e.goto_page(0);
        let bytes = e.save(5.0).unwrap();
        let mut v = engine();
        v.set_read_only(true);
        assert!(v.read_only());
        v.load(&bytes).unwrap();
        let rev = v.lesson.version();
        // A loaded slide sits at fit, where there is nothing to pan to.
        v.zoom_at(2.0, 640.0, 360.0);
        let cam = v.camera;
        for tool in [
            ToolKind::Pen,
            ToolKind::Eraser,
            ToolKind::Shape(ShapeKind::Rect),
        ] {
            v.set_tool(tool);
            draw(
                &mut v,
                1,
                PointerKind::Mouse,
                [100.0, 100.0],
                [300.0, 100.0],
            );
        }
        assert_ne!(v.camera, cam, "dragging pans instead of drawing");
        assert_eq!(v.stroke_count(), 2);
        let all: Vec<ItemId> = v.scene.ordered().iter().map(|i| i.id.clone()).collect();
        v.select(all.clone());
        v.delete_selection();
        v.move_items(&all, 5.0, 5.0);
        v.bring_to_front_many(&all);
        v.set_object_step(&obj, 1);
        v.set_object_state(&obj, "{}");
        v.set_geom(&obj, BoxGeom::default());
        v.clear_page();
        v.set_background(Pattern::Grid, 0);
        v.set_title("hacked");
        v.add_page(true, Pattern::Plain);
        v.move_page(0, 1);
        v.delete_page();
        assert!(v.duplicate_page(0).is_none());
        assert!(v.add_text(10.0, 10.0, "x", 40.0, 1.0).is_none());
        assert!(v.add_image(PNG.to_vec(), "png", 10.0, 10.0, 1.0).is_none());
        assert!(
            v.add_object_html("<p>y</p>", "Y", 10.0, 10.0, 0, 1.0)
                .is_err()
        );
        let json = e.export_items_json(&all);
        assert!(v.import_items_json(&json, 0.0, 0.0, 1.0).is_err());
        assert!(v.duplicate_items(&all, 1.0, 1.0, 1.0).is_empty());
        assert!(!v.undo() && !v.redo());
        assert_eq!(v.lesson.version(), rev, "the document is untouched");
        assert_eq!(v.lesson.page_count(), 2);
        // Viewing still works.
        assert!(v.goto_page(1));
        assert!(v.goto_page(0));
        let d = v.replay_start();
        assert!(d >= 0.0);
        v.replay_seek(d, 0.0);
        assert_eq!(v.sink.strokes.len(), 1);
        v.replay_stop();
        v.set_read_only(false);
        draw(
            &mut v,
            1,
            PointerKind::Mouse,
            [100.0, 300.0],
            [300.0, 300.0],
        );
        assert_eq!(v.stroke_count(), 3);
    }

    #[test]
    fn bring_to_front_many_and_resize_text() {
        let mut e = engine();
        let t = e.add_text(10.0, 10.0, "Hi", 40.0, 1.0).unwrap();
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 100.0],
            [300.0, 100.0],
        );
        e.bring_to_front_many(std::slice::from_ref(&t));
        let top = e.scene.ordered().last().unwrap().id.clone();
        assert_eq!(top, t);
        e.resize_text(
            &t,
            BoxGeom {
                x: 1.0,
                y: 2.0,
                w: 900.0,
                h: 90.0,
                rot: 0.0,
            },
            60.0,
        );
        let dom: serde_json::Value = serde_json::from_str(&e.dom_items_json()).unwrap();
        assert_eq!(dom[0]["size"], 60.0);
        assert_eq!(dom[0]["w"], 900.0);
        e.resize_text(&t, BoxGeom::default(), f64::NAN);
        let dom: serde_json::Value = serde_json::from_str(&e.dom_items_json()).unwrap();
        assert_eq!(dom[0]["size"], 60.0, "invalid size ignored");
    }

    #[test]
    fn lasso_selection_exports_strokes_only_once() {
        let mut e = engine();
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 100.0],
            [200.0, 100.0],
        );
        select_rect(&mut e, [50.0, 50.0], [250.0, 150.0]);
        let mut ids = e.selection().to_vec();
        ids.push(ids[0].clone());
        let clip = e.export_items(&ids);
        assert_eq!(clip.items.len(), 1);
        // The point codec quantises once; after that the JSON is stable.
        let once = Clip::from_json(&clip.to_json()).unwrap();
        assert_eq!(Clip::from_json(&once.to_json()).unwrap(), once);
        assert_eq!(once.bounds, clip.bounds);
    }

    fn item_geom(e: &Engine<MockSink>, id: &str) -> BoxGeom {
        match e.lesson.item(&e.page.clone(), id).unwrap().data {
            ItemData::Object(o) => o.geom,
            ItemData::Image(i) => i.geom,
            ItemData::Text(t) => t.geom,
            ItemData::Stroke(_) => panic!("not a box"),
        }
    }

    fn screen_of(e: &Engine<MockSink>, g: &BoxGeom, fx: f64, fy: f64) -> [f32; 2] {
        let (x, y) = e
            .camera
            .world_to_screen((g.x + g.w * fx) as f32, (g.y + g.h * fy) as f32);
        [x, y]
    }

    #[test]
    fn press_and_drag_selects_and_moves_a_box_in_one_gesture() {
        for kind in [PointerKind::Mouse, PointerKind::Pen, PointerKind::Touch] {
            let mut e = engine();
            // A pen was used before: palm rejection must not stop a finger in Select.
            draw(&mut e, 1, PointerKind::Pen, [20.0, 20.0], [60.0, 20.0]);
            let id = e
                .add_object_html("<p>x</p>", "X", 400.0, 300.0, 0, 1.0)
                .unwrap();
            e.select(vec![]);
            e.set_tool(ToolKind::Lasso);
            let g0 = item_geom(&e, &id);
            let p = screen_of(&e, &g0, 0.5, 0.5);
            draw(&mut e, 2, kind, p, [p[0] + 100.0, p[1] + 40.0]);
            assert_eq!(
                e.selection(),
                std::slice::from_ref(&id),
                "{kind:?}: selected"
            );
            let g1 = item_geom(&e, &id);
            let z = f64::from(e.camera.zoom);
            assert!((g1.x - g0.x - 100.0 / z).abs() < 0.5, "{kind:?}: moved");
            assert!((g1.y - g0.y - 40.0 / z).abs() < 0.5);
            assert!(e.undo(), "one move is one undo step");
            assert_eq!(item_geom(&e, &id), g0);
            // A plain click selects too.
            e.select(vec![]);
            e.pointer_down(3, p[0], p[1], 0.5, 0.0, kind, 0, 0.0);
            e.pointer_up(3, 0.0);
            assert_eq!(e.selection(), std::slice::from_ref(&id));
            assert_eq!(item_geom(&e, &id), g0, "a click does not move");
        }
    }

    #[test]
    fn page_sized_boxes_still_let_a_lasso_start_on_them() {
        let mut e = engine();
        let bg = e
            .add_image(PNG.to_vec(), "png", 1920.0, 1080.0, 1.0)
            .unwrap();
        e.move_items(
            std::slice::from_ref(&bg),
            -item_geom(&e, &bg).x,
            -item_geom(&e, &bg).y,
        );
        let s = {
            draw(
                &mut e,
                1,
                PointerKind::Mouse,
                [500.0, 300.0],
                [600.0, 300.0],
            );
            e.lesson.items(&e.page.clone()).unwrap()[1].id.clone()
        };
        e.select(vec![]);
        select_rect(&mut e, [450.0, 250.0], [650.0, 350.0]);
        assert_eq!(
            e.selection(),
            std::slice::from_ref(&s),
            "lasso over a PDF slide"
        );
        assert_eq!(item_geom(&e, &bg).x, 0.0, "the slide did not move");
        e.select(vec![]);
        e.pointer_down(1, 300.0, 600.0, 0.5, 0.0, PointerKind::Mouse, 0, 0.0);
        e.pointer_up(1, 0.0);
        assert_eq!(
            e.selection(),
            std::slice::from_ref(&bg),
            "a click selects the slide"
        );
    }

    #[test]
    fn a_second_finger_turns_a_gesture_into_a_pinch() {
        let mut e = engine();
        e.pointer_down(1, 300.0, 300.0, 0.5, 0.0, PointerKind::Touch, 0, 0.0);
        e.pointer_move(1, &[320.0, 300.0, 0.5, 1.0], &[], 0.0);
        e.pointer_down(2, 600.0, 300.0, 0.5, 0.0, PointerKind::Touch, 0, 0.0);
        let z0 = e.camera.zoom;
        e.pointer_move(2, &[700.0, 300.0, 0.5, 2.0], &[], 0.0);
        e.pointer_move(2, &[800.0, 300.0, 0.5, 3.0], &[], 0.0);
        e.pointer_up(1, 0.0);
        e.pointer_up(2, 0.0);
        assert!(e.camera.zoom > z0, "pinch zooms");
        assert_eq!(e.stroke_count(), 0, "the half-drawn stroke is dropped");
        assert!(e.sink.wet.is_empty());
    }

    #[test]
    fn groups_select_move_delete_and_ungroup_together() {
        let mut e = engine();
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 100.0],
            [200.0, 100.0],
        );
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 400.0],
            [200.0, 400.0],
        );
        let obj = e
            .add_object_html("<p>x</p>", "X", 200.0, 100.0, 0, 1.0)
            .unwrap();
        let all: Vec<ItemId> = e.scene.ordered().iter().map(|i| i.id.clone()).collect();
        // Group the first stroke and the object.
        e.select(vec![all[0].clone(), obj.clone()]);
        let g = e.group_selection().unwrap();
        let meta: serde_json::Value = serde_json::from_str(&e.selection_meta_json()).unwrap();
        assert_eq!(meta["grouped"], true);
        assert_eq!(
            (meta["strokes"].as_u64(), meta["boxes"].as_u64()),
            (Some(1), Some(1))
        );
        // A click on one member selects the whole group.
        e.select(vec![]);
        e.set_tool(ToolKind::Lasso);
        e.pointer_down(1, 150.0, 100.0, 0.5, 0.0, PointerKind::Mouse, 0, 0.0);
        e.pointer_up(1, 0.0);
        let mut sel = e.selection().to_vec();
        sel.sort();
        let mut want = vec![all[0].clone(), obj.clone()];
        want.sort();
        assert_eq!(sel, want);
        // Dragging a member moves the group.
        let g0 = item_geom(&e, &obj);
        draw(
            &mut e,
            2,
            PointerKind::Mouse,
            [150.0, 100.0],
            [250.0, 100.0],
        );
        assert!(
            item_geom(&e, &obj).x > g0.x,
            "the object moved with the stroke"
        );
        // A lasso around part of a group takes the whole group.
        e.select(vec![]);
        select_rect(&mut e, [180.0, 80.0], [320.0, 130.0]);
        assert_eq!(
            e.selection().len(),
            2,
            "the stroke (moved +100) pulls in the object"
        );
        // Copies form a new group of their own.
        let sel = e.selection().to_vec();
        let copies = e.duplicate_items(&sel, 24.0, 24.0, 5.0);
        let cg = e.scene.get(&copies[0]).unwrap().group.clone().unwrap();
        assert_ne!(cg, g);
        assert!(
            copies
                .iter()
                .all(|c| e.scene.get(c).unwrap().group.as_deref() == Some(&cg))
        );
        // Ungroup: members select alone again.
        e.select(vec![obj.clone()]);
        assert_eq!(e.selection().len(), 2);
        assert_eq!(e.ungroup_selection(), 1);
        e.select(vec![obj.clone()]);
        assert_eq!(e.selection(), std::slice::from_ref(&obj));
        // Delete takes a whole group.
        e.select(vec![copies[0].clone()]);
        e.delete_selection();
        assert!(copies.iter().all(|c| e.scene.get(c).is_none()));
        assert!(e.group_selection().is_none(), "needs two or more items");
    }

    #[test]
    fn merge_strokes_moves_and_erases_as_one_and_mixed_merge_groups() {
        let mut e = engine();
        // Whole-stroke erasing (the partial eraser: duster.rs).
        e.set_eraser_mode(EraserMode::Stroke);
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 100.0],
            [300.0, 100.0],
        );
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 200.0],
            [300.0, 200.0],
        );
        draw(
            &mut e,
            1,
            PointerKind::Mouse,
            [100.0, 500.0],
            [300.0, 500.0],
        );
        select_rect(&mut e, [50.0, 50.0], [350.0, 250.0]);
        assert_eq!(e.selection().len(), 2);
        assert_eq!(e.merge_selection(), "merged");
        e.set_tool(ToolKind::Eraser);
        draw(&mut e, 2, PointerKind::Mouse, [200.0, 80.0], [200.0, 120.0]);
        assert_eq!(e.stroke_count(), 1, "erasing one merged stroke erases both");
        assert!(e.undo());
        assert_eq!(e.stroke_count(), 3);
        // Mixed selection: grouped instead.
        e.add_object_html("<p>x</p>", "X", 200.0, 100.0, 0, 1.0)
            .unwrap();
        e.select_all();
        assert_eq!(e.merge_selection(), "grouped");
        e.select(vec![]);
        assert_eq!(e.merge_selection(), "none");
    }

    /// Screen rectangle of the 1920 × 1080 slide: [left, top, right, bottom].
    fn slide_on_screen(e: &Engine<MockSink>) -> [f32; 4] {
        let (l, t) = e.camera.world_to_screen(0.0, 0.0);
        let (r, b) = e.camera.world_to_screen(1920.0, 1080.0);
        [l, t, r, b]
    }

    #[test]
    fn a_slide_never_zooms_out_past_fit_nor_in_past_five_times() {
        let mut e = engine();
        e.set_insets(60.0, 12.0, 72.0, 200.0);
        e.fit_page();
        let fit = e.camera;
        for _ in 0..200 {
            e.zoom_at(0.8, 10.0, 10.0);
        }
        assert!((e.camera.zoom - fit.zoom).abs() < 1e-4, "stops at fit");
        assert!((e.camera.x - fit.x).abs() < 0.01 && (e.camera.y - fit.y).abs() < 0.01);
        for _ in 0..200 {
            e.zoom_at(1.25, 900.0, 400.0);
        }
        assert!((e.camera.zoom / e.fit_zoom() - MAX_ZOOM_OVER_FIT).abs() < 1e-3);
        // A portrait page fits by its height, and its own fit is the floor.
        let spec = |frame| PageSpec {
            frame,
            ..Default::default()
        };
        let tall = Some(Frame {
            w: 600.0,
            h: 1000.0,
        });
        e.lesson.add_page(None, spec(tall)).unwrap();
        e.goto_page(1);
        let fit = e.camera.zoom;
        e.zoom_at(0.5, 10.0, 10.0);
        assert!((e.camera.zoom - fit).abs() < 1e-4);
        let (_, top) = e.camera.world_to_screen(0.0, 0.0);
        assert!(top > 60.0, "the tall page sits below the header");
        // An infinite board keeps the wide range.
        e.lesson.add_page(None, spec(None)).unwrap();
        e.goto_page(2);
        for _ in 0..200 {
            e.zoom_at(0.8, 10.0, 10.0);
        }
        assert!((e.camera.zoom - MIN_ZOOM).abs() < 1e-4);
        e.pan_by(-1e5, 0.0);
        assert!(e.camera.x > 1e5, "and pans freely");
    }

    #[test]
    fn pan_keeps_the_slide_on_screen() {
        let mut e = engine();
        let [t, r, b, l] = [60.0, 12.0, 72.0, 200.0];
        e.set_insets(t, r, b, l);
        e.fit_page();
        let fit = e.camera;
        e.pan_by(400.0, -300.0);
        assert_eq!(e.camera, fit, "at fit the slide stays centred");
        e.zoom_at(3.0, 700.0, 400.0);
        for (dx, dy) in [(5000.0, 0.0), (-5000.0, 0.0), (0.0, 5000.0), (0.0, -5000.0)] {
            e.pan_by(dx, dy);
            let [sl, st, sr, sb] = slide_on_screen(&e);
            assert!(
                sl <= l + 0.01 && st <= t + 0.01,
                "no gap left/top: {sl} {st}"
            );
            assert!(
                sr >= 1280.0 - r - 0.01 && sb >= 720.0 - b - 0.01,
                "no gap right/bottom"
            );
        }
        // Panning still moves the slide inside those bounds.
        let before = e.camera;
        e.pan_by(30.0, 0.0);
        assert_ne!(e.camera, before);
    }

    #[test]
    fn a_pinch_below_fit_springs_back_and_resizing_clamps() {
        let mut e = engine();
        e.touch_mode = TouchMode::Navigate;
        e.fit_page();
        let fit = e.camera.zoom;
        e.pointer_down(1, 400.0, 360.0, 0.5, 0.0, PointerKind::Touch, 0, 0.0);
        e.pointer_down(2, 880.0, 360.0, 0.5, 0.0, PointerKind::Touch, 0, 0.0);
        e.pointer_move(1, &[560.0, 360.0, 0.5, 1.0], &[], 10.0);
        e.pointer_move(2, &[720.0, 360.0, 0.5, 2.0], &[], 10.0);
        assert!(e.camera.zoom < fit && e.camera.zoom >= fit * PINCH_UNDER_FIT - 1e-4);
        e.pointer_up(1, 20.0);
        e.pointer_up(2, 20.0);
        e.tick(20.0 + SETTLE_MS / 2.0);
        assert!(e.camera.zoom < fit, "animates");
        e.tick(20.0 + SETTLE_MS);
        assert!((e.camera.zoom - fit).abs() < 1e-4, "back at fit");
        assert!(!e.has_animation());
        // Reduced motion: no animation.
        e.set_reduced_motion(true);
        e.zoom_at(1.0, 0.0, 0.0);
        e.pointer_down(1, 400.0, 360.0, 0.5, 0.0, PointerKind::Touch, 0, 30.0);
        e.pointer_down(2, 880.0, 360.0, 0.5, 0.0, PointerKind::Touch, 0, 30.0);
        e.pointer_move(1, &[560.0, 360.0, 0.5, 31.0], &[], 40.0);
        e.pointer_up(1, 50.0);
        e.pointer_up(2, 50.0);
        assert!((e.camera.zoom - fit).abs() < 1e-4);
        // A bigger window never leaves the slide below its new fit.
        e.set_view(2560.0, 1440.0);
        assert!((e.camera.zoom - e.fit_zoom()).abs() < 1e-4);
        let [l, t, r, b] = slide_on_screen(&e);
        assert!(
            (l + r - 2560.0).abs() < 0.1 && (t + b - 1440.0).abs() < 0.1,
            "centred"
        );
    }

    #[test]
    fn fit_margin_zero_makes_the_slide_touch_the_screen() {
        let mut e = engine();
        e.set_view(1920.0, 1080.0);
        e.fit_page();
        assert!(e.camera.zoom < 0.98, "default fit keeps a margin");
        e.set_fit_margin(0.0);
        e.fit_page();
        let c = e.camera;
        assert!((c.zoom - 1.0).abs() < 1e-6 && c.x.abs() < 1e-3 && c.y.abs() < 1e-3);
        // A 16:10 window: the width limits, the page is centred vertically.
        e.set_view(1280.0, 800.0);
        e.fit_page();
        let (l, t) = e.camera.world_to_screen(0.0, 0.0);
        let (r, b) = e.camera.world_to_screen(1920.0, 1080.0);
        assert!(
            l.abs() < 0.01 && (r - 1280.0).abs() < 0.01,
            "touches left and right"
        );
        assert!(
            (t - 40.0).abs() < 0.01 && (800.0 - b - 40.0).abs() < 0.01,
            "even letterbox"
        );
        // Page changes keep the margin; the explicit form ignores it.
        e.add_page(true, Pattern::Plain);
        assert!((e.camera.world_to_screen(1920.0, 0.0).0 - 1280.0).abs() < 0.01);
        e.fit_page_margin(0.03);
        assert!(e.camera.world_to_screen(0.0, 0.0).0 > 10.0);
        e.set_fit_margin(f32::NAN);
        e.fit_page();
        assert!(
            e.camera.world_to_screen(0.0, 0.0).0 > 10.0,
            "NaN falls back to the default"
        );
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
