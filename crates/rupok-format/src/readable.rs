//! Plain-text entries that make a `.rpk` self-describing for people, search
//! tools and language models: `README.md`, `content.md` and `content.json`
//! (spec: `spec/rupok-format.md`, Readable entries).
//!
//! They are *derived*: the writer regenerates them from the lesson on every
//! save and readers ignore them. `doc.loro` stays authoritative. Everything
//! here is bounded (text is capped, page and item lists are truncated) and
//! carries nothing personal beyond what `manifest.json` already holds.
//!
//! Handwriting is summarised as a stroke count. Where the offline maths
//! recogniser ([`rupok_ink::math`]) reads a cluster of ink with high
//! confidence, the LaTeX is included and marked as recognised. Results are
//! cached per cluster in [`ReadableCache`], so an autosave only recognises
//! ink that is new since the last save.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::OnceLock;

use loro::{
    Container, LoroBinaryValue, LoroMapValue, LoroMovableList, LoroValue, ValueOrContainer,
};
use rupok_core::Lesson;
use rupok_ink::math::{AUTO_CONVERT_CONFIDENCE, MathRecognizer};
use rupok_ink::{StrokePoint, decode_points};
use serde::{Deserialize, Serialize};

use crate::object::ObjectBundle;
use crate::package::{CourseRef, Manifest};

/// Entry names.
pub const README_ENTRY: &str = "README.md";
pub const CONTENT_MD_ENTRY: &str = "content.md";
pub const CONTENT_JSON_ENTRY: &str = "content.json";

/// `content.json` `format` value and schema version.
pub const CONTENT_FORMAT: &str = "rupok-lesson-content";
pub const CONTENT_VERSION: u32 = 1;

/// Public documentation the entries point to.
pub const FORMAT_URL: &str = "https://rupok.app/rpk/";
pub const SKILL_URL: &str = "https://rupok.app/skills/read-rpk.md";
pub const OPEN_URL: &str = "https://rupok.app/open/";

/// Pages described in full; later pages are counted only.
const MAX_PAGES: usize = 300;
/// Entries of each kind (text, shapes, images, objects) listed per page.
const MAX_PER_PAGE: usize = 80;
/// Characters kept from one text item, title or description.
const MAX_TEXT: usize = 1_000;
const MAX_TITLE: usize = 200;
const MAX_DESCRIPTION: usize = 300;
/// Bytes of `content.md` / `content.json` before the outline is cut.
const MAX_CONTENT_MD: usize = 128 << 10;
/// `README.md` repeats the outline up to this size, then points to
/// `content.md`.
const MAX_README_OUTLINE: usize = 24 << 10;
/// Handwriting recognition limits.
const CLUSTER_PAD: f32 = 28.0;
const MAX_CLUSTER_STROKES: usize = 40;
const MAX_STROKES_CLUSTERED: usize = 800;
const MAX_HANDWRITING_PER_PAGE: usize = 24;
/// New (uncached) clusters recognised per save; the rest wait for the next.
const MAX_NEW_RECOGNITIONS: usize = 16;

// ---------------------------------------------------------------- model --

