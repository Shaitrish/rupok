//! Run an [`Op`] on the open lesson.
//!
//! Every document change is exactly **one** Loro commit whose message names
//! the agent ([`crate::commit_message`]): one agent action is one undo step on
//! the board and stays attributed in the history (and so in replay).
//!
//! Smart Object bundles live in the package, outside the CRDT. So that
//! `update_smart_object` can be undone too, it stores the new HTML as a
//! *new* bundle and re-points the placements in a Loro change; the old bundle
//! stays in the package for undo and replay.

use std::collections::BTreeMap;

use rupok_core::{
    BoxGeom, CoreError, Frame, Item, ItemData, Lesson, ObjectData, PageInfo, PageSpec, Pattern,
    TextData,
};
use rupok_format::{Diagnostic, ObjectBundle, Package, Severity};
use serde::Serialize;
use serde_json::{Value, json};

use crate::op::{Access, Op, PatternName};

pub const DEFAULT_OBJECT_W: f64 = 1280.0;
pub const DEFAULT_OBJECT_H: f64 = 720.0;
pub const DEFAULT_TEXT_SIZE: f64 = 32.0;
/// Smallest text size considered readable on a classroom projector.
pub const MIN_PROJECTOR_TEXT: f64 = 24.0;
const MAX_TEXT_CHARS: usize = 20_000;
const MAX_ITEMS_PER_OP: usize = 5_000;
const OUTLINE_TEXT_CHARS: usize = 4_000;
/// Largest step a Smart Object without declared steps may be moved to.
const MAX_STEPS: u32 = 500;

/// The board an op runs against.
pub struct BoardCtx<'a> {
    pub pkg: &'a mut Package,
    pub lesson: &'a mut Lesson,
    /// Id of the page the board shows.
    pub page_id: &'a str,
    /// Selected item ids on that page.
    pub selection: &'a [String],
    /// Student view: document changes are refused.
    pub read_only: bool,
}

/// What an op did.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// JSON returned to the agent.
    pub result: Value,
    /// The document changed: refresh the scene.
    pub changed: bool,
    /// Show this page (0-based) afterwards.
    pub show_page: Option<usize>,
}

