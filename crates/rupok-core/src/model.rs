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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageSpec {
    pub title: String,
    pub frame: Option<Frame>,
    pub pattern: Pattern,
    pub bg_color: u32,
}

impl Default for PageSpec {
    fn default() -> Self {
        Self {
            title: String::new(),
            frame: Some(Frame::SLIDE_16_9),
            pattern: Pattern::Plain,
            bg_color: 0xffffffff,
        }
    }
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
///
/// `fill` and `dash` are style: a soft fill under a closed shape and a dashed
/// outline. They travel with the geometry (a cut shape loses both).
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
    /// Fill colour (0xRRGGBBAA) of a closed shape, drawn under the ink
    /// (the optional `fill` key, proposed, pending RFC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<u32>,
    /// Draw the outline dashed (the optional `dash` key, proposed, pending RFC).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dash: bool,
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