/// The readable outline of a lesson: the body of `content.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Content {
    pub format: String,
    pub version: u32,
    /// One line telling a reader what this file is.
    pub about: String,
    pub spec: String,
    pub title: String,
    /// ISO 8601 (UTC) dates, from the manifest.
    pub created: String,
    pub modified: String,
    pub generator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub course: Option<CourseRef>,
    pub page_count: usize,
    pub pages: Vec<PageContent>,
    /// True when pages, items or text were cut to keep the file small.
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PageContent {
    /// 1-based page number.
    pub number: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// `slide` (with `width` × `height`) or `board` (infinite canvas).
    pub layout: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    pub background: String,
    /// Typed text in reading order (top to bottom, left to right).
    pub text: Vec<TextEntry>,
    pub ink: InkSummary,
    /// Shapes and Smart Ink diagram templates, by kind.
    pub shapes: Vec<ShapeEntry>,
    pub images: Vec<ImageEntry>,
    /// Videos, audio clips and web embeds (proposed, pending RFC).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media: Vec<MediaEntry>,
    pub objects: Vec<ObjectEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TextEntry {
    pub text: String,
    /// `text` is LaTeX maths.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub latex: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InkSummary {
    /// Freehand pen strokes (shapes not included).
    pub strokes: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub highlighter: usize,
    /// Ink clusters the offline recogniser read as maths with high
    /// confidence. Machine readings: they may be wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recognized: Vec<Recognized>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Recognized {
    pub latex: String,
    pub confidence: f32,
    pub strokes: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShapeEntry {
    /// The stored `shape` value, e.g. `rect`, `axes`, `and-gate`.
    pub kind: String,
    /// Plain-English name, e.g. "coordinate axes", "AND gate".
    pub name: String,
    pub count: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ImageEntry {
    /// Path inside the package, `assets/<blake3>.<ext>`.
    pub file: String,
    pub width: f64,
    pub height: f64,
}

/// A media item: what plays, where to find it, and its poster.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaEntry {
    /// `youtube`, `video`, `audio` or `web`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// The link, for `youtube` and `web`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    /// The clip inside the package, `assets/<blake3>.<ext>`, for `video` and `audio`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub file: String,
    /// Trusted site of a web embed (`phet`, `geogebra`, …).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub site: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<f64>,
    /// The poster picture, `assets/<blake3>.<ext>`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub poster: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ObjectEntry {
    pub title: String,
    pub bundle: String,
    /// Build steps the presenter clicks through (0 = static).
    pub steps: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Entry HTML inside the package, e.g. `objects/<bundle>/index.html`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub html: String,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

// ------------------------------------------------------------ the outline --

/// What [`Outline::from_lesson`] reads from the document: everything except
/// what lives in the package (manifest dates, bundle manifests), which is
/// joined in at write time by [`Outline::content`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outline {
    pages: Vec<PageContent>,
    page_count: usize,
    truncated: bool,
}

/// Recognised handwriting by cluster, kept across saves. Bounded: each
/// refresh keeps only the clusters still on the pages.
#[derive(Clone, Debug, Default)]
pub struct ReadableCache {
    math: BTreeMap<String, Option<(String, f32)>>,
}

fn recognizer() -> &'static MathRecognizer {
    static REC: OnceLock<MathRecognizer> = OnceLock::new();
    REC.get_or_init(MathRecognizer::new)
}

impl Outline {
    /// Read the outline of `lesson`. `cache` carries recognised handwriting
    /// between saves; pass `None` to skip handwriting recognition.
    pub fn from_lesson(lesson: &Lesson, mut cache: Option<&mut ReadableCache>) -> Self {
        let infos = lesson.pages();
        let mut out = Outline {
            page_count: infos.len(),
            truncated: infos.len() > MAX_PAGES,
            ..Default::default()
        };
        let mut next_cache = BTreeMap::new();
        let mut budget = MAX_NEW_RECOGNITIONS;
        let pages = lesson.loro().get_movable_list("pages");
        for (i, info) in infos.iter().take(MAX_PAGES).enumerate() {
            let items = read_page_items(&pages, i);
            let mut page = PageContent {
                number: i + 1,
                title: cap(&info.title, MAX_TITLE),
                layout: if info.frame.is_some() {
                    "slide"
                } else {
                    "board"
                }
                .into(),
                width: info.frame.map(|f| f.w),
                height: info.frame.map(|f| f.h),
                background: info.pattern.as_str().into(),
                ..Default::default()
            };
            // Reading order: top to bottom, then left to right.
            let mut boxed: Vec<&Lite> = items.iter().filter(|it| it.boxed).collect();
            boxed.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
            for it in boxed {
                match &it.kind {
                    LiteKind::Text { text, latex } => {
                        if text.trim().is_empty() {
                            continue;
                        }
                        if page.text.len() >= MAX_PER_PAGE {
                            out.truncated = true;
                            continue;
                        }
                        let capped = cap(text, MAX_TEXT);
                        out.truncated |= capped.len() < text.len();
                        page.text.push(TextEntry {
                            text: capped,
                            latex: *latex,
                        });
                    }
                    LiteKind::Image {
                        asset,
                        media: Some(m),
                    } => {
                        if page.media.len() >= MAX_PER_PAGE {
                            out.truncated = true;
                            continue;
                        }
                        let (url, file) = match m.src_asset() {
                            Some(a) => (String::new(), format!("assets/{}", cap(a, MAX_TITLE))),
                            None => (cap(&m.src, 2048), String::new()),
                        };
                        page.media.push(MediaEntry {
                            kind: m.kind.clone(),
                            title: cap(&m.title, MAX_TITLE),
                            url,
                            file,
                            site: m.site.clone(),
                            start: m.start,
                            end: m.end,
                            poster: if asset.is_empty() {
                                String::new()
                            } else {
                                format!("assets/{}", cap(asset, MAX_TITLE))
                            },
                        });
                    }
                    LiteKind::Image { asset, media: None } => {
                        if page.images.len() >= MAX_PER_PAGE {
                            out.truncated = true;
                            continue;
                        }
                        page.images.push(ImageEntry {
                            file: format!("assets/{}", cap(asset, MAX_TITLE)),
                            width: it.w.round(),
                            height: it.h.round(),
                        });
                    }
                    LiteKind::Object { bundle, title } => {
                        if page.objects.len() >= MAX_PER_PAGE {
                            out.truncated = true;
                            continue;
                        }
                        page.objects.push(ObjectEntry {
                            title: cap(title, MAX_TITLE),
                            bundle: cap(bundle, MAX_TITLE),
                            ..Default::default()
                        });
                    }
                    LiteKind::Stroke { .. } => {}
                }
            }
            // Strokes in stacking order: shapes by kind in order of first
            // appearance; freehand ink counted (and maybe recognised).
            let mut freehand: Vec<&Lite> = Vec::new();
            for it in &items {
                if let LiteKind::Stroke {
                    shape, highlighter, ..
                } = &it.kind
                {
                    match shape {
                        Some(kind) => add_shape(&mut page.shapes, kind, &mut out.truncated),
                        None if *highlighter => page.ink.highlighter += 1,
                        None => {
                            page.ink.strokes += 1;
                            freehand.push(it);
                        }
                    }
                }
            }
            if let Some(cache) = cache.as_deref_mut() {
                page.ink.recognized =
                    recognize_page(&freehand, &cache.math, &mut next_cache, &mut budget);
            }
            out.pages.push(page);
        }
        if let Some(cache) = cache {
            cache.math = next_cache;
        }
        out
    }

    /// Join the outline with the manifest and bundles into `content.json`.
    pub fn content(&self, m: &Manifest, objects: &BTreeMap<String, ObjectBundle>) -> Content {
        let mut pages = self.pages.clone();
        for p in &mut pages {
            for o in &mut p.objects {
                if let Some(b) = objects.get(&o.bundle) {
                    o.steps = b.manifest.steps;
                    o.description = cap(b.manifest.description.trim(), MAX_DESCRIPTION);
                    if b.files.contains_key(&b.manifest.entry) {
                        o.html = format!("objects/{}/{}", o.bundle, b.manifest.entry);
                    }
                    if o.title.is_empty() {
                        o.title = cap(&b.manifest.title, MAX_TITLE);
                    }
                }
            }
        }
        Content {
            format: CONTENT_FORMAT.into(),
            version: CONTENT_VERSION,
            about: "Readable outline of a Rupok lesson (Rich Presentation Kit, .rpk), \
                    generated on save. doc.loro is authoritative."
                .into(),
            spec: FORMAT_URL.into(),
            title: cap(&m.title, MAX_TITLE),
            created: iso_date(m.created),
            modified: iso_date(m.modified),
            generator: cap(&m.generator, MAX_TITLE),
            course: m.course.clone(),
            page_count: self.page_count,
            pages,
            truncated: self.truncated,
        }
    }
}

fn add_shape(shapes: &mut Vec<ShapeEntry>, kind: &str, truncated: &mut bool) {
    let kind = cap(kind, 32);
    if let Some(s) = shapes.iter_mut().find(|s| s.kind == kind) {
        s.count += 1;
    } else if shapes.len() < MAX_PER_PAGE {
        shapes.push(ShapeEntry {
            name: shape_name(&kind),
            kind,
            count: 1,
        });
    } else {
        *truncated = true;
    }
}

/// Plain-English names for stored `shape` values (spec: Shape geometry,
/// Templates). Unknown kinds keep their stored name.
pub fn shape_name(kind: &str) -> String {
    let name = match kind {
        "rect" => "rectangle",
        "square" => "square",
        "ellipse" => "ellipse",
        "circle" => "circle",
        "line" => "line",
        "arrow" => "arrow",
        "triangle" => "triangle",
        "diamond" => "diamond (flowchart decision)",
        "pentagon" => "pentagon",
        "hexagon" => "hexagon",
        "star" => "star",
        "parallelogram" => "parallelogram (flowchart input/output)",
        "terminator" => "flowchart terminator",
        "axes" => "coordinate axes",
        "numberline" => "number line",
        "angle" => "angle arc",
        "right-angle" => "right-angle mark",
        "table" => "table",
        "and-gate" => "AND gate",
        "nand-gate" => "NAND gate",
        "or-gate" => "OR gate",
        "nor-gate" => "NOR gate",
        "xor-gate" => "XOR gate",
        "xnor-gate" => "XNOR gate",
        "not-gate" => "NOT gate",
        "resistor" => "resistor",
        "cell" => "cell (battery)",
        "bulb" => "bulb",
        "switch" => "switch",
        "equilibrium" => "equilibrium arrows",
        "graph-line" => "straight-line graph",
        "graph-quad" => "quadratic graph",
        "graph-sine" => "sine graph",
        "graph-exp" => "exponential graph",
        "venn" => "Venn diagram",
        "benzene" => "benzene ring",
        other => other,
    };
    name.to_string()
}

// ---------------------------------------------------------- lite reader --

/// What the outline needs from one item. Read straight from the Loro value
/// so stroke points are only decoded for ink that may be recognised (a full
/// `Lesson::items` read decodes every point and dominates the save time of
/// big lessons).
struct Lite {
    id: String,
    z: f64,
    created: f64,
    group: Option<String>,
    /// Box items (text, image, object) have a position for reading order.
    boxed: bool,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    kind: LiteKind,
}

enum LiteKind {
    Text {
        text: String,
        latex: bool,
    },
    Stroke {
        shape: Option<String>,
        highlighter: bool,
        pts: Option<LoroBinaryValue>,
        offset: [f32; 2],
    },
    Image {
        asset: String,
        media: Option<rupok_core::MediaData>,
    },
    Object {
        bundle: String,
        title: String,
    },
}

fn read_page_items(pages: &LoroMovableList, idx: usize) -> Vec<Lite> {
    let Some(ValueOrContainer::Container(Container::Map(page))) = pages.get(idx) else {
        return Vec::new();
    };
    let Some(ValueOrContainer::Container(Container::Map(items))) = page.get("items") else {
        return Vec::new();
    };
    let LoroValue::Map(all) = items.get_deep_value() else {
        return Vec::new();
    };
    let mut out: Vec<Lite> = all
        .iter()
        .filter_map(|(id, v)| match v {
            LoroValue::Map(m) => read_lite(id, m),
            _ => None,
        })
        .collect();
    out.sort_by(|a, b| a.z.total_cmp(&b.z).then_with(|| a.id.cmp(&b.id)));
    out
}

fn read_lite(id: &str, m: &LoroMapValue) -> Option<Lite> {
    let num = |k: &str| match m.get(k) {
        Some(LoroValue::Double(d)) if d.is_finite() => *d,
        Some(LoroValue::I64(i)) => *i as f64,
        _ => 0.0,
    };
    let text = |k: &str| match m.get(k) {
        Some(LoroValue::String(s)) => Some(s.to_string()),
        _ => None,
    };
    let kind = match text("kind")?.as_str() {
        "text" => LiteKind::Text {
            text: text("text").unwrap_or_default(),
            latex: matches!(m.get("latex"), Some(LoroValue::Bool(true))),
        },
        "stroke" => LiteKind::Stroke {
            shape: text("shape").filter(|s| !s.is_empty()),
            highlighter: text("tool").as_deref() == Some("highlighter"),
            pts: match m.get("pts") {
                Some(LoroValue::Binary(b)) => Some(b.clone()),
                _ => None,
            },
            offset: [num("ox") as f32, num("oy") as f32],
        },
        "image" => LiteKind::Image {
            asset: text("asset").unwrap_or_default(),
            media: m.get("media").and_then(rupok_core::media_from_value),
        },
        "object" => LiteKind::Object {
            bundle: text("bundle").unwrap_or_default(),
            title: text("title").unwrap_or_default(),
        },
        // Unknown kinds are skipped (forward compatibility).
        _ => return None,
    };
    Some(Lite {
        id: id.to_string(),
        z: num("z"),
        created: num("created"),
        group: text("group").filter(|g| !g.is_empty()),
        boxed: !matches!(kind, LiteKind::Stroke { .. }),
        x: num("x"),
        y: num("y"),
        w: num("w"),
        h: num("h"),
        kind,
    })
}

// ---------------------------------------------------------- handwriting --

/// Cluster freehand strokes (by group, then by touching padded boxes) and
/// read each cluster with the maths recogniser, reusing cached readings.
fn recognize_page(
    strokes: &[&Lite],
    old: &BTreeMap<String, Option<(String, f32)>>,
    next: &mut BTreeMap<String, Option<(String, f32)>>,
    budget: &mut usize,
) -> Vec<Recognized> {
    if strokes.is_empty() || strokes.len() > MAX_STROKES_CLUSTERED {
        return Vec::new();
    }
    // Decode the points (moved by their offset); undecodable ink is left out.
    let ink: Vec<(&Lite, Vec<StrokePoint>)> = strokes
        .iter()
        .filter_map(|it| {
            let LiteKind::Stroke {
                pts: Some(pts),
                offset,
                ..
            } = &it.kind
            else {
                return None;
            };
            let pts = decode_points(pts).ok().filter(|p| !p.is_empty())?;
            let moved = pts
                .iter()
                .map(|p| StrokePoint {
                    x: p.x + offset[0],
                    y: p.y + offset[1],
                    ..*p
                })
                .collect();
            Some((*it, moved))
        })
        .collect();
    let n = ink.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let boxes: Vec<[f32; 4]> = ink.iter().map(|(_, p)| rupok_ink::bounds(p, 0.0)).collect();
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (boxes[i], boxes[j]);
            let touch = a[0] - CLUSTER_PAD <= b[2]
                && b[0] - CLUSTER_PAD <= a[2]
                && a[1] - CLUSTER_PAD <= b[3]
                && b[1] - CLUSTER_PAD <= a[3];
            let same_group = ink[i].0.group.is_some() && ink[i].0.group == ink[j].0.group;
            if touch || same_group {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                parent[ri] = rj;
            }
        }
    }
    let mut clusters: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        let r = find(&mut parent, i);
        clusters.entry(r).or_default().push(i);
    }
    // Reading order of clusters: by their top edge.
    let mut list: Vec<Vec<usize>> = clusters.into_values().collect();
    let top = |c: &Vec<usize>| c.iter().map(|&i| boxes[i][1]).fold(f32::MAX, f32::min);
    list.sort_by(|a, b| top(a).total_cmp(&top(b)));
    let mut out = Vec::new();
    for mut members in list {
        if out.len() >= MAX_HANDWRITING_PER_PAGE {
            break;
        }
        if members.len() > MAX_CLUSTER_STROKES {
            continue;
        }
        // Time order is what the recogniser expects.
        members.sort_by(|&a, &b| {
            ink[a]
                .0
                .created
                .total_cmp(&ink[b].0.created)
                .then_with(|| ink[a].0.id.cmp(&ink[b].0.id))
        });
        let key = cluster_key(&members, &ink);
        let reading = match old.get(&key).or_else(|| next.get(&key)) {
            Some(r) => r.clone(),
            None if *budget > 0 => {
                *budget -= 1;
                let group: Vec<Vec<StrokePoint>> =
                    members.iter().map(|&i| ink[i].1.clone()).collect();
                recognizer()
                    .recognize(&group)
                    .into_iter()
                    .next()
                    .filter(|c| c.confidence >= AUTO_CONVERT_CONFIDENCE && !c.latex.is_empty())
                    .map(|c| (cap(&c.latex, MAX_TEXT), c.confidence))
            }
            // Out of budget: read on a later save.
            None => continue,
        };
        next.insert(key, reading.clone());
        if let Some((latex, confidence)) = reading {
            out.push(Recognized {
                latex,
                confidence: (confidence * 100.0).round() / 100.0,
                strokes: members.len(),
            });
        }
    }
    out
}