impl Outcome {
    pub(crate) fn read(result: Value) -> Self {
        Self {
            result,
            changed: false,
            show_page: None,
        }
    }
    pub(crate) fn changed(result: Value) -> Self {
        Self {
            result,
            changed: true,
            show_page: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LiveError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    NotFound(String),
    #[error("the smart object failed validation with {} error(s); nothing was changed on the board. Fix the errors and try again", .0.iter().filter(|d| d.severity == Severity::Error).count())]
    Diagnostics(Vec<Diagnostic>),
    #[error(
        "the board is in Student view (read-only); ask the teacher to switch to Prepare or Teach"
    )]
    ReadOnly,
    #[error("`{0}` is handled by the board app, not the lesson document")]
    Unsupported(&'static str),
    #[error(transparent)]
    Core(#[from] CoreError),
}

impl LiveError {
    /// The JSON body an agent sees for this error.
    pub fn to_json(&self) -> Value {
        match self {
            LiveError::Diagnostics(d) => json!({
                "error": self.to_string(),
                "saved": false,
                "diagnostics": d,
            }),
            _ => json!({ "error": self.to_string() }),
        }
    }
}

type Result<T> = std::result::Result<T, LiveError>;

/// Run `op` on the board as `agent`. `now_ms` is the wall-clock time of the
/// board (ms since the Unix epoch), used for item creation times.
pub fn apply(ctx: BoardCtx<'_>, op: &Op, agent: &str, now_ms: f64) -> Result<Outcome> {
    apply_with(ctx, op, agent, now_ms, None)
}

/// [`apply`] with the board's handwriting glyphs (the teacher's calibrated
/// forms) for `write_handwriting`; `None` writes in the neat built-in hand.
pub fn apply_with(
    mut ctx: BoardCtx<'_>,
    op: &Op,
    agent: &str,
    now_ms: f64,
    glyphs: Option<&rupok_ink::write::GlyphSet>,
) -> Result<Outcome> {
    if ctx.read_only && op.access() == Access::Write {
        return Err(LiveError::ReadOnly);
    }
    let msg = crate::commit_message(agent, op.name());
    let out = if let Op::WriteHandwriting { .. } = op {
        crate::handwriting::run(&mut ctx, op, &msg, now_ms, glyphs)
    } else {
        run(&mut ctx, op, &msg, now_ms)
    };
    // A failed op leaves no commit behind, and Loro carries pending commit
    // options over to the next change: never let the agent's tag leak onto
    // the teacher's next stroke.
    ctx.lesson.loro().clear_next_commit_options();
    out
}

/// Tag the next commit with the agent's message. Call right before the one
/// mutation that commits.
pub(crate) fn tag(lesson: &Lesson, msg: &str) {
    lesson.loro().set_next_commit_message(msg);
}

fn run(ctx: &mut BoardCtx<'_>, op: &Op, msg: &str, now: f64) -> Result<Outcome> {
    match op {
        Op::GetPage { page } => {
            let (idx, info) = page_ref(ctx.lesson, ctx.page_id, *page)?;
            let current = page_index(ctx.lesson, ctx.page_id).unwrap_or(0);
            let outline = page_outline(ctx.pkg, ctx.lesson, idx, &info, now)?;
            let selection: Vec<&String> = if idx == current {
                ctx.selection.iter().collect()
            } else {
                Vec::new()
            };
            let strokes = outline.items.iter().filter(|i| i.kind == "stroke").count();
            Ok(Outcome::read(json!({
                "page": outline.page,
                "page_count": ctx.lesson.page_count(),
                "current_page": current + 1,
                "id": outline.id,
                "title": outline.title,
                "frame": outline.frame,
                "pattern": outline.pattern,
                "background": outline.background,
                "scheme": outline.scheme,
                "ink_strokes": strokes,
                "selection": selection,
                "items": outline.items,
            })))
        }
        Op::GetOutline => {
            let mut pages = Vec::new();
            for (i, p) in ctx.lesson.pages().into_iter().enumerate() {
                pages.push(page_outline(ctx.pkg, ctx.lesson, i, &p, now)?);
            }
            Ok(Outcome::read(json!({
                "title": ctx.lesson.title(),
                "page_count": pages.len(),
                "current_page": page_index(ctx.lesson, ctx.page_id).unwrap_or(0) + 1,
                "pages": pages,
            })))
        }
        Op::GetSelection => {
            let idx = page_index(ctx.lesson, ctx.page_id).unwrap_or(0);
            let items = ctx.lesson.items(ctx.page_id)?;
            let chosen: Vec<ItemOutline> = items
                .iter()
                .filter(|i| ctx.selection.contains(&i.id))
                .map(|i| item_outline(i, ctx.pkg, now))
                .collect();
            let bounds = union_bounds(items.iter().filter(|i| ctx.selection.contains(&i.id)));
            Ok(Outcome::read(json!({
                "page": idx + 1,
                "count": chosen.len(),
                "bounds": bounds.map(|b| geometry_of(&b)),
                "items": chosen,
            })))
        }
        Op::Snapshot { .. } => Err(LiveError::Unsupported("snapshot")),
        Op::WriteHandwriting { .. } => crate::handwriting::run(ctx, op, msg, now, None),
        Op::PointAt { .. } => Err(LiveError::Unsupported("point_at")),
        // Shape geometry lives in the board engine (rupok-wasm `live.rs`).
        Op::AddShape { .. } => Err(LiveError::Unsupported("add_shape")),
        Op::AddTable { .. } => Err(LiveError::Unsupported("add_table")),

        Op::GoToPage { page } => {
            let (idx, info) = page_ref(ctx.lesson, ctx.page_id, Some(*page))?;
            Ok(Outcome {
                result: json!({
                    "page": idx + 1,
                    "page_count": ctx.lesson.page_count(),
                    "title": info.title,
                }),
                changed: false,
                show_page: Some(idx),
            })
        }
        Op::SetBoardTheme {
            page_color,
            pattern,
            ui,
        } => {
            let (idx, info) = page_ref(ctx.lesson, ctx.page_id, None)?;
            let mut out = json!({ "page": idx + 1, "ui": ui });
            if page_color.is_none() && pattern.is_none() {
                out["background"] = json!(color_hex(info.bg_color));
                out["pattern"] = json!(info.pattern.as_str());
                return Ok(Outcome::read(out));
            }
            let color = match page_color {
                Some(c) => board_color(c)?,
                None => info.bg_color,
            };
            let pat = pattern.map(Pattern::from).unwrap_or(info.pattern);
            tag(ctx.lesson, msg);
            ctx.lesson.set_page_background(&info.id, pat, color)?;
            out["background"] = json!(color_hex(color));
            out["pattern"] = json!(pat.as_str());
            out["scheme"] = json!(scheme(color));
            Ok(Outcome::changed(out))
        }

        Op::SetPageDesign(_) => crate::design::run_page_design(ctx, op, msg),
        Op::SetBranding(_) => crate::design::run_branding(ctx, op, msg),

        Op::AddPage {
            title,
            index,
            pattern,
            color,
        } => {
            let count = ctx.lesson.page_count();
            let idx0 = match index {
                None => count,
                Some(0) => {
                    return Err(LiveError::Invalid(
                        "index is 1-based; use 1 to insert before the first page".into(),
                    ));
                }
                Some(i) => (*i as usize - 1).min(count),
            };
            let spec = new_page_spec(ctx.lesson, ctx.page_id, title, *pattern, color)?;
            tag(ctx.lesson, msg);
            let id = ctx.lesson.add_page(Some(idx0), spec)?;
            Ok(Outcome::changed(json!({
                "page": idx0 + 1,
                "page_id": id,
                "page_count": ctx.lesson.page_count(),
            })))
        }
        Op::AddPageAfterCurrent {
            title,
            pattern,
            color,
            show,
        } => {
            let idx0 =
                page_index(ctx.lesson, ctx.page_id).map_or(ctx.lesson.page_count(), |i| i + 1);
            let spec = new_page_spec(ctx.lesson, ctx.page_id, title, *pattern, color)?;
            tag(ctx.lesson, msg);
            let id = ctx.lesson.add_page(Some(idx0), spec)?;
            Ok(Outcome {
                result: json!({
                    "page": idx0 + 1,
                    "page_id": id,
                    "page_count": ctx.lesson.page_count(),
                    "shown": show,
                }),
                changed: true,
                show_page: show.then_some(idx0),
            })
        }
        Op::AddText {
            page,
            text,
            x,
            y,
            w,
            size,
            color,
        } => {
            check_text(text)?;
            let size = positive("size", size.unwrap_or(DEFAULT_TEXT_SIZE))?;
            let x = finite("x", *x)?;
            let y = finite("y", *y)?;
            if let Some(w) = w {
                positive("w", *w)?;
            }
            let (idx, info) = page_ref(ctx.lesson, ctx.page_id, *page)?;
            let frame = info.frame.unwrap_or(Frame::SLIDE_16_9);
            let (w, h) = text_box(text, size, *w, x, frame);
            let geom = BoxGeom {
                x,
                y,
                w,
                h,
                rot: 0.0,
            };
            let color = match color {
                Some(c) => parse_color(c)?,
                None => default_ink(info.bg_color),
            };
            let mut warnings = Vec::new();
            if size < MIN_PROJECTOR_TEXT {
                warnings.push(warning(
                    "font_size",
                    format!(
                        "text size {size}px is below {MIN_PROJECTOR_TEXT}px and may be unreadable on a projector"
                    ),
                ));
            }
            warnings.extend(placement_warning(&geom, info.frame));
            let data = ItemData::Text(TextData {
                latex: false,
                geom,
                text: text.clone(),
                size,
                color,
                stamp: None,
            });
            tag(ctx.lesson, msg);
            let item_id = ctx.lesson.add_item(&info.id, &data, now)?;
            Ok(Outcome::changed(json!({
                "item_id": item_id,
                "page": idx + 1,
                "page_id": info.id,
                "geometry": geometry_of(&geom),
                "warnings": warnings,
            })))
        }
        Op::AddSmartObject {
            page,
            html,
            title,
            width,
            height,
            steps,
            x,
            y,
            network_hosts,
        } => {
            if title.trim().is_empty() {
                return Err(LiveError::Invalid("title must not be empty".into()));
            }
            let w = positive("width", width.unwrap_or(DEFAULT_OBJECT_W))?;
            let h = positive("height", height.unwrap_or(DEFAULT_OBJECT_H))?;
            let bundle = html_bundle(
                html,
                title,
                w,
                h,
                steps.unwrap_or(0),
                network_hosts.clone().unwrap_or_default(),
            );
            let diagnostics = bundle.validate();
            if has_errors(&diagnostics) {
                return Err(LiveError::Diagnostics(diagnostics));
            }
            let (idx, info) = page_ref(ctx.lesson, ctx.page_id, *page)?;
            let frame = info.frame.unwrap_or(Frame::SLIDE_16_9);
            let geom = BoxGeom {
                x: finite("x", x.unwrap_or((frame.w - w) / 2.0))?,
                y: finite("y", y.unwrap_or((frame.h - h) / 2.0))?,
                w,
                h,
                rot: 0.0,
            };
            let mut warnings = diagnostics;
            warnings.extend(placement_warning(&geom, info.frame));
            let bundle_id = unique_bundle_id(title, html, &ctx.pkg.objects);
            let steps = bundle.manifest.steps;
            let title = bundle.manifest.title.clone();
            ctx.pkg.objects.insert(bundle_id.clone(), bundle);
            let data = ItemData::Object(ObjectData {
                geom,
                bundle: bundle_id.clone(),
                title,
                step: 0,
                state: None,
            });
            tag(ctx.lesson, msg);
            let item_id = match ctx.lesson.add_item(&info.id, &data, now) {
                Ok(id) => id,
                Err(e) => {
                    ctx.pkg.objects.remove(&bundle_id);
                    return Err(e.into());
                }
            };
            Ok(Outcome::changed(json!({
                "item_id": item_id,
                "bundle_id": bundle_id,
                "page": idx + 1,
                "page_id": info.id,
                "geometry": geometry_of(&geom),
                "steps": steps,
                "warnings": warnings,
            })))
        }
        Op::AddMedia {
            page,
            url,
            file_base64,
            file_name,
            title,
            start,
            end,
            muted,
            looped,
            autoplay,
            x,
            y,
            width,
            height,
        } => {
            let file = match file_base64 {
                Some(b) => Some(
                    rupok_format::media::decode_base64(b)
                        .ok_or_else(|| LiveError::Invalid("file_base64 is not base64".into()))?,
                ),
                None => None,
            };
            let input = rupok_format::media::MediaInput {
                url: url.clone(),
                file,
                file_name: file_name.clone(),
                title: title.clone().filter(|t| !t.trim().is_empty()),
                start: start.map(|v| finite("start", v)).transpose()?,
                end: end.map(|v| finite("end", v)).transpose()?,
                muted: muted.unwrap_or(false),
                looped: looped.unwrap_or(false),
                autoplay: autoplay.unwrap_or(false),
            };
            let (idx, info) = page_ref(ctx.lesson, ctx.page_id, *page)?;
            let (media, poster) =
                rupok_format::media::prepare(ctx.pkg, input).map_err(LiveError::Invalid)?;
            let frame = info.frame.unwrap_or(Frame::SLIDE_16_9);
            let (dw, dh) = if media.kind == "audio" {
                (800.0, 450.0)
            } else {
                (DEFAULT_OBJECT_W, DEFAULT_OBJECT_H)
            };
            let w = positive("width", width.unwrap_or(dw))?;
            let h = positive("height", height.unwrap_or(dh))?;
            let geom = BoxGeom {
                x: finite("x", x.unwrap_or((frame.w - w) / 2.0))?,
                y: finite("y", y.unwrap_or((frame.h - h) / 2.0))?,
                w,
                h,
                rot: 0.0,
            };
            let warnings: Vec<Diagnostic> =
                placement_warning(&geom, info.frame).into_iter().collect();
            let kind = media.kind.clone();
            let src = media.src.clone();
            let data = ItemData::Image(rupok_core::ImageData {
                geom,
                asset: poster,
                media: Some(media),
            });
            tag(ctx.lesson, msg);
            let item_id = ctx.lesson.add_item(&info.id, &data, now)?;
            Ok(Outcome::changed(json!({
                "item_id": item_id,
                "kind": kind,
                "src": src,
                "page": idx + 1,
                "page_id": info.id,
                "geometry": geometry_of(&geom),
                "warnings": warnings,
            })))
        }
        Op::AddModel {
            page,
            file_base64,
            file_name,
            title,
            x,
            y,
            width,
            height,
        } => {
            let bytes = rupok_format::media::decode_base64(file_base64)
                .ok_or_else(|| LiveError::Invalid("file_base64 is not base64".into()))?;
            let w = positive("width", width.unwrap_or(DEFAULT_OBJECT_W))?;
            let h = positive("height", height.unwrap_or(DEFAULT_OBJECT_H))?;
            let name = file_name.clone().unwrap_or_else(|| "model.glb".into());
            let bundle = rupok_format::model3d::model_bundle(
                &bytes,
                &name,
                title.as_deref().unwrap_or(""),
                w,
                h,
            )
            .map_err(|e| LiveError::Invalid(e.to_string()))?;
            let (idx, info) = page_ref(ctx.lesson, ctx.page_id, *page)?;
            let frame = info.frame.unwrap_or(Frame::SLIDE_16_9);
            let geom = BoxGeom {
                x: finite("x", x.unwrap_or((frame.w - w) / 2.0))?,
                y: finite("y", y.unwrap_or((frame.h - h) / 2.0))?,
                w,
                h,
                rot: 0.0,
            };
            let warnings: Vec<Diagnostic> =
                placement_warning(&geom, info.frame).into_iter().collect();
            let title = bundle.manifest.title.clone();
            let bundle_id = unique_bundle_id(&title, &name, &ctx.pkg.objects);
            ctx.pkg.objects.insert(bundle_id.clone(), bundle);
            let data = ItemData::Object(ObjectData {
                geom,
                bundle: bundle_id.clone(),
                title,
                step: 0,
                state: None,
            });
            tag(ctx.lesson, msg);
            let item_id = match ctx.lesson.add_item(&info.id, &data, now) {
                Ok(id) => id,
                Err(e) => {
                    ctx.pkg.objects.remove(&bundle_id);
                    return Err(e.into());
                }
            };
            Ok(Outcome::changed(json!({
                "item_id": item_id,
                "bundle_id": bundle_id,
                "page": idx + 1,
                "page_id": info.id,
                "geometry": geometry_of(&geom),
                "warnings": warnings,
            })))
        }
        Op::UpdateSmartObject {
            bundle_id,
            html,
            steps,
            network_hosts,
        } => {
            let Some(existing) = ctx.pkg.objects.get(bundle_id) else {
                let known: Vec<&str> = ctx.pkg.objects.keys().map(String::as_str).collect();
                return Err(LiveError::NotFound(format!(
                    "no smart object bundle `{bundle_id}` on this board (bundles: {}); call get_current_page for bundle ids",
                    if known.is_empty() {
                        "none".to_string()
                    } else {
                        known.join(", ")
                    }
                )));
            };
            let b = updated_bundle(existing, html, *steps, network_hosts.clone());
            let diagnostics = b.validate();
            if has_errors(&diagnostics) {
                return Err(LiveError::Diagnostics(diagnostics));
            }
            let new_steps = b.manifest.steps;
            let libs = b.manifest.libs.clone();
            let mut placements: Vec<(String, Item, ObjectData)> = Vec::new();
            for p in ctx.lesson.pages() {
                for it in ctx.lesson.items(&p.id)? {
                    if let ItemData::Object(o) = &it.data
                        && &o.bundle == bundle_id
                    {
                        placements.push((p.id.clone(), it.clone(), o.clone()));
                    }
                }
            }
            let new_id = unique_bundle_id(&b.manifest.title, html, &ctx.pkg.objects);
            ctx.pkg.objects.insert(new_id.clone(), b);
            let mut item_ids = Vec::new();
            for (page_id, item, o) in &placements {
                let data = ItemData::Object(ObjectData {
                    geom: o.geom,
                    bundle: new_id.clone(),
                    title: o.title.clone(),
                    step: o.step.min(new_steps),
                    state: None,
                });
                tag(ctx.lesson, msg);
                item_ids.push(ctx.lesson.replace_items(
                    page_id,
                    std::slice::from_ref(&item.id),
                    &data,
                    item.created,
                )?);
            }
            Ok(Outcome::changed(json!({
                "bundle_id": new_id,
                "previous_bundle_id": bundle_id,
                "item_ids": item_ids,
                "placements": placements.len(),
                "steps": new_steps,
                "libs": libs,
                "warnings": diagnostics,
            })))
        }
        Op::DeleteItem { page, item_id } => {
            let (idx, info) = page_ref(ctx.lesson, ctx.page_id, *page)?;
            let item = find_item(ctx.lesson, &info, idx, item_id)?;
            tag(ctx.lesson, msg);
            ctx.lesson
                .remove_items(&info.id, std::slice::from_ref(&item.id))?;
            Ok(Outcome::changed(json!({
                "item_id": item.id,
                "kind": item.data.kind(),
                "page": idx + 1,
            })))
        }
        Op::SetPageBackground {
            page,
            pattern,
            color,
        } => {
            let (idx, info) = page_ref(ctx.lesson, ctx.page_id, *page)?;
            let pat = pattern.map(Pattern::from).unwrap_or(info.pattern);
            let color = match color {
                Some(c) => board_color(c)?,
                None => info.bg_color,
            };
            tag(ctx.lesson, msg);
            ctx.lesson.set_page_background(&info.id, pat, color)?;
            Ok(Outcome::changed(json!({
                "page": idx + 1,
                "pattern": pat.as_str(),
                "background": color_hex(color),
            })))
        }
        Op::ReplaceItemsWithText {
            page,
            item_ids,
            text,
            x,
            y,
            size,
            color,
        } => {
            check_text(text)?;
            if item_ids.is_empty() {
                return Err(LiveError::Invalid(
                    "item_ids must list the items to replace (e.g. the ink strokes from get_current_page or get_selection)".into(),
                ));
            }
            if item_ids.len() > MAX_ITEMS_PER_OP {
                return Err(LiveError::Invalid(format!(
                    "at most {MAX_ITEMS_PER_OP} items per call"
                )));
            }
            let (idx, info) = page_ref(ctx.lesson, ctx.page_id, *page)?;
            let items = ctx.lesson.items(&info.id)?;
            let chosen: Vec<&Item> = items.iter().filter(|i| item_ids.contains(&i.id)).collect();
            let missing: Vec<&str> = item_ids
                .iter()
                .filter(|id| !items.iter().any(|i| &i.id == *id))
                .map(String::as_str)
                .collect();
            if !missing.is_empty() {
                return Err(LiveError::NotFound(format!(
                    "not on page {}: {}; call get_current_page for item ids",
                    idx + 1,
                    missing.join(", ")
                )));
            }
            let b = union_bounds(chosen.iter().copied()).unwrap_or_default();
            let frame = info.frame.unwrap_or(Frame::SLIDE_16_9);
            let lines = text.split('\n').count().max(1) as f64;
            let size = match size {
                Some(s) => positive("size", *s)?,
                None => ((b.h / lines) * 0.7)
                    .round()
                    .clamp(MIN_PROJECTOR_TEXT, 160.0),
            };
            let x = finite("x", x.unwrap_or(b.x))?;
            let y = finite("y", y.unwrap_or(b.y))?;
            let (est_w, _) = text_box(text, size, None, x, frame);
            let w = est_w.max(b.w.min((frame.w - x).max(size * 2.0)));
            let (w, h) = text_box(text, size, Some(w), x, frame);
            let color = match color {
                Some(c) => parse_color(c)?,
                None => chosen
                    .iter()
                    .find_map(|i| match &i.data {
                        ItemData::Stroke(s) if s.opacity >= 0.9 => Some(s.color),
                        ItemData::Text(t) => Some(t.color),
                        _ => None,
                    })
                    .unwrap_or_else(|| default_ink(info.bg_color)),
            };
            let geom = BoxGeom {
                x,
                y,
                w,
                h,
                rot: 0.0,
            };
            let data = ItemData::Text(TextData {
                latex: false,
                geom,
                text: text.clone(),
                size,
                color,
                stamp: None,
            });
            let ids: Vec<String> = chosen.iter().map(|i| i.id.clone()).collect();
            tag(ctx.lesson, msg);
            let item_id = ctx.lesson.replace_items(&info.id, &ids, &data, now)?;
            Ok(Outcome::changed(json!({
                "item_id": item_id,
                "removed": ids.len(),
                "page": idx + 1,
                "geometry": geometry_of(&geom),
            })))
        }
        Op::StepObject {
            page,
            item_id,
            step,
            delta,
        } => {
            let (idx, info) = page_ref(ctx.lesson, ctx.page_id, *page)?;
            let items = ctx.lesson.items(&info.id)?;
            let declared = |o: &ObjectData| {
                ctx.pkg
                    .objects
                    .get(&o.bundle)
                    .map_or(0, |b| b.manifest.steps)
            };
            let forward = step.is_some() || delta.unwrap_or(1) >= 0;
            let objects: Vec<(&Item, &ObjectData)> = items
                .iter()
                .filter_map(|i| match &i.data {
                    ItemData::Object(o) => Some((i, o)),
                    _ => None,
                })
                .collect();
            let (item, o) = match item_id {
                Some(id) => *objects.iter().find(|(i, _)| &i.id == id).ok_or_else(|| {
                    LiveError::NotFound(format!(
                        "no Smart Object `{id}` on page {}; call get_current_page for item ids",
                        idx + 1
                    ))
                })?,
                None => *objects
                    .iter()
                    .find(|(_, o)| {
                        if forward {
                            o.step < declared(o)
                        } else {
                            o.step > 0
                        }
                    })
                    .or(objects.first())
                    .ok_or_else(|| {
                        LiveError::NotFound(format!("page {} has no Smart Object", idx + 1))
                    })?,
            };
            let steps = declared(o);
            let max = if steps > 0 { steps } else { MAX_STEPS };
            let target =
                match step {
                    Some(s) => *s,
                    None => (i64::from(o.step) + i64::from(delta.unwrap_or(1)))
                        .clamp(0, i64::from(max)) as u32,
                }
                .min(max);
            let mut out = json!({
                "item_id": item.id,
                "page": idx + 1,
                "step": target,
                "steps": steps,
                "changed": target != o.step,
            });
            if target == o.step {
                out["note"] = json!("already at that step");
                return Ok(Outcome::read(out));
            }
            tag(ctx.lesson, msg);
            ctx.lesson.set_object_step(&info.id, &item.id, target)?;
            Ok(Outcome::changed(out))
        }
    }
}

// ------------------------------------------------------------------ pages

/// 0-based index of a page id.
pub fn page_index(lesson: &Lesson, page_id: &str) -> Option<usize> {
    lesson.pages().iter().position(|p| p.id == page_id)
}

/// Resolve a 1-based page number (default: the current page).
pub fn page_ref(lesson: &Lesson, current: &str, page: Option<u32>) -> Result<(usize, PageInfo)> {
    let pages = lesson.pages();
    let idx = match page {
        None => pages.iter().position(|p| p.id == current).unwrap_or(0),
        Some(n) => {
            let n = n as usize;
            if n == 0 || n > pages.len() {
                return Err(LiveError::Invalid(format!(
                    "page {n} does not exist; the board has {} page(s), numbered from 1",
                    pages.len()
                )));
            }
            n - 1
        }
    };
    pages
        .get(idx)
        .cloned()
        .map(|p| (idx, p))
        .ok_or_else(|| LiveError::Invalid("the board has no pages".into()))
}

/// A new page: the given look, else the look of the current page (a lesson
/// on a chalkboard stays on chalkboard, with its background image and page
/// branding when no pattern or colour is asked for).
pub(crate) fn new_page_spec(
    lesson: &Lesson,
    current: &str,
    title: &Option<String>,
    pattern: Option<PatternName>,
    color: &Option<String>,
) -> Result<PageSpec> {
    let cur = lesson.page(current).ok();
    let same_look = pattern.is_none() && color.is_none();
    let (background, branding) = match (&cur, same_look) {
        (Some(p), true) => (p.background.clone(), p.branding.clone()),
        _ => (None, None),
    };
    Ok(PageSpec {
        background,
        branding,
        title: title.clone().unwrap_or_default(),
        frame: Some(
            cur.as_ref()
                .and_then(|p| p.frame)
                .unwrap_or(Frame::SLIDE_16_9),
        ),
        pattern: pattern
            .map(Pattern::from)
            .or(cur.as_ref().map(|p| p.pattern))
            .unwrap_or_default(),
        bg_color: match color {
            Some(c) => board_color(c)?,
            None => cur.map_or(0xffffffff, |p| p.bg_color),
        },
    })
}

fn find_item(lesson: &Lesson, info: &PageInfo, idx: usize, id: &str) -> Result<Item> {
    match lesson.item(&info.id, id) {
        Ok(i) => Ok(i),
        Err(CoreError::ItemNotFound(_)) => Err(LiveError::NotFound(format!(
            "item `{id}` is not on page {}; call get_current_page to see item ids",
            idx + 1
        ))),
        Err(e) => Err(e.into()),
    }
}

/// Identity of a lesson, stable across saves and replicas:
/// `<created ms>-<peer that created it, hex>`. The board sends it so the
/// server can tell "same lesson, reconnecting" from "another lesson".
pub fn doc_key(lesson: &Lesson) -> String {
    let creator = lesson
        .loro()
        .get_map("meta")
        .get_last_editor("format")
        .unwrap_or(0);
    format!("{}-{creator:x}", lesson.created() as i64)
}

// --------------------------------------------------------------- outlines

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
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

fn r1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn geometry_of(g: &BoxGeom) -> Geometry {
    Geometry {
        x: r1(g.x),
        y: r1(g.y),
        w: r1(g.w),
        h: r1(g.h),
        rot: r1(g.rot),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Seconds since the item was made (handwriting "just now" is small).
    pub age_s: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FrameSize {
    pub w: f64,
    pub h: f64,
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
    /// `light` or `dark`, from the page colour: theme new content to match.
    pub scheme: &'static str,
    pub items: Vec<ItemOutline>,
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('…');
        t
    }
}

pub fn item_outline(item: &Item, pkg: &Package, now_ms: f64) -> ItemOutline {
    let mut o = ItemOutline {
        id: item.id.clone(),
        kind: item.data.kind(),
        geometry: Geometry::default(),
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
        group: item.group.clone(),
        age_s: ((now_ms - item.created) / 1000.0).max(0.0).round(),
    };
    match &item.data {
        ItemData::Stroke(s) => {
            let b = s.aabb();
            o.geometry = Geometry {
                x: r1(f64::from(b[0])),
                y: r1(f64::from(b[1])),
                w: r1(f64::from(b[2] - b[0])),
                h: r1(f64::from(b[3] - b[1])),
                rot: 0.0,
            };
            o.color = Some(color_hex(s.color));
            o.tool = Some(s.tool.clone());
            o.shape = s.shape.clone();
            o.points = Some(s.points.len());
        }
        ItemData::Text(t) => {
            o.geometry = geometry_of(&t.geom);
            o.text = Some(truncate(&t.text, OUTLINE_TEXT_CHARS));
            o.font_size = Some(t.size);
            o.color = Some(color_hex(t.color));
        }
        ItemData::Image(i) => {
            o.geometry = geometry_of(&i.geom);
            o.asset = Some(i.asset.clone());
            if let Some(m) = &i.media {
                o.title = Some(m.title.clone()).filter(|t| !t.is_empty());
                o.media = Some(m.clone());
            }
        }
        ItemData::Object(ob) => {
            o.geometry = geometry_of(&ob.geom);
            o.title = Some(ob.title.clone());
            o.bundle_id = Some(ob.bundle.clone());
            o.steps = pkg.objects.get(&ob.bundle).map(|b| b.manifest.steps);
            o.step = Some(ob.step);
        }
    }
    o
}

pub fn page_outline(
    pkg: &Package,
    lesson: &Lesson,
    idx: usize,
    p: &PageInfo,
    now_ms: f64,
) -> Result<PageOutline> {
    Ok(PageOutline {
        page: idx + 1,
        id: p.id.clone(),
        title: p.title.clone(),
        frame: p.frame.map(|f| FrameSize { w: f.w, h: f.h }),
        pattern: p.pattern.as_str(),
        background: color_hex(p.bg_color),
        scheme: scheme(p.bg_color),
        items: lesson
            .items(&p.id)?
            .iter()
            .map(|i| item_outline(i, pkg, now_ms))
            .collect(),
    })
}

pub(crate) fn union_bounds<'a>(items: impl Iterator<Item = &'a Item>) -> Option<BoxGeom> {
    let mut b: Option<[f32; 4]> = None;
    for it in items {
        let a = it.data.aabb();
        b = Some(match b {
            None => a,
            Some(c) => [
                c[0].min(a[0]),
                c[1].min(a[1]),
                c[2].max(a[2]),
                c[3].max(a[3]),
            ],
        });
    }
    b.map(|b| BoxGeom {
        x: f64::from(b[0]),
        y: f64::from(b[1]),
        w: f64::from(b[2] - b[0]),
        h: f64::from(b[3] - b[1]),
        rot: 0.0,
    })
}

// ---------------------------------------------------------------- helpers

fn warning(code: &'static str, message: String) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        code,
        message,
    }
}

fn has_errors(d: &[Diagnostic]) -> bool {
    d.iter().any(|d| d.severity == Severity::Error)
}

fn check_text(text: &str) -> Result<()> {
    if text.trim().is_empty() {
        return Err(LiveError::Invalid("text must not be empty".into()));
    }
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(LiveError::Invalid(format!(
            "text is longer than {MAX_TEXT_CHARS} characters; split it over several text boxes or pages"
        )));
    }
    Ok(())
}

