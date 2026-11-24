//! Plain data types mirrored to/from the Loro document.

use rupok_ink::{Aabb, StrokePoint};
use serde::{Deserialize, Serialize};

pub type ItemId = String;
pub type PageId = String;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Pattern {
    #[default]
    Plain,
    Grid,
    Dots,
    Lines,
}

impl Pattern {
    pub fn as_str(self) -> &'static str {
        match self {
            Pattern::Plain => "plain",
            Pattern::Grid => "grid",
            Pattern::Dots => "dots",
            Pattern::Lines => "lines",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "grid" => Pattern::Grid,
            "dots" => Pattern::Dots,
            "lines" => Pattern::Lines,
            _ => Pattern::Plain,
        }
    }
}

/// Slide bounds. `None` on a page means a pure infinite board.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub w: f64,
    pub h: f64,
}

impl Frame {
    pub const SLIDE_16_9: Frame = Frame {
        w: 1920.0,
        h: 1080.0,
    };
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageInfo {
    pub id: PageId,
    pub title: String,
    pub frame: Option<Frame>,
    pub pattern: Pattern,
    /// 0xRRGGBBAA
    pub bg_color: u32,
    pub item_count: usize,
    /// The page's background image (the optional `background` key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<PageBackground>,
    /// This page's own branding (the optional `branding` key), which
    /// replaces the lesson's; see [`crate::Lesson::page_branding`] for the
    /// branding a page actually shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branding: Option<Branding>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageSpec {
    pub title: String,
    pub frame: Option<Frame>,
    pub pattern: Pattern,
    pub bg_color: u32,
    #[serde(default)]
    pub background: Option<PageBackground>,
    #[serde(default)]
    pub branding: Option<Branding>,
}

impl Default for PageSpec {
    fn default() -> Self {
        Self {
            title: String::new(),
            frame: Some(Frame::SLIDE_16_9),
            pattern: Pattern::Plain,
            bg_color: 0xffffffff,
            background: None,
            branding: None,
        }
    }
}

/// The look of a page: colour, pattern and background image. A new page
/// copies it from the page it is added after ([`crate::Lesson::add_page_like`]),
/// and "Use for all pages" writes it to every page
/// ([`crate::Lesson::set_page_style`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageStyle {
    pub pattern: Pattern,
    /// 0xRRGGBBAA
    pub bg_color: u32,
    #[serde(default)]
    pub background: Option<PageBackground>,
}

impl Default for PageStyle {
    fn default() -> Self {
        Self {
            pattern: Pattern::Plain,
            bg_color: 0xffffffff,
            background: None,
        }
    }
}

/// Longest branding text a reader keeps (characters, per field).
pub const MAX_BRAND_TEXT: usize = 120;
/// Longest asset name a reader accepts in `background` and `branding`.
pub const MAX_ASSET_NAME: usize = 128;

/// How a background image covers the page (the `fit` key).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BgFit {
    /// The whole image, as large as fits (letterboxed in the page colour).
    #[default]
    Fit,
    /// Covers the page, cropped to it.
    Fill,
    /// Exactly the page, ignoring the image's aspect ratio.
    Stretch,
    /// Repeated at its natural size (one image pixel = one world unit).
    Tile,
    /// Centred at its natural size.
    Center,
}

impl BgFit {
    pub fn as_str(self) -> &'static str {
        match self {
            BgFit::Fit => "fit",
            BgFit::Fill => "fill",
            BgFit::Stretch => "stretch",
            BgFit::Tile => "tile",
            BgFit::Center => "center",
        }
    }
    /// Unknown values read as `fit`.
    pub fn parse(s: &str) -> Self {
        match s {
            "fill" => BgFit::Fill,
            "stretch" => BgFit::Stretch,
            "tile" => BgFit::Tile,
            "center" => BgFit::Center,
            _ => BgFit::Fit,
        }
    }
}