/// Identity of a cluster: its strokes' ids and moved points (points never
/// change in place, so this changes exactly when the ink does).
fn cluster_key(members: &[usize], ink: &[(&Lite, Vec<StrokePoint>)]) -> String {
    let mut h = blake3::Hasher::new();
    for &i in members {
        let (it, pts) = &ink[i];
        h.update(it.id.as_bytes());
        h.update(&[0]);
        h.update(&(pts.len() as u64).to_le_bytes());
        if let Some(p) = pts.first() {
            h.update(&p.x.to_le_bytes());
            h.update(&p.y.to_le_bytes());
        }
    }
    h.finalize().to_hex()[..24].to_string()
}

// ------------------------------------------------------------- markdown --

/// `content.md`: the outline as Markdown.
pub fn content_md(c: &Content) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "# {}",
        md_line(if c.title.is_empty() {
            "Untitled lesson"
        } else {
            &c.title
        })
    );
    s.push('\n');
    if let Some(course) = &c.course {
        for (k, v) in [
            ("Subject", &course.subject),
            ("Course", &course.course),
            ("Chapter", &course.chapter),
        ] {
            if !v.is_empty() {
                let _ = writeln!(s, "- {k}: {}", md_line(v));
            }
        }
    }
    let _ = writeln!(s, "- Pages: {}", c.page_count);
    if !c.created.is_empty() {
        let _ = writeln!(s, "- Created: {}", c.created);
    }
    if !c.modified.is_empty() {
        let _ = writeln!(s, "- Modified: {}", c.modified);
    }
    let _ = writeln!(s, "- Made with: {}", md_line(&c.generator));
    for p in &c.pages {
        if s.len() > MAX_CONTENT_MD {
            let _ = writeln!(
                s,
                "\n_Outline cut here to keep it small; {} of {} pages listed._",
                p.number - 1,
                c.page_count
            );
            return s;
        }
        page_md(&mut s, p);
    }
    if c.page_count > c.pages.len() {
        let _ = writeln!(
            s,
            "\n_{} more pages are in the lesson but not listed here._",
            c.page_count - c.pages.len()
        );
    }
    if c.truncated {
        s.push_str("\n_Some items or text were shortened to keep this outline small._\n");
    }
    s
}