pub(crate) fn finite(name: &str, v: f64) -> Result<f64> {
    if v.is_finite() && v.abs() < 1e7 {
        Ok(v)
    } else {
        Err(LiveError::Invalid(format!(
            "{name} must be a finite number"
        )))
    }
}

pub(crate) fn positive(name: &str, v: f64) -> Result<f64> {
    if v.is_finite() && v > 0.0 && v < 1e6 {
        Ok(v)
    } else {
        Err(LiveError::Invalid(format!(
            "{name} must be a positive number"
        )))
    }
}

/// `#rrggbb` or `#rrggbbaa` → `0xRRGGBBAA`.
pub fn parse_color(s: &str) -> Result<u32> {
    let t = s.trim();
    let hex = t.strip_prefix('#').unwrap_or(t);
    let bad = || LiveError::Invalid(format!("color must look like #rrggbb, got `{s}`"));
    if !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(bad());
    }
    match hex.len() {
        6 => Ok((u32::from_str_radix(hex, 16).map_err(|_| bad())? << 8) | 0xff),
        8 => u32::from_str_radix(hex, 16).map_err(|_| bad()),
        _ => Err(bad()),
    }
}

/// The board's page colours by name (as in the pages panel), or `#rrggbb`.
pub fn board_color(s: &str) -> Result<u32> {
    match s.trim().to_ascii_lowercase().as_str() {
        "white" => Ok(0xffffffff),
        "paper" => Ok(0xfbf7edff),
        "chalkboard" | "green" => Ok(0x1f3b30ff),
        "blackboard" | "black" => Ok(0x17191cff),
        _ => parse_color(s).map_err(|_| {
            LiveError::Invalid(format!(
                "page colour must be white, paper, chalkboard, blackboard or #rrggbb, got `{s}`"
            ))
        }),
    }
}