/// A page's background image, drawn over the page colour and pattern and
/// under everything else (the optional page key `background`; spec
/// `rupok-format.md`, "Page design").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageBackground {
    /// Asset name `<blake3>.<ext>` (the bytes are `assets/<image>`).
    pub image: String,
    #[serde(default)]
    pub fit: BgFit,
    /// 0..1, default 1.
    #[serde(default = "one")]
    pub opacity: f64,
    /// Dim for readability: a wash of the page colour at 50 % over the image.
    #[serde(default)]
    pub dim: bool,
}

fn one() -> f64 {
    1.0
}

impl PageBackground {
    /// Clamp numbers into range; `None` when there is no usable image.
    pub fn sanitized(mut self) -> Option<Self> {
        if !valid_asset_name(&self.image) {
            return None;
        }
        self.opacity = unit(self.opacity, 1.0);
        Some(self)
    }
}

/// Where the branding text line sits (the `text_pos` key).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BrandPos {
    Top,
    #[default]
    Bottom,
}

/// How the branding text line is aligned (the `text_align` key).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BrandAlign {
    Left,
    #[default]
    Center,
    Right,
}

/// A school logo and a line of text (school, teacher, class, date) drawn
/// on pages behind the ink. Not an item: it cannot be selected or erased,
/// and only Page design edits it. Stored for the whole lesson in
/// `meta.branding`, and per page in the page's `branding` key, which
/// replaces the lesson's on that page (`off` hides it there).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Branding {
    /// Logo asset name `<blake3>.<ext>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    /// Logo box: top-left corner and width as fractions of the page frame
    /// (0..1); the height follows the image's aspect ratio.
    #[serde(default = "Branding::default_x")]
    pub logo_x: f64,
    #[serde(default = "Branding::default_y")]
    pub logo_y: f64,
    #[serde(default = "Branding::default_w")]
    pub logo_w: f64,
    #[serde(default = "one")]
    pub logo_opacity: f64,
    #[serde(default)]
    pub school: String,
    #[serde(default)]
    pub teacher: String,
    #[serde(default)]
    pub class: String,
    /// Show the date of the day the page is shown (or exported).
    #[serde(default)]
    pub date: bool,
    #[serde(default)]
    pub text_pos: BrandPos,
    #[serde(default)]
    pub text_align: BrandAlign,
    /// Page key only: no branding on this page.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub off: bool,
}

impl Default for Branding {
    fn default() -> Self {
        Self {
            logo: None,
            logo_x: Self::default_x(),
            logo_y: Self::default_y(),
            logo_w: Self::default_w(),
            logo_opacity: 1.0,
            school: String::new(),
            teacher: String::new(),
            class: String::new(),
            date: false,
            text_pos: BrandPos::Bottom,
            text_align: BrandAlign::Center,
            off: false,
        }
    }
}

impl Branding {
    pub const MIN_LOGO_W: f64 = 0.02;
    pub const MAX_LOGO_W: f64 = 0.6;
    fn default_x() -> f64 {
        0.025
    }
    fn default_y() -> f64 {
        0.035
    }
    fn default_w() -> f64 {
        0.1
    }
    /// The "hide branding on this page" marker.
    pub fn hidden() -> Self {
        Self {
            off: true,
            ..Self::default()
        }
    }
    /// Clamp numbers into range, cap the text, drop a bad logo name.
    pub fn sanitized(mut self) -> Self {
        if self.logo.as_deref().is_some_and(|l| !valid_asset_name(l)) {
            self.logo = None;
        }
        self.logo_w = if self.logo_w.is_finite() {
            self.logo_w.clamp(Self::MIN_LOGO_W, Self::MAX_LOGO_W)
        } else {
            Self::default_w()
        };
        self.logo_x = unit(self.logo_x, Self::default_x());
        self.logo_y = unit(self.logo_y, Self::default_y());
        self.logo_opacity = unit(self.logo_opacity, 1.0);
        for t in [&mut self.school, &mut self.teacher, &mut self.class] {
            *t = cap_chars(t.trim(), MAX_BRAND_TEXT);
        }
        self
    }
    /// The text parts that are set, in reading order (without the date).
    pub fn text_parts(&self) -> Vec<&str> {
        [&self.school, &self.teacher, &self.class]
            .into_iter()
            .map(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .collect()
    }
    /// True when it draws nothing (no logo, no text, no date).
    pub fn is_empty(&self) -> bool {
        self.off || (self.logo.is_none() && self.text_parts().is_empty() && !self.date)
    }
}

/// An asset name as items use it: 1–128 bytes, one path segment, no
/// control characters.
pub fn valid_asset_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_ASSET_NAME
        && !s.contains(['/', '\\'])
        && s != "."
        && s != ".."
        && !s.chars().any(char::is_control)
}

