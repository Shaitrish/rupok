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
    /// Set when produced by a shape tool ("rect", "ellipse", "line", "arrow").
    pub shape: Option<String>,
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
    pub data: ItemData,
}