fn page_md(s: &mut String, p: &PageContent) {
    let _ = write!(s, "\n## Page {}", p.number);
    if !p.title.is_empty() {
        let _ = write!(s, ": {}", md_line(&p.title));
    }
    s.push_str("\n\n");
    let empty = p.text.is_empty()
        && p.ink.strokes == 0
        && p.ink.highlighter == 0
        && p.shapes.is_empty()
        && p.images.is_empty()
        && p.media.is_empty()
        && p.objects.is_empty();
    if empty {
        s.push_str("_(empty page)_\n");
        return;
    }
    for t in &p.text {
        if t.latex {
            let _ = writeln!(s, "$${}$$\n", t.text.trim());
        } else {
            // Keep the teacher's line breaks; quote each line so headings or
            // lists in the text cannot change the outline's structure.
            for line in t.text.lines() {
                let _ = writeln!(s, "> {}", line.trim_end());
            }
            s.push('\n');
        }
    }
    if p.ink.strokes > 0 {
        let _ = writeln!(
            s,
            "- Handwriting: {} ink stroke{}",
            p.ink.strokes,
            plural(p.ink.strokes)
        );
        for r in &p.ink.recognized {
            let _ = writeln!(
                s,
                "  - recognised maths (machine reading, may be wrong): ${}$",
                r.latex
            );
        }
    }
    if p.ink.highlighter > 0 {
        let _ = writeln!(
            s,
            "- Highlighter: {} stroke{}",
            p.ink.highlighter,
            plural(p.ink.highlighter)
        );
    }
    if !p.shapes.is_empty() {
        let list: Vec<String> = p
            .shapes
            .iter()
            .map(|x| {
                if x.count > 1 {
                    format!("{} × {}", x.count, x.name)
                } else {
                    x.name.clone()
                }
            })
            .collect();
        let _ = writeln!(s, "- Shapes and diagrams: {}", list.join(", "));
    }
    for img in &p.images {
        let _ = writeln!(
            s,
            "- Image: `{}` ({} × {})",
            img.file, img.width, img.height
        );
    }
    for m in &p.media {
        let what = match m.kind.as_str() {
            "youtube" => "YouTube video".to_string(),
            "video" => "Video".to_string(),
            "audio" => "Audio".to_string(),
            _ => match crate::media::site(&m.site) {
                Some(site) => format!("Web embed ({})", site.name),
                None => "Web embed".to_string(),
            },
        };
        let _ = write!(s, "- {what}");
        if !m.title.is_empty() {
            let _ = write!(s, ": **{}**", md_line(&m.title));
        }
        if !m.url.is_empty() {
            let _ = write!(s, " <{}>", m.url.replace(['<', '>', ' '], ""));
        }
        if !m.file.is_empty() {
            let _ = write!(s, " `{}`", m.file);
        }
        let clock = |t: f64| {
            let t = t.max(0.0).round() as u64;
            format!("{}:{:02}", t / 60, t % 60)
        };
        match (m.start, m.end) {
            (Some(a), Some(b)) => {
                let _ = write!(s, " (plays {} to {})", clock(a), clock(b));
            }
            (Some(a), None) => {
                let _ = write!(s, " (from {})", clock(a));
            }
            (None, Some(b)) => {
                let _ = write!(s, " (until {})", clock(b));
            }
            _ => {}
        }
        s.push('\n');
    }
    for o in &p.objects {
        let _ = write!(s, "- Smart Object: **{}** (", md_line(&o.title));
        if o.steps > 0 {
            let _ = write!(s, "{} step{}", o.steps, plural(o.steps as usize));
        } else {
            s.push_str("interactive");
        }
        if !o.html.is_empty() {
            let _ = write!(s, ", `{}`", o.html);
        }
        s.push(')');
        if !o.description.is_empty() {
            let _ = write!(s, ": {}", md_line(&o.description));
        }
        s.push('\n');
    }
}