fn unit(v: f64, default: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        default
    }
}

fn cap_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Position/size for box-shaped items (world units).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct BoxGeom {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default)]
    pub rot: f64,
}

impl BoxGeom {
    pub fn aabb(&self) -> Aabb {
        // Rotation-aware AABB.
        let (s, c) = self.rot.sin_cos();
        let (hw, hh) = (self.w / 2.0, self.h / 2.0);
        let (cx, cy) = (self.x + hw, self.y + hh);
        let ex = (hw * c).abs() + (hh * s).abs();
        let ey = (hw * s).abs() + (hh * c).abs();
        [
            (cx - ex) as f32,
            (cy - ey) as f32,
            (cx + ex) as f32,
            (cy + ey) as f32,
        ]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrokeData {
    pub tool: String,
    /// 0xRRGGBBAA
    pub color: u32,
    pub width: f32,
    pub opacity: f32,
    pub points: Vec<StrokePoint>,
    /// Translation applied at render time (moving a stroke never rewrites points).
    pub offset: [f32; 2],
    /// Set when produced by a shape tool or Smart Ink ("rect", "ellipse",
    /// "line", "arrow", "circle", "triangle", …).
    pub shape: Option<String>,
    /// Editable geometry of a shape stroke (the optional `geom` key), so the
    /// shape can be resized by regenerating `points` at a constant width.
    pub geom: Option<ShapeGeom>,
}

/// Geometry of a shape stroke, in the same coordinate frame as its points
/// (the rendered shape is moved by the stroke's offset like the points).
///
/// - `rect`, `ellipse`: the unrotated box (`x`, `y` top-left, `w`, `h`)
///   rotated by `rot` radians about its centre.
/// - `line`, `arrow`: the segment from (`x`, `y`) to (`x + w`, `y + h`);
///   `w`/`h` may be negative; for arrows the second end is the tip.
/// - `poly`: a closed polyline through `v` (flat `[x0, y0, x1, y1, …]`,
///   normalised to the box, 0..1), rotated by `rot` about the box centre.
/// - Smart Ink templates (`axes`, `numberline`, `gate-and`, …) use the box
///   the same way, with kind-specific numbers in `v` (see
///   `spec/rupok-format.md`, Shape geometry).
///
/// `from` / `to` link a connector (`line`, `arrow`) to the items its ends
/// are attached to, so it follows them when they move.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShapeGeom {
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default)]
    pub rot: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub v: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

impl ShapeGeom {
    /// Longest vertex list a reader accepts (pairs).
    pub const MAX_VERTICES: usize = 64;

    /// Finite numbers, a known size, sane vertex list: anything else is
    /// treated as absent (the stroke still draws from its points).
    pub fn is_valid(&self) -> bool {
        let nums = [self.x, self.y, self.w, self.h, self.rot];
        nums.iter().all(|v| v.is_finite() && v.abs() < 1e9)
            && !self.kind.is_empty()
            && self.kind.len() <= 16
            && self.v.len().is_multiple_of(2)
            && self.v.len() <= 2 * Self::MAX_VERTICES
            && self.v.iter().all(|v| v.is_finite() && v.abs() <= 16.0)
            && (self.kind != "poly" || self.v.len() >= 6)
            && [&self.from, &self.to].iter().all(|l| {
                l.as_ref()
                    .is_none_or(|id| !id.is_empty() && id.len() <= 256)
            })
    }
}

