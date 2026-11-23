//! Lesson operations shared by the CLI and the MCP server.
//!
//! Everything here works on already-resolved filesystem paths; sandboxing is
//! the caller's job (see [`crate::workspace`]). Each mutating operation reads
//! the package, changes the lesson through the `Lesson` API, refreshes the
//! package and writes it back atomically.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rupok_core::{
    BoxGeom, CoreError, Frame, Item, ItemData, Lesson, ObjectData, PageInfo, PageSpec, Pattern,
    TextData,
};
use rupok_format::{Diagnostic, ObjectBundle, Package, Severity};
use serde::Serialize;

use crate::workspace::{PathError, is_lesson_path};

pub const GENERATOR: &str = concat!("rupok-cli/", env!("CARGO_PKG_VERSION"));
/// Manifest size given to a bare HTML file when no size is specified.
pub const DEFAULT_OBJECT_W: f64 = 1280.0;
pub const DEFAULT_OBJECT_H: f64 = 720.0;
pub const DEFAULT_TEXT_SIZE: f64 = 32.0;
/// Smallest text size we consider readable on a classroom projector.
pub const MIN_PROJECTOR_TEXT: f64 = 24.0;
const MAX_NEW_PAGES: u32 = 200;
const MAX_LISTED: usize = 5000;