/// `README.md`: what this file is, where the format is documented, then the
/// outline (cut short for big lessons; the whole outline is `content.md`).
pub fn readme_md(c: &Content, outline_md: &str) -> String {
    let mut s = String::new();
    s.push_str(
        "This is a Rupok lesson (Rich Presentation Kit, `.rpk`).\n\
         Full format: https://rupok.app/rpk/ · Agent skill: https://rupok.app/skills/read-rpk.md\n\n",
    );
    let _ = writeln!(
        s,
        "Open it in Rupok, free in the browser: {OPEN_URL}. A `.rpk` is a ZIP archive. \
         `content.md` and `content.json` are readable outlines written on every save; \
         interactive Smart Objects are HTML files under `objects/`; the lesson itself, \
         with every stroke and its full history, is `doc.loro` (a Loro CRDT snapshot, \
         authoritative).\n"
    );
    s.push_str("---\n\n");
    if outline_md.len() <= MAX_README_OUTLINE {
        s.push_str(outline_md);
    } else {
        let cut = floor_char_boundary(outline_md, MAX_README_OUTLINE);
        let cut = outline_md[..cut].rfind('\n').unwrap_or(cut);
        s.push_str(&outline_md[..cut]);
        let _ = writeln!(
            s,
            "\n\n_The outline continues in `content.md` ({} pages in all)._",
            c.page_count
        );
    }
    s
}