impl StrokeData {
    pub fn max_radius(&self) -> f32 {
        self.points
            .iter()
            .map(|p| rupok_ink::point_radius(p.p, self.width))
            .fold(0.0, f32::max)
    }
    pub fn aabb(&self) -> Aabb {
        let b = rupok_ink::bounds(&self.points, self.max_radius());
        [
            b[0] + self.offset[0],
            b[1] + self.offset[1],
            b[2] + self.offset[0],
            b[3] + self.offset[1],
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextData {
    pub geom: BoxGeom,
    pub text: String,
    pub size: f64,
    pub color: u32,
    /// `text` is LaTeX maths, shown typeset (the optional `latex` key).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub latex: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageData {
    pub geom: BoxGeom,
    /// Content address: `<blake3-hex>.<ext>` inside the package `assets/`.
    pub asset: String,
}

/// A live Smart Object placed on a page. The bundle itself lives in the
/// package under `objects/<bundle>/`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectData {
    pub geom: BoxGeom,
    pub bundle: String,
    pub title: String,
    /// Current build step (presenter clicker position inside the object).
    pub step: u32,
    /// Opaque JSON saved by the object through `rupok.state.save()`.
    pub state: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ItemData {
    Stroke(StrokeData),
    Text(TextData),
    Image(ImageData),
    Object(ObjectData),
}

impl ItemData {
    pub fn kind(&self) -> &'static str {
        match self {
            ItemData::Stroke(_) => "stroke",
            ItemData::Text(_) => "text",
            ItemData::Image(_) => "image",
            ItemData::Object(_) => "object",
        }
    }
    pub fn aabb(&self) -> Aabb {
        match self {
            ItemData::Stroke(s) => s.aabb(),
            ItemData::Text(t) => t.geom.aabb(),
            ItemData::Image(i) => i.geom.aabb(),
            ItemData::Object(o) => o.geom.aabb(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: ItemId,
    /// Stacking order; ties broken by id so every replica agrees.
    pub z: f64,
    /// Creation wall-clock time (ms since Unix epoch) — drives replay order.
    pub created: f64,
    /// Group id: items sharing one select, move, copy and delete together.
    /// Optional `group` key in the document; opaque, unique within a page.
    pub group: Option<String>,
    pub data: ItemData,
}

/// Several document edits applied as **one** change (one undo step) with
/// [`crate::Lesson::apply_edits`]: a Smart Ink composite (strokes rewritten,
/// raw ink removed, labels added in a group), a move that drags attached
/// connectors along, or a resize of a whole group.
#[derive(Clone, Debug, Default)]
pub struct Edits {
    /// Move items by (dx, dy) (strokes accumulate an offset).
    pub translate: Vec<(ItemId, f64, f64)>,
    /// Rewrite strokes in place (see [`crate::Lesson::edit_strokes`]).
    pub strokes: Vec<(ItemId, StrokeData)>,
    /// New geometry (and font size, for text) of box items.
    pub boxes: Vec<(ItemId, BoxGeom, Option<f64>)>,
    /// Set (`Some`) or clear (`None`) items' group.
    pub groups: Vec<(ItemId, Option<String>)>,
    pub remove: Vec<ItemId>,
    /// Added on top, in order (unless an item sets its own `z`).
    pub add: Vec<NewItem>,
    /// Recolour text items (0xRRGGBBAA); other kinds are skipped.
    pub text_colors: Vec<(ItemId, u32)>,
}

impl Edits {
    pub fn is_empty(&self) -> bool {
        self.translate.is_empty()
            && self.strokes.is_empty()
            && self.boxes.is_empty()
            && self.groups.is_empty()
            && self.remove.is_empty()
            && self.add.is_empty()
            && self.text_colors.is_empty()
    }
}

/// An item to add with [`crate::Lesson::add_items`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewItem {
    pub data: ItemData,
    pub created: f64,
    pub group: Option<String>,
    /// Stacking order to keep (a piece of a cut stroke stays where the
    /// stroke was); `None` puts the item on top.
    pub z: Option<f64>,
}