pub fn color_hex(c: u32) -> String {
    if c & 0xff == 0xff {
        format!("#{:06x}", c >> 8)
    } else {
        format!("#{c:08x}")
    }
}

/// Relative luminance below 0.25 counts as a dark page (same rule as the
/// Smart Object theme in the board app).
pub fn is_dark(rgba: u32) -> bool {
    let ch = |shift: u32| {
        let c = f64::from((rgba >> shift) & 0xff) / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * ch(24) + 0.7152 * ch(16) + 0.0722 * ch(8) < 0.25
}

fn scheme(rgba: u32) -> &'static str {
    if is_dark(rgba) { "dark" } else { "light" }
}

/// Default text colour on a page: near-black on light pages, near-white on
/// chalkboard and blackboard.
pub fn default_ink(bg: u32) -> u32 {
    if is_dark(bg) { 0xf1f3f5ff } else { 0x1c2024ff }
}

/// Rough text box size: ~0.55em per character, 1.3em line height (the same
/// estimate the file tools use).
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

fn placement_warning(g: &BoxGeom, frame: Option<Frame>) -> Option<Diagnostic> {
    let f = frame?;
    let tol = 0.5;
    let outside = g.x < -tol || g.y < -tol || g.x + g.w > f.w + tol || g.y + g.h > f.h + tol;
    outside.then(|| {
        warning(
            "placement",
            format!(
                "the item ({:.0},{:.0} {:.0}×{:.0}) extends beyond the {:.0}×{:.0} slide frame and will be cut off when presenting",
                g.x, g.y, g.w, g.h, f.w, f.h
            ),
        )
    })
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

/// A copy of `existing` with new entry HTML (and optionally steps / hosts);
/// `rupoklib://` libraries are detected again from the new HTML.
fn updated_bundle(
    existing: &ObjectBundle,
    html: &str,
    steps: Option<u32>,
    network: Option<Vec<String>>,
) -> ObjectBundle {
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
    b
}

fn slug(title: &str) -> String {
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

fn fnv1a(bytes: &[u8], seed: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ seed;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// `<slug>-<4 base36 chars>` from the title and HTML, unique among `existing`
/// (no clock or randomness: this runs in wasm).
pub fn unique_bundle_id(
    title: &str,
    html: &str,
    existing: &BTreeMap<String, ObjectBundle>,
) -> String {
    const D: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let base = slug(title);
    for seed in 0..10_000u64 {
        let mut v = fnv1a(html.as_bytes(), seed);
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

#[cfg(test)]
mod tests {
    use super::*;
    use rupok_ink::StrokePoint;

    const HTML: &str = r#"<!doctype html><html><head><title>Bars</title>
<style>body{margin:0;font:32px system-ui}</style></head><body><div id="b"></div>
<script>rupok.steps.onChange(s => { document.getElementById("b").textContent = "Step " + s; });</script>
</body></html>"#;

    struct Board {
        pkg: Package,
        lesson: Lesson,
        page: String,
        selection: Vec<String>,
        read_only: bool,
    }

    impl Board {
        fn new() -> Self {
            let lesson = Lesson::new("Live", 1_000.0);
            let pkg = Package::from_lesson(&lesson, "test", 1_000.0);
            let page = lesson.page_id_at(0).unwrap();
            Self {
                pkg,
                lesson,
                page,
                selection: Vec::new(),
                read_only: false,
            }
        }
        fn run(&mut self, op: Op) -> Result<Outcome> {
            let out = apply(
                BoardCtx {
                    pkg: &mut self.pkg,
                    lesson: &mut self.lesson,
                    page_id: &self.page,
                    selection: &self.selection,
                    read_only: self.read_only,
                },
                &op,
                "Claude Code",
                5_000.0,
            )?;
            if let Some(i) = out.show_page {
                self.page = self.lesson.page_id_at(i).unwrap();
            }
            Ok(out)
        }
        fn items(&self) -> Vec<Item> {
            self.lesson.items(&self.page).unwrap()
        }
        fn stroke(&mut self) -> String {
            let pts = (0..20)
                .map(|i| StrokePoint {
                    x: 100.0 + i as f32 * 10.0,
                    y: 200.0 + (i % 3) as f32 * 8.0,
                    p: 0.5,
                    dt: i * 8,
                })
                .collect();
            let data = ItemData::Stroke(rupok_core::StrokeData {
                tool: "pen".into(),
                color: 0x3e63ddff,
                width: 4.0,
                opacity: 1.0,
                points: pts,
                offset: [0.0, 0.0],
                shape: None,
                geom: None,
                brush: None,
            });
            self.lesson
                .add_item(&self.page.clone(), &data, 4_000.0)
                .unwrap()
        }
    }

    fn text(t: &str) -> Op {
        Op::AddText {
            page: None,
            text: t.into(),
            x: 100.0,
            y: 80.0,
            w: None,
            size: Some(48.0),
            color: None,
        }
    }

    fn last_message(l: &Lesson) -> Option<String> {
        let f = l.loro().oplog_frontiers();
        let id = f.iter().next()?;
        l.loro().get_change(id)?.message.map(|m| m.to_string())
    }

    #[test]
    fn every_write_is_one_attributed_undo_step() {
        let mut b = Board::new();
        let out = b.run(text("Practice problems")).unwrap();
        assert!(out.changed);
        let id = out.result["item_id"].as_str().unwrap().to_string();
        assert_eq!(b.items().len(), 1);
        let msg = last_message(&b.lesson).unwrap();
        assert_eq!(
            crate::parse_commit_message(&msg),
            Some(("Claude Code".into(), "add_text".into()))
        );
        assert!(b.lesson.can_undo());
        assert!(b.lesson.undo().unwrap());
        assert!(b.items().is_empty(), "one undo removes the agent's text");
        assert!(b.lesson.redo().unwrap());
        assert_eq!(b.items()[0].id, id);

        // a failed op leaves no tag behind for the teacher's next change
        let err = b
            .run(Op::AddSmartObject {
                page: None,
                html: r#"<script src="https://cdn.example/x.js"></script>"#.into(),
                title: "Bad".into(),
                width: None,
                height: None,
                steps: None,
                x: None,
                y: None,
                network_hosts: None,
            })
            .unwrap_err();
        assert!(matches!(err, LiveError::Diagnostics(_)));
        assert_eq!(err.to_json()["saved"], false);
        b.stroke();
        assert_eq!(
            last_message(&b.lesson),
            None,
            "teacher stroke stays untagged"
        );
    }

    fn op(v: Value) -> Op {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn page_design_and_branding_by_an_agent_are_one_attributed_step_each() {
        let mut b = Board::new();
        b.run(Op::AddPage {
            title: None,
            index: None,
            pattern: None,
            color: None,
        })
        .unwrap();
        let svg = r##"<svg viewBox="0 0 1920 1080"><rect width="1920" height="1080" fill="#1f3b30"/><circle cx="1700" cy="900" r="120" fill="#ffffff" opacity=".06"/></svg>"##;
        let out = b
            .run(op(json!({"op": "set_page_design", "color": "chalkboard", "pattern": "grid", "background_svg": svg, "all_pages": true})))
            .unwrap();
        assert_eq!(out.result["pages_changed"], 2);
        assert_eq!(out.result["scheme"], "dark");
        assert_eq!(out.result["background_image"]["fit"], "stretch");
        let pages = b.lesson.pages();
        assert!(
            pages
                .iter()
                .all(|p| p.bg_color == 0x1f3b30ff && p.background.is_some())
        );
        let image = pages[0].background.as_ref().unwrap().image.clone();
        assert!(image.ends_with(".svg") && b.pkg.assets.contains_key(&image));
        assert_eq!(
            crate::parse_commit_message(&last_message(&b.lesson).unwrap()),
            Some(("Claude Code".into(), "set_page_design".into()))
        );
        // Only the fit: the image stays.
        b.run(op(
            json!({"op": "set_page_design", "background_fit": "tile"}),
        ))
        .unwrap();
        assert_eq!(
            b.lesson
                .page(&b.page.clone())
                .unwrap()
                .background
                .unwrap()
                .fit,
            rupok_core::BgFit::Tile
        );
        assert!(b.lesson.undo().unwrap());
        assert!(b.lesson.undo().unwrap());
        assert!(
            b.lesson.pages().iter().all(|p| p.background.is_none()),
            "one undo, every page"
        );

        // Unsafe SVG changes nothing.
        let err = b
            .run(op(
                json!({"op": "set_page_design", "background_svg": "<svg onload=\"x()\"/>"}),
            ))
            .unwrap_err();
        assert!(err.to_string().contains("event attribute"), "{err}");
        assert!(b.lesson.pages().iter().all(|p| p.background.is_none()));

        // Branding: a logo in the top-right corner and a footer with placeholders.
        let logo =
            r##"<svg viewBox="0 0 200 100"><rect width="200" height="100" fill="#e5484d"/></svg>"##;
        let out = b
            .run(op(json!({"op": "set_branding", "logo_svg": logo, "logo_corner": "top-right", "logo_width": 0.1,
                "school": "Dhaka Model School", "class": "Class 9 · {lesson}", "text_position": "bottom"})))
            .unwrap();
        let br = b.lesson.branding().unwrap();
        assert!(
            (br.logo_x - 0.875).abs() < 1e-9 && (br.logo_y - 0.035).abs() < 1e-9,
            "{out:?}"
        );
        assert_eq!(br.class, "Class 9 · {lesson}");
        assert_eq!(out.result["lesson"]["school"], "Dhaka Model School");
        // A wider logo stays in its corner.
        b.run(op(json!({"op": "set_branding", "logo_width": 0.2})))
            .unwrap();
        let br = b.lesson.branding().unwrap();
        assert!((br.logo_x - 0.775).abs() < 1e-9);
        // Hidden on page 2 only.
        b.run(op(json!({"op": "set_branding", "page": 2, "hidden": true})))
            .unwrap();
        let p2 = b.lesson.pages()[1].clone();
        assert_eq!(b.lesson.branding_for(&p2), None);
        let read = b.run(op(json!({"op": "set_branding"}))).unwrap();
        assert!(!read.changed && read.result["lesson"]["logo"].is_string());
        for bad in [
            json!({"op": "set_branding", "logo_corner": "middle"}),
            json!({"op": "set_branding", "logo_width": 3}),
            json!({"op": "set_branding", "scope": "class"}),
            json!({"op": "set_page_design", "background_fit": "tile"}),
            json!({"op": "set_branding", "logo_image": "aGVsbG8="}),
        ] {
            assert!(b.run(op(bad.clone())).is_err(), "{bad}");
        }
    }

    #[test]
    fn media_and_models_from_agents() {
        let mut b = Board::new();
        let out = b
            .run(Op::AddMedia {
                page: None,
                url: Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=42".into()),
                file_base64: None,
                file_name: None,
                title: Some("Photosynthesis".into()),
                start: None,
                end: Some(120.0),
                muted: Some(true),
                looped: None,
                autoplay: None,
                x: None,
                y: None,
                width: None,
                height: None,
            })
            .unwrap();
        assert_eq!(out.result["kind"], "youtube");
        let id = out.result["item_id"].as_str().unwrap().to_string();
        let page = b.lesson.page_id_at(0).unwrap();
        let ItemData::Image(img) = b.lesson.item(&page, &id).unwrap().data else {
            panic!("not an image")
        };
        let m = img.media.unwrap();
        assert_eq!((m.start, m.end, m.muted), (Some(42.0), Some(120.0), true));
        assert!(img.asset.ends_with(".svg") && b.pkg.assets.contains_key(&img.asset));
        assert!(last_message(&b.lesson).unwrap().contains("add_media"));

        let err = b
            .run(Op::AddMedia {
                page: None,
                url: Some("https://example.com/video".into()),
                file_base64: None,
                file_name: None,
                title: None,
                start: None,
                end: None,
                muted: None,
                looped: None,
                autoplay: None,
                x: None,
                y: None,
                width: None,
                height: None,
            })
            .unwrap_err();
        assert!(
            err.to_string().contains("not a link Rupok can embed"),
            "{err}"
        );

        let err = b
            .run(Op::AddModel {
                page: None,
                file_base64: "PGh0bWw+".into(),
                file_name: Some("x.glb".into()),
                title: None,
                x: None,
                y: None,
                width: None,
                height: None,
            })
            .unwrap_err();
        assert!(err.to_string().contains("glb"), "{err}");
    }

    #[test]
    fn smart_objects_add_update_undo() {
        let mut b = Board::new();
        let out = b
            .run(Op::AddSmartObject {
                page: Some(1),
                html: HTML.into(),
                title: "Fraction bars".into(),
                width: None,
                height: None,
                steps: Some(3),
                x: None,
                y: None,
                network_hosts: None,
            })
            .unwrap();
        let bundle = out.result["bundle_id"].as_str().unwrap().to_string();
        assert!(bundle.starts_with("fraction-bars-"), "{bundle}");
        assert_eq!(out.result["geometry"]["x"], 320.0);
        assert!(b.pkg.objects.contains_key(&bundle));

        // step forward twice, then past the end clamps
        let s = b
            .run(Op::StepObject {
                page: None,
                item_id: None,
                step: None,
                delta: None,
            })
            .unwrap();
        assert_eq!(s.result["step"], 1);
        let s = b
            .run(Op::StepObject {
                page: None,
                item_id: None,
                step: Some(9),
                delta: None,
            })
            .unwrap();
        assert_eq!(s.result["step"], 3);

        // update makes a new bundle and re-points the placement, undoably
        let html2 = HTML.replace("Step ", "Stage ");
        let up = b
            .run(Op::UpdateSmartObject {
                bundle_id: bundle.clone(),
                html: html2.clone(),
                steps: Some(2),
                network_hosts: None,
            })
            .unwrap();
        let new_bundle = up.result["bundle_id"].as_str().unwrap().to_string();
        assert_ne!(new_bundle, bundle);
        assert_eq!(up.result["placements"], 1);
        let ItemData::Object(o) = &b.items()[0].data else {
            panic!()
        };
        assert_eq!(o.bundle, new_bundle);
        assert_eq!(o.step, 2, "step clamped to the new count");
        assert_eq!(b.pkg.objects[&new_bundle].entry_html().unwrap(), html2);
        assert!(b.lesson.undo().unwrap());
        let ItemData::Object(o) = &b.items()[0].data else {
            panic!()
        };
        assert_eq!(o.bundle, bundle, "undo brings the old version back");
        assert!(b.pkg.objects.contains_key(&bundle));
    }

    #[test]
    fn ink_to_text_is_one_step() {
        let mut b = Board::new();
        let s1 = b.stroke();
        let s2 = b.stroke();
        b.selection = vec![s1.clone(), s2.clone()];
        let sel = b.run(Op::GetSelection).unwrap();
        assert_eq!(sel.result["count"], 2);
        let out = b
            .run(Op::ReplaceItemsWithText {
                page: None,
                item_ids: vec![s1.clone(), s2.clone()],
                text: "x² + 1".into(),
                x: None,
                y: None,
                size: None,
                color: None,
            })
            .unwrap();
        assert_eq!(out.result["removed"], 2);
        let items = b.items();
        assert_eq!(items.len(), 1);
        let ItemData::Text(t) = &items[0].data else {
            panic!()
        };
        assert_eq!(t.color, 0x3e63ddff, "keeps the ink colour");
        assert!(t.size >= MIN_PROJECTOR_TEXT);
        assert!(b.lesson.undo().unwrap());
        assert_eq!(b.items().len(), 2, "one undo brings both strokes back");
        let missing = b
            .run(Op::ReplaceItemsWithText {
                page: None,
                item_ids: vec!["nope".into()],
                text: "x".into(),
                x: None,
                y: None,
                size: None,
                color: None,
            })
            .unwrap_err();
        assert!(matches!(missing, LiveError::NotFound(_)));
    }

    #[test]
    fn pages_theme_and_reads() {
        let mut b = Board::new();
        b.run(Op::SetBoardTheme {
            page_color: Some("chalkboard".into()),
            pattern: Some(PatternName::Grid),
            ui: None,
        })
        .unwrap();
        let p = b.run(Op::GetPage { page: None }).unwrap();
        assert_eq!(p.result["scheme"], "dark");
        assert_eq!(p.result["pattern"], "grid");
        // new page inherits the look and can be shown
        let np = b
            .run(Op::AddPageAfterCurrent {
                title: Some("Practice".into()),
                pattern: None,
                color: None,
                show: true,
            })
            .unwrap();
        assert_eq!(np.show_page, Some(1));
        assert_eq!(np.result["page"], 2);
        let p = b.run(Op::GetPage { page: None }).unwrap();
        assert_eq!(p.result["page"], 2);
        assert_eq!(p.result["background"], "#1f3b30");
        // text defaults to light ink on a dark page
        b.run(text("Q1")).unwrap();
        let ItemData::Text(t) = &b.items()[0].data else {
            panic!()
        };
        assert_eq!(t.color, 0xf1f3f5ff);
        let g = b.run(Op::GoToPage { page: 1 }).unwrap();
        assert_eq!(g.show_page, Some(0));
        assert!(b.run(Op::GoToPage { page: 9 }).is_err());
        let o = b.run(Op::GetOutline).unwrap();
        assert_eq!(o.result["page_count"], 2);
        assert_eq!(o.result["pages"][1]["items"][0]["text"], "Q1");
        assert!(matches!(
            b.run(Op::PointAt {
                x: 0.0,
                y: 0.0,
                w: None,
                h: None,
                label: None,
                seconds: None
            }),
            Err(LiveError::Unsupported(_))
        ));
        // delete
        let id = b.run(text("gone")).unwrap().result["item_id"]
            .as_str()
            .unwrap()
            .to_string();
        let d = b
            .run(Op::DeleteItem {
                page: None,
                item_id: id.clone(),
            })
            .unwrap();
        assert_eq!(d.result["kind"], "text");
        assert!(
            b.run(Op::DeleteItem {
                page: None,
                item_id: id
            })
            .is_err()
        );
    }

    #[test]
    fn student_view_refuses_writes_but_allows_reads() {
        let mut b = Board::new();
        b.read_only = true;
        assert!(matches!(b.run(text("x")), Err(LiveError::ReadOnly)));
        assert!(b.run(Op::GetPage { page: None }).is_ok());
        assert!(b.run(Op::GoToPage { page: 1 }).is_ok());
    }

    #[test]
    fn validation_of_inputs() {
        let mut b = Board::new();
        assert!(b.run(text("   ")).is_err());
        let mut t = text("x");
        if let Op::AddText { x, .. } = &mut t {
            *x = f64::NAN;
        }
        assert!(b.run(t).is_err());
        assert!(parse_color("#12345").is_err());
        assert_eq!(board_color("Paper").unwrap(), 0xfbf7edff);
        assert!(board_color("purple").is_err());
        assert_eq!(
            doc_key(&b.lesson),
            doc_key(
                &Package::read(&{
                    b.pkg.update_lesson(&b.lesson, "t", 1.0);
                    b.pkg.write().unwrap()
                })
                .unwrap()
                .lesson()
                .unwrap()
            )
        );
    }
}