/// `content.json`, pretty-printed; pages are dropped from the end until it
/// fits the size cap.
pub fn content_json(c: &Content) -> Vec<u8> {
    let mut c = c.clone();
    loop {
        let bytes = serde_json::to_vec_pretty(&c).unwrap_or_default();
        if bytes.len() <= MAX_CONTENT_MD || c.pages.is_empty() {
            return bytes;
        }
        let keep = c.pages.len() * 3 / 4;
        c.pages.truncate(keep);
        c.truncated = true;
    }
}

// -------------------------------------------------------------- helpers --

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn floor_char_boundary(s: &str, max: usize) -> usize {
    if max >= s.len() {
        return s.len();
    }
    let mut i = max;
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// At most `max` bytes of `s`, cut on a char boundary, with "…" when cut.
fn cap(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut out = s[..floor_char_boundary(s, max)].to_string();
    out.push('…');
    out
}

/// One Markdown line: newlines folded, leading `#` escaped.
fn md_line(s: &str) -> String {
    let one: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let one = one.trim();
    if one.starts_with('#') {
        format!("\\{one}")
    } else {
        one.to_string()
    }
}

/// `YYYY-MM-DDTHH:MMZ` (UTC) for ms since the Unix epoch; empty when out of
/// range. No clock: the time comes from the manifest.
pub fn iso_date(ms: f64) -> String {
    if !ms.is_finite() || !(0.0..=253_402_300_799_000.0).contains(&ms) || ms == 0.0 {
        return String::new();
    }
    let secs = (ms / 1000.0).floor() as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(iso_date(1_763_870_400_000.0), "2025-11-23T04:00Z");
        assert_eq!(iso_date(0.0), "");
        assert_eq!(iso_date(f64::NAN), "");
    }

    #[test]
    fn caps_on_char_boundaries() {
        let s = "অআইঈউ".repeat(400);
        let c = cap(&s, 1000);
        assert!(c.len() <= 1000 + '…'.len_utf8());
        assert!(c.ends_with('…'));
    }

    #[test]
    fn md_line_is_one_safe_line() {
        assert_eq!(md_line("# Big\nheading"), "\\# Big heading");
    }
}