#[derive(Debug, thiserror::Error)]
pub enum OpError {
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    NotFound(String),
    #[error("the smart object failed validation with {} error(s); nothing was saved. Fix the errors and try again", error_count(.0))]
    Diagnostics(Vec<Diagnostic>),
    // Not named `source`: thiserror would chain it and `{:#}` would print it twice.
    #[error("{path}: {err}")]
    Io { path: String, err: std::io::Error },
    #[error(transparent)]
    Format(#[from] rupok_format::FormatError),
    #[error(transparent)]
    Core(#[from] CoreError),
}

pub type Result<T> = std::result::Result<T, OpError>;

fn io_err(path: &Path, err: std::io::Error) -> OpError {
    OpError::Io {
        path: path.display().to_string(),
        err,
    }
}

pub fn error_count(d: &[Diagnostic]) -> usize {
    d.iter().filter(|d| d.severity == Severity::Error).count()
}

pub fn warning_count(d: &[Diagnostic]) -> usize {
    d.iter().filter(|d| d.severity == Severity::Warning).count()
}

pub fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

// ------------------------------------------------------------------ output

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Geometry {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub rot: f64,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl From<BoxGeom> for Geometry {
    fn from(g: BoxGeom) -> Self {
        Self {
            x: g.x,
            y: g.y,
            w: g.w,
            h: g.h,
            rot: g.rot,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct FrameSize {
    pub w: f64,
    pub h: f64,
}

impl From<Frame> for FrameSize {
    fn from(f: Frame) -> Self {
        Self { w: f.w, h: f.h }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ItemOutline {
    pub id: String,
    pub kind: &'static str,
    pub geometry: Geometry,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    /// Build steps declared by the object's bundle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,
    /// Current build step of this placement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    /// A media item (video, audio, YouTube, web embed): what it plays.
    /// `asset` is then its poster.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<rupok_core::MediaData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub points: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PageOutline {
    /// 1-based page number.
    pub page: usize,
    pub id: String,
    pub title: String,
    /// `None` = infinite whiteboard page.
    pub frame: Option<FrameSize>,
    pub pattern: &'static str,
    pub background: String,
    pub items: Vec<ItemOutline>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LessonOutline {
    pub title: String,
    pub page_count: usize,
    pub pages: Vec<PageOutline>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct KindCounts {
    pub stroke: usize,
    pub text: usize,
    pub image: usize,
    pub object: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct PageSummary {
    pub page: usize,
    pub id: String,
    pub title: String,
    pub frame: Option<FrameSize>,
    pub pattern: &'static str,
    pub background: String,
    pub item_count: usize,
    pub items: KindCounts,
}

#[derive(Clone, Debug, Serialize)]
pub struct ObjectSummary {
    pub id: String,
    pub title: String,
    pub width: f64,
    pub height: f64,
    pub steps: u32,
    pub libs: Vec<String>,
    pub network: Vec<String>,
    pub files: usize,
    pub bytes: usize,
    pub valid: bool,
    pub errors: usize,
    pub warnings: usize,
    /// Number of items in the lesson that place this bundle.
    pub placements: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct LessonInfo {
    pub title: String,
    pub generator: Option<String>,
    pub created: f64,
    pub modified: Option<f64>,
    pub page_count: usize,
    pub pages: Vec<PageSummary>,
    pub objects: Vec<ObjectSummary>,
    pub asset_count: usize,
    pub asset_bytes: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreatedLesson {
    pub path: String,
    pub title: String,
    pub page_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AddedPage {
    pub page: usize,
    pub page_id: String,
    pub page_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct AddedObject {
    pub item_id: String,
    pub bundle_id: String,
    pub page: usize,
    pub geometry: Geometry,
    pub steps: u32,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpdatedObject {
    pub bundle_id: String,
    pub steps: u32,
    pub libs: Vec<String>,
    /// Items placing this bundle (all now show the new HTML).
    pub placements: usize,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AddedText {
    pub item_id: String,
    pub page: usize,
    pub geometry: Geometry,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReplacedWithText {
    pub item_id: String,
    pub page: usize,
    /// How many strokes the text replaced.
    pub removed: usize,
    pub geometry: Geometry,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeletedItem {
    pub item_id: String,
    pub kind: &'static str,
    pub page: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct PageBackground {
    pub page: usize,
    pub pattern: &'static str,
    pub background: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub errors: usize,
    pub warnings: usize,
    pub diagnostics: Vec<Diagnostic>,
}

impl ValidationReport {
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            valid: error_count(&diagnostics) == 0,
            errors: error_count(&diagnostics),
            warnings: warning_count(&diagnostics),
            diagnostics,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LessonListing {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ------------------------------------------------------------------ parsing

/// `plain | grid | dots | lines` (strict, unlike `Pattern::parse`).
pub fn parse_pattern(s: &str) -> Result<Pattern> {
    match s.trim().to_ascii_lowercase().as_str() {
        "plain" => Ok(Pattern::Plain),
        "grid" => Ok(Pattern::Grid),
        "dots" => Ok(Pattern::Dots),
        "lines" => Ok(Pattern::Lines),
        other => Err(OpError::Invalid(format!(
            "unknown pattern `{other}`; use plain, grid, dots or lines"
        ))),
    }
}

/// `#rrggbb` or `#rrggbbaa` → `0xRRGGBBAA`.
pub fn parse_color(s: &str) -> Result<u32> {
    let hex = s.trim().strip_prefix('#').unwrap_or(s.trim());
    let bad = || {
        OpError::Invalid(format!(
            "color must look like #rrggbb (or #rrggbbaa), got `{s}`"
        ))
    };
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(bad());
    }
    match hex.len() {
        6 => Ok((u32::from_str_radix(hex, 16).map_err(|_| bad())? << 8) | 0xff),
        8 => u32::from_str_radix(hex, 16).map_err(|_| bad()),
        _ => Err(bad()),
    }
}

pub fn color_hex(c: u32) -> String {
    if c & 0xff == 0xff {
        format!("#{:06x}", c >> 8)
    } else {
        format!("#{c:08x}")
    }
}

/// Text of the first `<title>` element, if any.
pub fn html_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open = lower.find("<title")?;
    let start = open + lower[open..].find('>')? + 1;
    let end = start + lower[start..].find("</title")?;
    let t = html[start..end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!t.is_empty()).then_some(t)
}

pub fn slug(title: &str) -> String {
    let mut s = String::new();
    for c in title.chars() {
        if s.len() >= 32 {
            break;
        }
        if c.is_ascii_alphanumeric() {
            s.push(c.to_ascii_lowercase());
        } else if !s.is_empty() && !s.ends_with('-') {
            s.push('-');
        }
    }
    let s = s.trim_end_matches('-');
    if s.is_empty() {
        "object".into()
    } else {
        s.to_string()
    }
}

fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// `<slug>-<4 base36 chars>`, unique among `existing`.
pub fn unique_bundle_id(title: &str, existing: &BTreeMap<String, ObjectBundle>) -> String {
    const D: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let base = slug(title);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let seed = nanos ^ (u64::from(std::process::id()) << 40);
    for i in 0..10_000u64 {
        let mut v = splitmix(seed.wrapping_add(i));
        let suffix: String = (0..4)
            .map(|_| {
                let c = D[(v % 36) as usize] as char;
                v /= 36;
                c
            })
            .collect();
        let id = format!("{base}-{suffix}");
        if !existing.contains_key(&id) {
            return id;
        }
    }
    format!("{base}-{}", existing.len())
}

// ------------------------------------------------------------------ io

pub fn read_package(path: &Path) -> Result<Package> {
    let bytes = fs::read(path).map_err(|e| io_err(path, e))?;
    Ok(Package::read(&bytes)?)
}

pub fn load(path: &Path) -> Result<(Package, Lesson)> {
    let pkg = read_package(path)?;
    let lesson = pkg.lesson()?;
    Ok((pkg, lesson))
}

pub fn save(path: &Path, pkg: &mut Package, lesson: &Lesson) -> Result<()> {
    pkg.update_lesson(lesson, GENERATOR, now_ms());
    let bytes = pkg.write()?;
    write_atomic(path, &bytes)
}

/// Write via a temp file in the same directory, then rename over `path`.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "lesson".into());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    for attempt in 0..32u32 {
        let tmp = dir.join(format!(
            ".{name}.{}.{}.tmp",
            std::process::id(),
            nanos.wrapping_add(attempt)
        ));
        let mut f = match OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io_err(&tmp, e)),
        };
        let res = f.write_all(bytes).and_then(|_| f.sync_all());
        drop(f);
        if let Err(e) = res.and_then(|_| fs::rename(&tmp, path)) {
            let _ = fs::remove_file(&tmp);
            return Err(io_err(path, e));
        }
        #[cfg(unix)]
        if let Ok(d) = fs::File::open(&dir) {
            let _ = d.sync_all();
        }
        return Ok(());
    }
    Err(io_err(
        path,
        std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not create a temporary file",
        ),
    ))
}

// ------------------------------------------------------------------ helpers

fn page_at(lesson: &Lesson, page: usize) -> Result<PageInfo> {
    let pages = lesson.pages();
    if page == 0 || page > pages.len() {
        return Err(OpError::Invalid(format!(
            "page {page} does not exist; the lesson has {} page(s), numbered from 1",
            pages.len()
        )));
    }
    Ok(pages[page - 1].clone())
}

fn finite(name: &str, v: f64) -> Result<f64> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(OpError::Invalid(format!("{name} must be a finite number")))
    }
}

fn positive(name: &str, v: f64) -> Result<f64> {
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(OpError::Invalid(format!(
            "{name} must be a positive number"
        )))
    }
}

fn placement_warning(g: &BoxGeom, frame: Option<Frame>) -> Option<Diagnostic> {
    let f = frame?;
    let tol = 0.5;
    let outside = g.x < -tol || g.y < -tol || g.x + g.w > f.w + tol || g.y + g.h > f.h + tol;
    outside.then(|| Diagnostic {
        severity: Severity::Warning,
        code: "placement",
        message: format!(
            "the item ({:.0},{:.0} {:.0}×{:.0}) extends beyond the {:.0}×{:.0} slide frame and will be cut off when presenting",
            g.x, g.y, g.w, g.h, f.w, f.h
        ),
    })
}

fn snippet(s: &str, max: usize) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        flat
    } else {
        let mut t: String = flat.chars().take(max).collect();
        t.push('…');
        t
    }
}

fn item_outline(item: &Item, pkg: &Package) -> ItemOutline {
    let mut o = ItemOutline {
        id: item.id.clone(),
        kind: item.data.kind(),
        geometry: Geometry {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
            rot: 0.0,
        },
        title: None,
        text: None,
        font_size: None,
        color: None,
        bundle_id: None,
        steps: None,
        step: None,
        asset: None,
        media: None,
        tool: None,
        shape: None,
        points: None,
    };
    match &item.data {
        ItemData::Stroke(s) => {
            let b = s.aabb();
            o.geometry = Geometry {
                x: f64::from(b[0]),
                y: f64::from(b[1]),
                w: f64::from(b[2] - b[0]),
                h: f64::from(b[3] - b[1]),
                rot: 0.0,
            };
            o.color = Some(color_hex(s.color));
            o.tool = Some(s.tool.clone());
            o.shape = s.shape.clone();
            o.points = Some(s.points.len());
        }
        ItemData::Text(t) => {
            o.geometry = t.geom.into();
            o.text = Some(snippet(&t.text, 120));
            o.font_size = Some(t.size);
            o.color = Some(color_hex(t.color));
        }
        ItemData::Image(i) => {
            o.geometry = i.geom.into();
            o.asset = Some(i.asset.clone());
            if let Some(m) = &i.media {
                o.title = Some(m.title.clone()).filter(|t| !t.is_empty());
                o.media = Some(m.clone());
            }
        }
        ItemData::Object(ob) => {
            o.geometry = ob.geom.into();
            o.title = Some(ob.title.clone());
            o.bundle_id = Some(ob.bundle.clone());
            o.steps = pkg.objects.get(&ob.bundle).map(|b| b.manifest.steps);
            o.step = Some(ob.step);
        }
    }
    o
}

// ------------------------------------------------------------------ reads

pub fn outline_of(pkg: &Package, lesson: &Lesson) -> Result<LessonOutline> {
    let mut pages = Vec::new();
    for (i, p) in lesson.pages().into_iter().enumerate() {
        let items = lesson
            .items(&p.id)?
            .iter()
            .map(|it| item_outline(it, pkg))
            .collect();
        pages.push(PageOutline {
            page: i + 1,
            id: p.id,
            title: p.title,
            frame: p.frame.map(Into::into),
            pattern: p.pattern.as_str(),
            background: color_hex(p.bg_color),
            items,
        });
    }
    Ok(LessonOutline {
        title: lesson.title(),
        page_count: pages.len(),
        pages,
    })
}

pub fn outline(path: &Path) -> Result<LessonOutline> {
    let (pkg, lesson) = load(path)?;
    outline_of(&pkg, &lesson)
}

/// The lesson as Markdown (the package's `content.md`, regenerated with
/// handwriting recognition), or with `json` the structured `content.json`.
pub fn readable(path: &Path, json: bool) -> Result<String> {
    let mut pkg = read_package(path)?;
    pkg.refresh_outline()?;
    let content = pkg
        .content()
        .ok_or_else(|| OpError::Invalid("lesson has no readable document".into()))?;
    Ok(if json {
        String::from_utf8_lossy(&rupok_format::readable::content_json(&content)).into_owned()
    } else {
        rupok_format::readable::content_md(&content)
    })
}

pub fn info(path: &Path) -> Result<LessonInfo> {
    let (pkg, lesson) = load(path)?;
    let mut placements: BTreeMap<String, usize> = BTreeMap::new();
    let mut pages = Vec::new();
    for (i, p) in lesson.pages().into_iter().enumerate() {
        let mut counts = KindCounts::default();
        let items = lesson.items(&p.id)?;
        for it in &items {
            match &it.data {
                ItemData::Stroke(_) => counts.stroke += 1,
                ItemData::Text(_) => counts.text += 1,
                ItemData::Image(_) => counts.image += 1,
                ItemData::Object(o) => {
                    counts.object += 1;
                    *placements.entry(o.bundle.clone()).or_default() += 1;
                }
            }
        }
        pages.push(PageSummary {
            page: i + 1,
            id: p.id,
            title: p.title,
            frame: p.frame.map(Into::into),
            pattern: p.pattern.as_str(),
            background: color_hex(p.bg_color),
            item_count: items.len(),
            items: counts,
        });
    }
    let objects = pkg
        .objects
        .iter()
        .map(|(id, b)| {
            let d = b.validate();
            ObjectSummary {
                id: id.clone(),
                title: b.manifest.title.clone(),
                width: b.manifest.size.w,
                height: b.manifest.size.h,
                steps: b.manifest.steps,
                libs: b.manifest.libs.clone(),
                network: b.manifest.permissions.network.clone(),
                files: b.files.len(),
                bytes: b.total_bytes(),
                valid: error_count(&d) == 0,
                errors: error_count(&d),
                warnings: warning_count(&d),
                placements: placements.get(id).copied().unwrap_or(0),
            }
        })
        .collect();
    let m = pkg.manifest.as_ref();
    Ok(LessonInfo {
        title: lesson.title(),
        generator: m.map(|m| m.generator.clone()),
        created: lesson.created(),
        modified: m.map(|m| m.modified),
        page_count: pages.len(),
        pages,
        objects,
        asset_count: pkg.assets.len(),
        asset_bytes: pkg.assets.values().map(Vec::len).sum(),
    })
}

/// Every `*.rpk` under `root`, recursively. Symlinks are not followed and
/// hidden, `node_modules` and `target` directories are skipped.
pub fn list_lessons(root: &Path) -> Vec<LessonListing> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    'walk: while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if ft.is_dir() {
                if !(name.starts_with('.') || name == "node_modules" || name == "target") {
                    stack.push(path);
                }
            } else if ft.is_file() && is_lesson_path(&path) && !name.starts_with('.') {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push(match read_package(&path) {
                    Ok(pkg) => {
                        let m = pkg.manifest.as_ref();
                        LessonListing {
                            path: rel,
                            title: m.map(|m| m.title.clone()),
                            pages: m.map(|m| m.pages),
                            error: None,
                        }
                    }
                    Err(e) => LessonListing {
                        path: rel,
                        title: None,
                        pages: None,
                        error: Some(e.to_string()),
                    },
                });
                if out.len() >= MAX_LISTED {
                    break 'walk;
                }
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

// ------------------------------------------------------------------ writes

pub fn create_lesson(path: &Path, title: &str, pages: u32) -> Result<CreatedLesson> {
    if title.trim().is_empty() {
        return Err(OpError::Invalid("title must not be empty".into()));
    }
    if pages == 0 || pages > MAX_NEW_PAGES {
        return Err(OpError::Invalid(format!(
            "pages must be between 1 and {MAX_NEW_PAGES}"
        )));
    }
    if fs::symlink_metadata(path).is_ok() {
        return Err(OpError::Invalid(format!(
            "{} already exists; choose another file name or edit the existing lesson",
            path.display()
        )));
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    }
    let now = now_ms();
    let mut lesson = Lesson::new(title.trim(), now);
    for _ in 1..pages {
        lesson.add_page(None, PageSpec::default())?;
    }
    let mut pkg = Package::from_lesson(&lesson, GENERATOR, now);
    save(path, &mut pkg, &lesson)?;
    Ok(CreatedLesson {
        path: path.display().to_string(),
        title: lesson.title(),
        page_ids: lesson.pages().into_iter().map(|p| p.id).collect(),
    })
}

#[derive(Clone, Debug, Default)]
pub struct NewPage {
    pub title: Option<String>,
    /// 1-based position the new page will take; `None` appends.
    pub index: Option<usize>,
    pub pattern: Option<Pattern>,
}

pub fn add_page(path: &Path, np: NewPage) -> Result<AddedPage> {
    let (mut pkg, mut lesson) = load(path)?;
    let count = lesson.page_count();
    let idx0 = match np.index {
        None => count,
        Some(0) => {
            return Err(OpError::Invalid(
                "index is 1-based; use 1 to insert before the first page".into(),
            ));
        }
        Some(i) => (i - 1).min(count),
    };
    let spec = PageSpec {
        title: np.title.unwrap_or_default(),
        pattern: np.pattern.unwrap_or_default(),
        ..PageSpec::default()
    };
    let id = lesson.add_page(Some(idx0), spec)?;
    save(path, &mut pkg, &lesson)?;
    Ok(AddedPage {
        page: idx0 + 1,
        page_id: id,
        page_count: lesson.page_count(),
    })
}

/// Where to put a new box item. Missing values fall back to defaults.
#[derive(Clone, Copy, Debug, Default)]
pub struct Placement {
    /// 1-based page; `None` = last page.
    pub page: Option<usize>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub w: Option<f64>,
    pub h: Option<f64>,
}

/// Wrap agent HTML as a single-file bundle.
pub fn html_bundle(
    html: &str,
    title: &str,
    w: f64,
    h: f64,
    steps: u32,
    network: Vec<String>,
) -> ObjectBundle {
    let mut b = ObjectBundle::from_single_html(html, title.trim(), w, h, steps);
    b.manifest.permissions.network = network
        .into_iter()
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();
    b
}

/// Validate `bundle` and, if it has no errors, place it on a page.
pub fn add_object(path: &Path, bundle: ObjectBundle, place: Placement) -> Result<AddedObject> {
    let diagnostics = bundle.validate();
    if error_count(&diagnostics) > 0 {
        return Err(OpError::Diagnostics(diagnostics));
    }
    let (mut pkg, mut lesson) = load(path)?;
    let page_no = place.page.unwrap_or(lesson.page_count());
    let page = page_at(&lesson, page_no)?;
    let frame = page.frame.unwrap_or(Frame::SLIDE_16_9);
    let w = positive("width", place.w.unwrap_or(bundle.manifest.size.w))?;
    let h = positive("height", place.h.unwrap_or(bundle.manifest.size.h))?;
    let x = finite("x", place.x.unwrap_or((frame.w - w) / 2.0))?;
    let y = finite("y", place.y.unwrap_or((frame.h - h) / 2.0))?;
    let geom = BoxGeom {
        x,
        y,
        w,
        h,
        rot: 0.0,
    };

    let mut warnings = diagnostics;
    warnings.extend(placement_warning(&geom, page.frame));
    let bundle_id = unique_bundle_id(&bundle.manifest.title, &pkg.objects);
    let title = bundle.manifest.title.clone();
    let steps = bundle.manifest.steps;
    pkg.objects.insert(bundle_id.clone(), bundle);
    let data = ItemData::Object(ObjectData {
        geom,
        bundle: bundle_id.clone(),
        title,
        step: 0,
        state: None,
    });
    let item_id = lesson.add_item(&page.id, &data, now_ms())?;
    save(path, &mut pkg, &lesson)?;
    Ok(AddedObject {
        item_id,
        bundle_id,
        page: page_no,
        geometry: geom.into(),
        steps,
        warnings,
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct AddedMedia {
    pub item_id: String,
    /// `youtube`, `video`, `audio` or `web`.
    pub kind: String,
    /// The stored link, or the clip's asset name.
    pub src: String,
    pub title: String,
    pub page: usize,
    pub geometry: Geometry,
    pub warnings: Vec<Diagnostic>,
}

/// Place a media item: a YouTube video or a trusted web embed by link, or a
/// video or audio file stored in the lesson (spec: Media items).
pub fn add_media(
    path: &Path,
    input: rupok_format::media::MediaInput,
    place: Placement,
) -> Result<AddedMedia> {
    let (mut pkg, mut lesson) = load(path)?;
    let page_no = place.page.unwrap_or(lesson.page_count());
    let page = page_at(&lesson, page_no)?;
    let (media, poster) =
        rupok_format::media::prepare(&mut pkg, input).map_err(OpError::Invalid)?;
    let frame = page.frame.unwrap_or(Frame::SLIDE_16_9);
    let (dw, dh) = if media.kind == "audio" {
        (800.0, 450.0)
    } else {
        (DEFAULT_OBJECT_W, DEFAULT_OBJECT_H)
    };
    let w = positive("width", place.w.unwrap_or(dw))?;
    let h = positive("height", place.h.unwrap_or(dh))?;
    let geom = BoxGeom {
        x: finite("x", place.x.unwrap_or((frame.w - w) / 2.0))?,
        y: finite("y", place.y.unwrap_or((frame.h - h) / 2.0))?,
        w,
        h,
        rot: 0.0,
    };
    let warnings = placement_warning(&geom, page.frame).into_iter().collect();
    let (kind, src, title) = (media.kind.clone(), media.src.clone(), media.title.clone());
    let data = ItemData::Image(rupok_core::ImageData {
        geom,
        asset: poster,
        media: Some(media),
    });
    let item_id = lesson.add_item(&page.id, &data, now_ms())?;
    save(path, &mut pkg, &lesson)?;
    Ok(AddedMedia {
        item_id,
        kind,
        src,
        title,
        page: page_no,
        geometry: geom.into(),
        warnings,
    })
}

/// Place a 3D model (`.glb`, or `.gltf` with everything embedded) as the
/// built-in viewer Smart Object.
pub fn add_model(
    path: &Path,
    bytes: &[u8],
    name: &str,
    title: &str,
    place: Placement,
) -> Result<AddedObject> {
    let w = place.w.unwrap_or(DEFAULT_OBJECT_W);
    let h = place.h.unwrap_or(DEFAULT_OBJECT_H);
    let bundle = rupok_format::model3d::model_bundle(
        bytes,
        name,
        title,
        positive("width", w)?,
        positive("height", h)?,
    )
    .map_err(|e| OpError::Invalid(e.to_string()))?;
    add_object(path, bundle, place)
}

/// Replace the entry HTML of an existing bundle (and optionally its step
/// count / network hosts). Validates before saving.
pub fn update_object_html(
    path: &Path,
    bundle_id: &str,
    html: &str,
    steps: Option<u32>,
    network: Option<Vec<String>>,
) -> Result<UpdatedObject> {
    let (mut pkg, mut lesson) = load(path)?;
    let Some(existing) = pkg.objects.get(bundle_id) else {
        let known: Vec<&str> = pkg.objects.keys().map(String::as_str).collect();
        return Err(OpError::NotFound(format!(
            "no smart object bundle `{bundle_id}` in this lesson (bundles: {})",
            if known.is_empty() {
                "none".to_string()
            } else {
                known.join(", ")
            }
        )));
    };
    let mut b = existing.clone();
    b.files
        .insert(b.manifest.entry.clone(), html.as_bytes().to_vec());
    if let Some(s) = steps {
        b.manifest.steps = s;
    }
    if let Some(n) = network {
        b.manifest.permissions.network = n
            .into_iter()
            .map(|h| h.trim().to_ascii_lowercase())
            .collect();
    }
    // Re-detect rupoklib:// libraries from the new HTML. Multi-file bundles keep
    // what they declared, since their other files may use more libraries.
    let detected = ObjectBundle::from_single_html(html, "x", 100.0, 100.0, 0)
        .manifest
        .libs;
    if b.files.len() == 1 {
        b.manifest.libs = detected;
    } else {
        for l in detected {
            if !b.manifest.libs.contains(&l) {
                b.manifest.libs.push(l);
            }
        }
    }
    let diagnostics = b.validate();
    if error_count(&diagnostics) > 0 {
        return Err(OpError::Diagnostics(diagnostics));
    }
    let new_steps = b.manifest.steps;
    let libs = b.manifest.libs.clone();
    pkg.objects.insert(bundle_id.to_string(), b);

    // Keep every placement's current step within the new range.
    let mut placements = 0;
    for p in lesson.pages() {
        for it in lesson.items(&p.id)? {
            if let ItemData::Object(o) = &it.data
                && o.bundle == bundle_id
            {
                placements += 1;
                if o.step > new_steps {
                    lesson.set_object_step(&p.id, &it.id, new_steps)?;
                }
            }
        }
    }
    save(path, &mut pkg, &lesson)?;
    Ok(UpdatedObject {
        bundle_id: bundle_id.to_string(),
        steps: new_steps,
        libs,
        placements,
        warnings: diagnostics,
    })
}

#[derive(Clone, Debug, Default)]
pub struct NewText {
    /// 1-based page.
    pub page: usize,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: Option<f64>,
    pub size: Option<f64>,
    pub color: Option<u32>,
}

/// Rough text box size: ~0.55em per character, 1.3em line height.
fn text_box(text: &str, size: f64, w: Option<f64>, x: f64, frame: Frame) -> (f64, f64) {
    let char_w = size * 0.55;
    let lines: Vec<&str> = text.split('\n').collect();
    let longest = lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0)
        .max(1) as f64;
    let w = w.unwrap_or_else(|| (longest * char_w + size * 0.5).min((frame.w - x).max(size * 4.0)));
    let rows: f64 = lines
        .iter()
        .map(|l| {
            ((l.chars().count().max(1) as f64 * char_w) / w)
                .ceil()
                .max(1.0)
        })
        .sum();
    (w, rows * size * 1.3)
}

pub fn add_text(path: &Path, nt: NewText) -> Result<AddedText> {
    if nt.text.trim().is_empty() {
        return Err(OpError::Invalid("text must not be empty".into()));
    }
    let size = positive("size", nt.size.unwrap_or(DEFAULT_TEXT_SIZE))?;
    let x = finite("x", nt.x)?;
    let y = finite("y", nt.y)?;
    if let Some(w) = nt.w {
        positive("w", w)?;
    }
    let (mut pkg, mut lesson) = load(path)?;
    let page = page_at(&lesson, nt.page)?;
    let (w, h) = text_box(
        &nt.text,
        size,
        nt.w,
        x,
        page.frame.unwrap_or(Frame::SLIDE_16_9),
    );
    let geom = BoxGeom {
        x,
        y,
        w,
        h,
        rot: 0.0,
    };
    let mut warnings = Vec::new();
    if size < MIN_PROJECTOR_TEXT {
        warnings.push(Diagnostic {
            severity: Severity::Warning,
            code: "font_size",
            message: format!("text size {size}px is below {MIN_PROJECTOR_TEXT}px and may be unreadable on a projector"),
        });
    }
    warnings.extend(placement_warning(&geom, page.frame));
    let data = ItemData::Text(TextData {
        latex: false,
        geom,
        text: nt.text,
        size,
        color: nt.color.unwrap_or(0x000000ff),
    });
    let item_id = lesson.add_item(&page.id, &data, now_ms())?;
    save(path, &mut pkg, &lesson)?;
    Ok(AddedText {
        item_id,
        page: nt.page,
        geometry: geom.into(),
        warnings,
    })
}

/// Most strokes one conversion may replace.
const MAX_REPLACED_STROKES: usize = 5000;

/// Replace handwritten strokes with a text box (handwriting conversion done
/// by the teacher's own agent): the text sits over the ink, sized to its
/// line height, in the ink's colour unless given. One change, so replay
/// still shows the ink being written before it became text.
pub fn replace_strokes_with_text(
    path: &Path,
    page: usize,
    stroke_ids: &[String],
    text: &str,
    size: Option<f64>,
    color: Option<u32>,
) -> Result<ReplacedWithText> {
    if text.trim().is_empty() {
        return Err(OpError::Invalid("text must not be empty".into()));
    }
    if stroke_ids.is_empty() || stroke_ids.len() > MAX_REPLACED_STROKES {
        return Err(OpError::Invalid(format!(
            "give between 1 and {MAX_REPLACED_STROKES} stroke ids"
        )));
    }
    if let Some(s) = size {
        positive("size", s)?;
    }
    let (mut pkg, mut lesson) = load(path)?;
    let p = page_at(&lesson, page)?;
    let mut bounds: Option<[f32; 4]> = None;
    let mut weight: BTreeMap<u32, usize> = BTreeMap::new();
    let mut ids: Vec<String> = Vec::with_capacity(stroke_ids.len());
    for id in stroke_ids {
        if ids.contains(id) {
            continue;
        }
        let item = match lesson.item(&p.id, id) {
            Ok(i) => i,
            Err(CoreError::ItemNotFound(_)) => {
                return Err(OpError::NotFound(format!(
                    "item `{id}` is not on page {page}; call get_lesson_outline to see item ids"
                )));
            }
            Err(e) => return Err(e.into()),
        };
        let ItemData::Stroke(s) = &item.data else {
            return Err(OpError::Invalid(format!(
                "item `{id}` is a {}, not an ink stroke",
                item.data.kind()
            )));
        };
        *weight.entry(s.color).or_default() += s.points.len();
        let b = s.aabb();
        bounds = Some(match bounds {
            Some(u) => [
                u[0].min(b[0]),
                u[1].min(b[1]),
                u[2].max(b[2]),
                u[3].max(b[3]),
            ],
            None => b,
        });
        ids.push(id.clone());
    }
    let b = bounds.map(|b| b.map(f64::from)).unwrap_or_default();
    let lines = text.lines().count().max(1) as f64;
    let h = (b[3] - b[1]).max(1.0);
    let size = size.unwrap_or_else(|| (h / lines / 1.1).clamp(16.0, 400.0));
    let geom = BoxGeom {
        x: b[0],
        y: b[1] + (h - lines * size * 1.25) / 2.0,
        w: (b[2] - b[0]).max(size),
        h: lines * size * 1.3,
        rot: 0.0,
    };
    let ink = weight
        .iter()
        .max_by_key(|(_, n)| **n)
        .map(|(c, _)| *c)
        .unwrap_or(0x000000ff);
    let mut warnings = Vec::new();
    if size < MIN_PROJECTOR_TEXT {
        warnings.push(Diagnostic {
            severity: Severity::Warning,
            code: "font_size",
            message: format!("text size {size}px is below {MIN_PROJECTOR_TEXT}px and may be unreadable on a projector"),
        });
    }
    let data = ItemData::Text(TextData {
        latex: false,
        geom,
        text: text.to_string(),
        size,
        color: color.unwrap_or(ink),
    });
    let item_id = lesson.replace_items(&p.id, &ids, &data, now_ms())?;
    save(path, &mut pkg, &lesson)?;
    Ok(ReplacedWithText {
        item_id,
        page,
        removed: ids.len(),
        geometry: geom.into(),
        warnings,
    })
}

/// Delete one item. Smart Object bundles stay in the package: the lesson
/// history (replay) still references them.
pub fn delete_item(path: &Path, page: usize, item_id: &str) -> Result<DeletedItem> {
    let (mut pkg, mut lesson) = load(path)?;
    let p = page_at(&lesson, page)?;
    let item = match lesson.item(&p.id, item_id) {
        Ok(i) => i,
        Err(CoreError::ItemNotFound(_)) => {
            return Err(OpError::NotFound(format!(
                "item `{item_id}` is not on page {page}; call get_lesson_outline to see item ids"
            )));
        }
        Err(e) => return Err(e.into()),
    };
    lesson.remove_items(&p.id, std::slice::from_ref(&item.id))?;
    save(path, &mut pkg, &lesson)?;
    Ok(DeletedItem {
        item_id: item.id,
        kind: item.data.kind(),
        page,
    })
}

pub fn set_page_background(
    path: &Path,
    page: usize,
    pattern: Pattern,
    color: Option<u32>,
) -> Result<PageBackground> {
    let (mut pkg, mut lesson) = load(path)?;
    let p = page_at(&lesson, page)?;
    let color = color.unwrap_or(p.bg_color);
    lesson.set_page_background(&p.id, pattern, color)?;
    save(path, &mut pkg, &lesson)?;
    Ok(PageBackground {
        page,
        pattern: pattern.as_str(),
        background: color_hex(color),
    })
}

// ------------------------------------------------------------------ bundles from disk

/// Overrides applied when turning a file or directory into a bundle.
#[derive(Clone, Debug, Default)]
pub struct SourceOptions {
    pub title: Option<String>,
    /// Manifest size for bare HTML (ignored for bundle directories).
    pub w: Option<f64>,
    pub h: Option<f64>,
    pub steps: Option<u32>,
}

pub fn is_html_path(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
}

/// Load a bundle directory or wrap an `.html` file.
pub fn load_source(src: &Path, opts: &SourceOptions) -> Result<ObjectBundle> {
    if src.is_dir() {
        let mut b = ObjectBundle::from_dir(src)?;
        if let Some(t) = &opts.title {
            b.manifest.title = t.clone();
        }
        if let Some(s) = opts.steps {
            b.manifest.steps = s;
        }
        return Ok(b);
    }
    if !is_html_path(src) {
        return Err(OpError::Invalid(format!(
            "{}: expected a bundle directory or an .html file",
            src.display()
        )));
    }
    let html = fs::read_to_string(src).map_err(|e| io_err(src, e))?;
    let title = opts
        .title
        .clone()
        .or_else(|| html_title(&html))
        .or_else(|| src.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "Smart Object".into());
    Ok(html_bundle(
        &html,
        &title,
        opts.w.unwrap_or(DEFAULT_OBJECT_W),
        opts.h.unwrap_or(DEFAULT_OBJECT_H),
        opts.steps.unwrap_or(0),
        Vec::new(),
    ))
}

#[derive(Clone, Debug, Serialize)]
pub struct ObjectValidation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub title: String,
    #[serde(flatten)]
    pub report: ValidationReport,
}

#[derive(Clone, Debug, Serialize)]
pub struct PathValidation {
    pub target: String,
    /// `bundle`, `html` or `lesson`.
    pub kind: &'static str,
    pub valid: bool,
    pub objects: Vec<ObjectValidation>,
}

/// Validate a bundle directory, an `.html` file or every bundle in a lesson.
pub fn validate_path(target: &Path, steps: Option<u32>) -> Result<PathValidation> {
    let display = target.display().to_string();
    let (kind, objects) = if target.is_dir() {
        let v = match load_source(
            target,
            &SourceOptions {
                steps,
                ..Default::default()
            },
        ) {
            Ok(b) => ObjectValidation {
                id: None,
                title: b.manifest.title.clone(),
                report: ValidationReport::new(b.validate()),
            },
            Err(OpError::Format(e)) => ObjectValidation {
                id: None,
                title: String::new(),
                report: ValidationReport::new(vec![Diagnostic {
                    severity: Severity::Error,
                    code: "manifest",
                    message: e.to_string(),
                }]),
            },
            Err(e) => return Err(e),
        };
        ("bundle", vec![v])
    } else if is_lesson_path(target) {
        let pkg = read_package(target)?;
        let objects = pkg
            .objects
            .iter()
            .map(|(id, b)| ObjectValidation {
                id: Some(id.clone()),
                title: b.manifest.title.clone(),
                report: ValidationReport::new(b.validate()),
            })
            .collect();
        ("lesson", objects)
    } else if is_html_path(target) {
        let b = load_source(
            target,
            &SourceOptions {
                steps,
                ..Default::default()
            },
        )?;
        (
            "html",
            vec![ObjectValidation {
                id: None,
                title: b.manifest.title.clone(),
                report: ValidationReport::new(b.validate()),
            }],
        )
    } else {
        return Err(OpError::Invalid(format!(
            "{display}: expected a bundle directory, an .html file or a .rpk lesson"
        )));
    };
    let valid = objects.iter().all(|o| o.report.valid);
    Ok(PathValidation {
        target: display,
        kind,
        valid,
        objects,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_helpers() {
        assert_eq!(parse_color("#ff0000").unwrap(), 0xff0000ff);
        assert_eq!(parse_color("#11223344").unwrap(), 0x11223344);
        assert!(parse_color("red").is_err());
        assert!(parse_color("#12345").is_err());
        assert_eq!(color_hex(0xff0000ff), "#ff0000");
        assert_eq!(color_hex(0x11223344), "#11223344");
        assert_eq!(parse_pattern("Grid").unwrap(), Pattern::Grid);
        assert!(parse_pattern("stripes").is_err());
        assert_eq!(
            html_title("<html><head><TITLE> The  Water\nCycle </TITLE>").as_deref(),
            Some("The Water Cycle")
        );
        assert_eq!(html_title("<p>no title</p>"), None);
        assert_eq!(slug("The Water Cycle!"), "the-water-cycle");
        assert_eq!(slug("¿¿??"), "object");
    }

    #[test]
    fn bundle_ids_are_unique() {
        let mut m = BTreeMap::new();
        for _ in 0..50 {
            let id = unique_bundle_id("Fractions", &m);
            assert!(
                id.starts_with("fractions-") && id.len() == "fractions-".len() + 4,
                "{id}"
            );
            assert!(rupok_format::safe_path(&id));
            assert!(!m.contains_key(&id));
            m.insert(
                id,
                ObjectBundle::from_single_html("<p>x</p>", "x", 100.0, 100.0, 0),
            );
        }
    }

    #[test]
    fn atomic_write_leaves_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.rpk");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
