//! Agent operations on pages and items: list, move, delete and copy pages;
//! select, move, align, restyle, scale, group, reorder and delete items; draw
//! ink and edit text.
//!
//! Each write is **one** Loro commit tagged with the agent (one undo step),
//! on a lesson file exactly as on the live board. Ink points are only
//! rewritten by `scale_items` (a scale is a new shape); a move changes the
//! stroke offset, as when the teacher drags it.

use std::collections::BTreeMap;

use rupok_core::{BoxGeom, Edits, Frame, Item, ItemData, NewItem, StrokeData};
use rupok_ink::{Brush, StrokePoint};
use serde_json::{Value, json};

use crate::apply::{
    BoardCtx, LiveError, Outcome, color_hex, finite, geometry_of, page_index, page_ref,
    parse_color, positive, tag, union_bounds,
};
use crate::op::{Op, StrokeArg};

type Result<T> = std::result::Result<T, LiveError>;

/// Most items one op may touch.
const MAX_ITEMS: usize = 5_000;
/// Most points in all the strokes of one `draw_stroke`.
const MAX_POINTS: usize = 50_000;
const MAX_STROKES: usize = 500;
const MIN_WIDTH: f64 = 0.5;
const MAX_WIDTH: f64 = 200.0;

pub(crate) fn run(ctx: &mut BoardCtx<'_>, op: &Op, msg: &str, now: f64) -> Result<Outcome> {
    match op {
        Op::ListPages => list_pages(ctx),
        Op::MovePages { pages, to } => move_pages(ctx, pages, *to, msg),
        Op::DeletePages { pages } => delete_pages(ctx, pages, msg),
        Op::DuplicatePages { pages } => duplicate_pages(ctx, pages, msg),
        Op::SetPageTitle { page, title } => {
            if title.chars().count() > 200 {
                return Err(LiveError::Invalid(
                    "title is longer than 200 characters".into(),
                ));
            }
            let (idx, info) = page_ref(ctx.lesson, ctx.page_id, *page)?;
            tag(ctx.lesson, msg);
            ctx.lesson.set_page_title(&info.id, title.trim())?;
            Ok(Outcome::changed(
                json!({ "page": idx + 1, "title": title.trim() }),
            ))
        }
        Op::SelectItems { page, item_ids } => {
            let (idx, _, chosen) = resolve_allow_empty(ctx, *page, item_ids)?;
            let ids: Vec<String> = chosen.iter().map(|i| i.id.clone()).collect();
            let current = page_index(ctx.lesson, ctx.page_id).unwrap_or(0);
            Ok(Outcome {
                result: json!({
                    "page": idx + 1,
                    "selected": ids.len(),
                    "bounds": union_bounds(chosen.iter()).map(|b| geometry_of(&b)),
                }),
                changed: false,
                show_page: (idx != current).then_some(idx),
                select: Some(ids),
            })
        }
        Op::MoveItems {
            page,
            item_ids,
            dx,
            dy,
            x,
            y,
        } => {
            let (idx, info, chosen) = resolve(ctx, *page, item_ids)?;
            let b = union_bounds(chosen.iter()).unwrap_or_default();
            let mut ddx = dx.map(|v| finite("dx", v)).transpose()?.unwrap_or(0.0);
            let mut ddy = dy.map(|v| finite("dy", v)).transpose()?.unwrap_or(0.0);
            if let Some(x) = x {
                ddx = finite("x", *x)? - b.x;
            }
            if let Some(y) = y {
                ddy = finite("y", *y)? - b.y;
            }
            if dx.is_none() && dy.is_none() && x.is_none() && y.is_none() {
                return Err(LiveError::Invalid(
                    "give dx/dy (move by) or x/y (move the items' top-left to)".into(),
                ));
            }
            let e = Edits {
                translate: chosen.iter().map(|i| (i.id.clone(), ddx, ddy)).collect(),
                ..Default::default()
            };
            commit(ctx, &info.id, &e, msg)?;
            let moved = BoxGeom {
                x: b.x + ddx,
                y: b.y + ddy,
                ..b
            };
            Ok(Outcome::changed(json!({
                "page": idx + 1,
                "moved": chosen.len(),
                "bounds": geometry_of(&moved),
            })))
        }
        Op::AlignItems {
            page,
            item_ids,
            align,
            distribute,
            relative_to,
        } => align_items(
            ctx,
            *page,
            item_ids,
            align.as_deref(),
            distribute.as_deref(),
            relative_to.as_deref(),
            msg,
        ),
        Op::RestyleItems { .. } => restyle(ctx, op, msg),
        Op::ScaleItems {
            page,
            item_ids,
            factor,
        } => scale(ctx, *page, item_ids, *factor, msg),
        Op::GroupItems { page, item_ids } => {
            let (idx, info, chosen) = resolve(ctx, *page, item_ids)?;
            if chosen.len() < 2 {
                return Err(LiveError::Invalid("group at least two items".into()));
            }
            let ids: Vec<String> = chosen.iter().map(|i| i.id.clone()).collect();
            tag(ctx.lesson, msg);
            let group = ctx.lesson.group_items(&info.id, &ids)?;
            Ok(Outcome::changed(
                json!({ "page": idx + 1, "group": group, "items": ids.len() }),
            ))
        }
        Op::UngroupItems { page, item_ids } => {
            let (idx, info, chosen) = resolve(ctx, *page, item_ids)?;
            let groups: Vec<&String> = chosen.iter().filter_map(|i| i.group.as_ref()).collect();
            if groups.is_empty() {
                return Err(LiveError::Invalid(
                    "none of these items is in a group".into(),
                ));
            }
            let all = ctx.lesson.items(&info.id)?;
            let members: Vec<String> = all
                .iter()
                .filter(|i| i.group.as_ref().is_some_and(|g| groups.contains(&g)))
                .map(|i| i.id.clone())
                .collect();
            tag(ctx.lesson, msg);
            ctx.lesson.set_group(&info.id, &members, None)?;
            Ok(Outcome::changed(
                json!({ "page": idx + 1, "ungrouped": members.len() }),
            ))
        }
        Op::ReorderItems { page, item_ids, to } => {
            let (idx, info, chosen) = resolve(ctx, *page, item_ids)?;
            let back = match to.as_deref().unwrap_or("front") {
                "front" => false,
                "back" => true,
                other => {
                    return Err(LiveError::Invalid(format!(
                        "to must be front or back, got `{other}`"
                    )));
                }
            };
            let chosen_ids: Vec<String> = chosen.iter().map(|i| i.id.clone()).collect();
            // Sending to the back raises everything else, in order: the same
            // stacking, without a "lowest z" in the format.
            let raise: Vec<String> = if back {
                ctx.lesson
                    .items(&info.id)?
                    .into_iter()
                    .filter(|i| !chosen_ids.contains(&i.id))
                    .map(|i| i.id)
                    .collect()
            } else {
                chosen_ids.clone()
            };
            if !raise.is_empty() {
                tag(ctx.lesson, msg);
                ctx.lesson.bring_items_to_front(&info.id, &raise)?;
            }
            Ok(Outcome::changed(json!({
                "page": idx + 1,
                "items": chosen_ids.len(),
                "to": if back { "back" } else { "front" },
            })))
        }
        Op::DeleteItems { page, item_ids } => {
            let (idx, info, chosen) = resolve(ctx, *page, item_ids)?;
            let ids: Vec<String> = chosen.iter().map(|i| i.id.clone()).collect();
            tag(ctx.lesson, msg);
            let n = ctx.lesson.remove_items(&info.id, &ids)?;
            Ok(Outcome::changed(json!({ "page": idx + 1, "deleted": n })))
        }
        Op::DrawStroke { .. } => draw(ctx, op, msg, now),
        Op::UpdateText {
            page,
            item_id,
            text,
            size,
            color,
            x,
            y,
            w,
        } => {
            let (idx, info, chosen) = resolve(ctx, *page, std::slice::from_ref(item_id))?;
            let item = &chosen[0];
            let ItemData::Text(t) = &item.data else {
                return Err(LiveError::Invalid(format!(
                    "item `{item_id}` is a {}, not a text box",
                    item.data.kind()
                )));
            };
            let mut e = Edits::default();
            if let Some(s) = text {
                crate::apply::check_text(s)?;
                if *s != t.text {
                    e.texts.push((item.id.clone(), s.clone()));
                }
            }
            if let Some(c) = color {
                let c = parse_color(c)?;
                if c != t.color {
                    e.text_colors.push((item.id.clone(), c));
                }
            }
            let mut g = t.geom;
            let mut new_size = None;
            if let Some(s) = size {
                let s = positive("size", *s)?.clamp(4.0, 2000.0);
                let k = s / t.size.max(1.0);
                g.w = (g.w * k).max(1.0);
                g.h = (g.h * k).max(1.0);
                new_size = Some(s);
            }
            if let Some(v) = x {
                g.x = finite("x", *v)?;
            }
            if let Some(v) = y {
                g.y = finite("y", *v)?;
            }
            if let Some(v) = w {
                g.w = positive("w", *v)?;
            }
            if g != t.geom || new_size.is_some() {
                e.boxes.push((item.id.clone(), g, new_size));
            }
            if e.is_empty() {
                return Err(LiveError::Invalid(
                    "nothing to change: give text, size, color, x, y or w".into(),
                ));
            }
            commit(ctx, &info.id, &e, msg)?;
            Ok(Outcome::changed(json!({
                "page": idx + 1,
                "item_id": item.id,
                "geometry": geometry_of(&g),
            })))
        }
        _ => Err(LiveError::Unsupported(op.name())),
    }
}

// ------------------------------------------------------------------ pages

fn list_pages(ctx: &BoardCtx<'_>) -> Result<Outcome> {
    let current = page_index(ctx.lesson, ctx.page_id).unwrap_or(0);
    let mut pages = Vec::new();
    for (i, p) in ctx.lesson.pages().iter().enumerate() {
        let items = ctx.lesson.items(&p.id)?;
        let count = |k: &str| items.iter().filter(|it| it.data.kind() == k).count();
        let title = if p.title.is_empty() {
            items.iter().find_map(|it| match &it.data {
                ItemData::Text(t) if t.stamp.is_none() => Some(
                    t.text
                        .lines()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(60)
                        .collect::<String>(),
                ),
                _ => None,
            })
        } else {
            None
        };
        pages.push(json!({
            "page": i + 1,
            "id": p.id,
            "title": p.title,
            "first_text": title,
            "background": color_hex(p.bg_color),
            "pattern": p.pattern.as_str(),
            "items": items.len(),
            "strokes": count("stroke"),
            "texts": count("text"),
            "objects": count("object"),
            "images": count("image"),
        }));
    }
    Ok(Outcome::read(json!({
        "title": ctx.lesson.title(),
        "page_count": pages.len(),
        "current_page": current + 1,
        "pages": pages,
    })))
}

/// 1-based page numbers → page ids (deduplicated, in page order).
fn page_ids(ctx: &BoardCtx<'_>, pages: &[u32]) -> Result<Vec<(usize, String)>> {
    if pages.is_empty() {
        return Err(LiveError::Invalid(
            "pages must list at least one page number (1-based)".into(),
        ));
    }
    let all = ctx.lesson.pages();
    let mut out: Vec<(usize, String)> = Vec::new();
    for &n in pages {
        let i = n as usize;
        if i == 0 || i > all.len() {
            return Err(LiveError::Invalid(format!(
                "page {n} does not exist; the lesson has {} page(s), numbered from 1",
                all.len()
            )));
        }
        if !out.iter().any(|(j, _)| *j == i - 1) {
            out.push((i - 1, all[i - 1].id.clone()));
        }
    }
    out.sort_by_key(|(i, _)| *i);
    Ok(out)
}

fn move_pages(ctx: &mut BoardCtx<'_>, pages: &[u32], to: u32, msg: &str) -> Result<Outcome> {
    let ids = page_ids(ctx, pages)?;
    let n = ctx.lesson.page_count();
    let k = ids.len();
    if to == 0 || to as usize > n - k + 1 {
        return Err(LiveError::Invalid(format!(
            "to must be between 1 and {} (where the first moved page ends up)",
            n - k + 1
        )));
    }
    // `to` counts positions after the move; `move_pages` takes the gap
    // before page `gap` among the pages as they are now.
    let start = to as usize - 1;
    let others: Vec<usize> = (0..n)
        .filter(|i| !ids.iter().any(|(j, _)| j == i))
        .collect();
    let gap = others.get(start).copied().unwrap_or(n);
    let list: Vec<String> = ids.iter().map(|(_, id)| id.clone()).collect();
    tag(ctx.lesson, msg);
    ctx.lesson.move_pages(&list, gap)?;
    let now: Vec<usize> = list
        .iter()
        .filter_map(|id| page_index(ctx.lesson, id))
        .map(|i| i + 1)
        .collect();
    Ok(Outcome::changed(json!({
        "moved": k,
        "pages_now": now,
        "current_page": page_index(ctx.lesson, ctx.page_id).map(|i| i + 1),
    })))
}

fn delete_pages(ctx: &mut BoardCtx<'_>, pages: &[u32], msg: &str) -> Result<Outcome> {
    let ids = page_ids(ctx, pages)?;
    if ids.len() >= ctx.lesson.page_count() {
        return Err(LiveError::Invalid(
            "a lesson keeps at least one page; leave one page out".into(),
        ));
    }
    let list: Vec<String> = ids.iter().map(|(_, id)| id.clone()).collect();
    let showing_deleted = list.iter().any(|id| id == ctx.page_id);
    let first = ids[0].0;
    tag(ctx.lesson, msg);
    let n = ctx.lesson.remove_pages(&list)?;
    let count = ctx.lesson.page_count();
    Ok(Outcome {
        result: json!({ "deleted": n, "page_count": count }),
        changed: true,
        show_page: showing_deleted.then_some(first.min(count - 1)),
        select: None,
    })
}

fn duplicate_pages(ctx: &mut BoardCtx<'_>, pages: &[u32], msg: &str) -> Result<Outcome> {
    let ids = page_ids(ctx, pages)?;
    let list: Vec<String> = ids.iter().map(|(_, id)| id.clone()).collect();
    tag(ctx.lesson, msg);
    let copies = ctx.lesson.duplicate_pages(&list)?;
    let numbers: Vec<usize> = copies
        .iter()
        .filter_map(|id| page_index(ctx.lesson, id))
        .map(|i| i + 1)
        .collect();
    Ok(Outcome::changed(json!({
        "copies": numbers,
        "page_count": ctx.lesson.page_count(),
    })))
}

// ------------------------------------------------------------------ items

/// The page and the listed items on it; every id must be there.
fn resolve(
    ctx: &BoardCtx<'_>,
    page: Option<u32>,
    ids: &[String],
) -> Result<(usize, rupok_core::PageInfo, Vec<Item>)> {
    if ids.is_empty() {
        return Err(LiveError::Invalid(
            "item_ids must list items (ids from get_current_page or get_selection)".into(),
        ));
    }
    resolve_allow_empty(ctx, page, ids)
}

fn resolve_allow_empty(
    ctx: &BoardCtx<'_>,
    page: Option<u32>,
    ids: &[String],
) -> Result<(usize, rupok_core::PageInfo, Vec<Item>)> {
    if ids.len() > MAX_ITEMS {
        return Err(LiveError::Invalid(format!(
            "at most {MAX_ITEMS} items per call"
        )));
    }
    let (idx, info) = page_ref(ctx.lesson, ctx.page_id, page)?;
    let items = ctx.lesson.items(&info.id)?;
    let missing: Vec<&str> = ids
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
    let chosen = items.into_iter().filter(|i| ids.contains(&i.id)).collect();
    Ok((idx, info, chosen))
}

fn commit(ctx: &mut BoardCtx<'_>, page_id: &str, e: &Edits, msg: &str) -> Result<Vec<String>> {
    if e.is_empty() {
        return Ok(Vec::new());
    }
    tag(ctx.lesson, msg);
    Ok(ctx.lesson.apply_edits(page_id, e)?)
}

fn aabb(i: &Item) -> [f64; 4] {
    let a = i.data.aabb();
    [
        f64::from(a[0]),
        f64::from(a[1]),
        f64::from(a[2]),
        f64::from(a[3]),
    ]
}

fn align_items(
    ctx: &mut BoardCtx<'_>,
    page: Option<u32>,
    ids: &[String],
    align: Option<&str>,
    distribute: Option<&str>,
    relative_to: Option<&str>,
    msg: &str,
) -> Result<Outcome> {
    let (idx, info, chosen) = resolve(ctx, page, ids)?;
    if align.is_none() && distribute.is_none() {
        return Err(LiveError::Invalid(
            "give align (left, center, right, top, middle, bottom) and/or distribute (horizontal, vertical)".into(),
        ));
    }
    // Units: a group moves as one (handwritten words stay words).
    let mut units: BTreeMap<String, (Vec<String>, [f64; 4])> = BTreeMap::new();
    for it in &chosen {
        let key = it.group.clone().unwrap_or_else(|| format!("#{}", it.id));
        let b = aabb(it);
        let e = units.entry(key).or_insert((Vec::new(), b));
        e.0.push(it.id.clone());
        e.1 = [
            e.1[0].min(b[0]),
            e.1[1].min(b[1]),
            e.1[2].max(b[2]),
            e.1[3].max(b[3]),
        ];
    }
    let mut units: Vec<(Vec<String>, [f64; 4], [f64; 2])> = units
        .into_values()
        .map(|(i, b)| (i, b, [0.0, 0.0]))
        .collect();
    let frame = info.frame.unwrap_or(Frame::SLIDE_16_9);
    let target = match relative_to.unwrap_or("items") {
        "items" => {
            let mut t = units[0].1;
            for u in &units {
                t = [
                    t[0].min(u.1[0]),
                    t[1].min(u.1[1]),
                    t[2].max(u.1[2]),
                    t[3].max(u.1[3]),
                ];
            }
            t
        }
        "page" => [0.0, 0.0, frame.w, frame.h],
        other => {
            return Err(LiveError::Invalid(format!(
                "relative_to must be items or page, got `{other}`"
            )));
        }
    };
    if relative_to.unwrap_or("items") == "items" && units.len() < 2 && distribute.is_none() {
        return Err(LiveError::Invalid(
            "aligning needs two or more items (or groups); use relative_to: \"page\" to align one item on the slide".into(),
        ));
    }
    if let Some(a) = align {
        for u in &mut units {
            let b = u.1;
            match a {
                "left" => u.2[0] = target[0] - b[0],
                "right" => u.2[0] = target[2] - b[2],
                "center" => u.2[0] = (target[0] + target[2]) / 2.0 - (b[0] + b[2]) / 2.0,
                "top" => u.2[1] = target[1] - b[1],
                "bottom" => u.2[1] = target[3] - b[3],
                "middle" => u.2[1] = (target[1] + target[3]) / 2.0 - (b[1] + b[3]) / 2.0,
                other => {
                    return Err(LiveError::Invalid(format!(
                        "align must be left, center, right, top, middle or bottom, got `{other}`"
                    )));
                }
            }
        }
    }
    if let Some(d) = distribute {
        let axis = match d {
            "horizontal" => 0,
            "vertical" => 1,
            other => {
                return Err(LiveError::Invalid(format!(
                    "distribute must be horizontal or vertical, got `{other}`"
                )));
            }
        };
        if units.len() < 3 {
            return Err(LiveError::Invalid(
                "distributing needs three or more items (or groups)".into(),
            ));
        }
        let lo = |u: &(Vec<String>, [f64; 4], [f64; 2])| u.1[axis] + u.2[axis];
        let size = |u: &(Vec<String>, [f64; 4], [f64; 2])| u.1[axis + 2] - u.1[axis];
        units.sort_by(|a, b| lo(a).total_cmp(&lo(b)));
        let start = lo(&units[0]);
        let last = units.len() - 1;
        let end = lo(&units[last]) + size(&units[last]);
        let total: f64 = units.iter().map(size).sum();
        let gap = (end - start - total) / last as f64;
        let mut at = start;
        for u in &mut units {
            let cur = u.1[axis] + u.2[axis];
            u.2[axis] += at - cur;
            at += size(u) + gap;
        }
    }
    let mut e = Edits::default();
    for (ids, _, d) in &units {
        if d[0].abs() > 1e-6 || d[1].abs() > 1e-6 {
            for id in ids {
                e.translate.push((id.clone(), d[0], d[1]));
            }
        }
    }
    let moved = e.translate.len();
    commit(ctx, &info.id, &e, msg)?;
    Ok(Outcome::changed(json!({
        "page": idx + 1,
        "moved": moved,
        "units": units.len(),
    })))
}

/// A brush name an agent may store (`pen` clears it; `magic` is not saved).
pub(crate) fn brush_key(name: &str) -> Result<Option<String>> {
    let b = Brush::parse(name);
    if b == Brush::Magic || (b == Brush::Pen && name != "pen") {
        return Err(LiveError::Invalid(format!(
            "brush must be one of pen, ballpoint, fountain, marker, pencil, chalk, brush, dashed, dotted, neon, rainbow; got `{name}`"
        )));
    }
    Ok(b.key().map(str::to_string))
}

fn restyle(ctx: &mut BoardCtx<'_>, op: &Op, msg: &str) -> Result<Outcome> {
    let Op::RestyleItems {
        page,
        item_ids,
        color,
        width,
        brush,
        fill,
        opacity,
        dashed,
        text_size,
    } = op
    else {
        return Err(LiveError::Unsupported(op.name()));
    };
    let (idx, info, chosen) = resolve(ctx, *page, item_ids)?;
    let color = color.as_deref().map(parse_color).transpose()?;
    let width = width
        .map(|w| positive("width", w).map(|w| w.clamp(MIN_WIDTH, MAX_WIDTH) as f32))
        .transpose()?;
    let brush = brush.as_deref().map(brush_key).transpose()?;
    let fill = match fill.as_deref() {
        None => None,
        Some("none") | Some("") => Some(None),
        Some(c) => Some(Some(parse_color(c)?)),
    };
    let opacity = match opacity {
        Some(o) if o.is_finite() && (0.05..=1.0).contains(o) => Some(*o as f32),
        Some(_) => {
            return Err(LiveError::Invalid(
                "opacity must be between 0.05 and 1".into(),
            ));
        }
        None => None,
    };
    let text_size = text_size
        .map(|s| positive("text_size", s).map(|s| s.clamp(4.0, 2000.0)))
        .transpose()?;
    if color.is_none()
        && width.is_none()
        && brush.is_none()
        && fill.is_none()
        && opacity.is_none()
        && dashed.is_none()
        && text_size.is_none()
    {
        return Err(LiveError::Invalid(
            "nothing to change: give color, width, brush, fill, opacity, dashed or text_size"
                .into(),
        ));
    }
    let mut e = Edits::default();
    let mut skipped: Vec<Value> = Vec::new();
    for it in &chosen {
        match &it.data {
            ItemData::Stroke(s) => {
                let mut n = s.clone();
                if let Some(c) = color {
                    // Highlighter ink keeps its own translucency (opacity).
                    n.color = if s.tool == "highlighter" {
                        (c & 0xffff_ff00) | 0xff
                    } else {
                        c
                    };
                }
                if let Some(w) = width {
                    n.width = w;
                }
                if let Some(b) = &brush {
                    n.brush = b.clone();
                }
                if let Some(o) = opacity {
                    n.opacity = o;
                }
                if (fill.is_some() || dashed.is_some())
                    && let Some(g) = n.geom.as_mut()
                {
                    if let Some(f) = fill {
                        g.fill = f;
                    }
                    if let Some(d) = dashed {
                        g.dash = *d;
                    }
                }
                if n != *s {
                    e.strokes.push((it.id.clone(), n));
                }
            }
            ItemData::Text(t) => {
                let mut touched = false;
                if let Some(c) = color
                    && c != t.color
                {
                    e.text_colors.push((it.id.clone(), c));
                    touched = true;
                }
                if let Some(sz) = text_size
                    && t.stamp.is_none()
                    && (sz - t.size).abs() > 1e-6
                {
                    let k = sz / t.size.max(1.0);
                    let g = BoxGeom {
                        w: (t.geom.w * k).max(1.0),
                        h: (t.geom.h * k).max(1.0),
                        ..t.geom
                    };
                    e.boxes.push((it.id.clone(), g, Some(sz)));
                    touched = true;
                }
                if !touched {
                    skipped.push(json!({ "item_id": it.id, "kind": "text" }));
                }
            }
            other => skipped.push(json!({ "item_id": it.id, "kind": other.kind() })),
        }
    }
    if e.is_empty() {
        return Err(LiveError::Invalid(format!(
            "nothing changed: the items already look like that, or are images or Smart Objects, which have no ink style ({} skipped)",
            skipped.len()
        )));
    }
    let mut touched: Vec<&String> = e
        .strokes
        .iter()
        .map(|(id, _)| id)
        .chain(e.text_colors.iter().map(|(id, _)| id))
        .chain(e.boxes.iter().map(|(id, _, _)| id))
        .collect();
    touched.sort();
    touched.dedup();
    let restyled = touched.len();
    commit(ctx, &info.id, &e, msg)?;
    Ok(Outcome::changed(json!({
        "page": idx + 1,
        "restyled": restyled,
        "skipped": skipped,
    })))
}

/// Scale about the centre of the items' bounds. A uniform scale maps a
/// stroke exactly: points, width and shape geometry scale together, so a
/// rectangle stays a rectangle and an arrow keeps its head.
fn scale(
    ctx: &mut BoardCtx<'_>,
    page: Option<u32>,
    ids: &[String],
    factor: f64,
    msg: &str,
) -> Result<Outcome> {
    if !(factor.is_finite() && (0.05..=20.0).contains(&factor)) {
        return Err(LiveError::Invalid(
            "factor must be between 0.05 and 20 (1.5 = half as big again)".into(),
        ));
    }
    let (idx, info, chosen) = resolve(ctx, page, ids)?;
    let b = union_bounds(chosen.iter()).unwrap_or_default();
    let (cx, cy) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
    let k = factor;
    let map = |x: f64, y: f64| (cx + (x - cx) * k, cy + (y - cy) * k);
    let mut e = Edits::default();
    for it in &chosen {
        match &it.data {
            ItemData::Stroke(s) => {
                let (ox, oy) = (f64::from(s.offset[0]), f64::from(s.offset[1]));
                let points = s
                    .points
                    .iter()
                    .map(|p| {
                        let (x, y) = map(f64::from(p.x) + ox, f64::from(p.y) + oy);
                        StrokePoint {
                            x: x as f32,
                            y: y as f32,
                            ..*p
                        }
                    })
                    .collect();
                let geom = s.geom.as_ref().map(|g| {
                    let (x, y) = map(g.x + ox, g.y + oy);
                    rupok_core::ShapeGeom {
                        x,
                        y,
                        w: g.w * k,
                        h: g.h * k,
                        ..g.clone()
                    }
                });
                e.strokes.push((
                    it.id.clone(),
                    StrokeData {
                        points,
                        width: (f64::from(s.width) * k).clamp(MIN_WIDTH, MAX_WIDTH) as f32,
                        offset: [0.0, 0.0],
                        geom,
                        ..s.clone()
                    },
                ));
            }
            ItemData::Text(t) => {
                let (x, y) = map(t.geom.x, t.geom.y);
                let g = BoxGeom {
                    x,
                    y,
                    w: (t.geom.w * k).max(1.0),
                    h: (t.geom.h * k).max(1.0),
                    rot: t.geom.rot,
                };
                e.boxes
                    .push((it.id.clone(), g, Some((t.size * k).clamp(4.0, 2000.0))));
            }
            ItemData::Image(i) => e.boxes.push((it.id.clone(), scaled(&i.geom, map, k), None)),
            ItemData::Object(o) => e.boxes.push((it.id.clone(), scaled(&o.geom, map, k), None)),
        }
    }
    commit(ctx, &info.id, &e, msg)?;
    let nb = BoxGeom {
        x: cx - b.w * k / 2.0,
        y: cy - b.h * k / 2.0,
        w: b.w * k,
        h: b.h * k,
        rot: 0.0,
    };
    let mut warnings = Vec::new();
    warnings.extend(crate::apply::placement_warning(&nb, info.frame));
    Ok(Outcome::changed(json!({
        "page": idx + 1,
        "scaled": chosen.len(),
        "bounds": geometry_of(&nb),
        "warnings": warnings,
    })))
}

fn scaled(g: &BoxGeom, map: impl Fn(f64, f64) -> (f64, f64), k: f64) -> BoxGeom {
    let (x, y) = map(g.x, g.y);
    BoxGeom {
        x,
        y,
        w: (g.w * k).max(1.0),
        h: (g.h * k).max(1.0),
        rot: g.rot,
    }
}

fn draw(ctx: &mut BoardCtx<'_>, op: &Op, msg: &str, now: f64) -> Result<Outcome> {
    let Op::DrawStroke {
        page,
        points,
        strokes,
        color,
        width,
        brush,
        opacity,
        tool,
    } = op
    else {
        return Err(LiveError::Unsupported(op.name()));
    };
    let mut list: Vec<StrokeArg> = strokes.clone().unwrap_or_default();
    if let Some(p) = points {
        list.insert(
            0,
            StrokeArg {
                points: p.clone(),
                ..Default::default()
            },
        );
    }
    if list.is_empty() {
        return Err(LiveError::Invalid(
            "give points ([[x, y], …] in slide pixels) or strokes".into(),
        ));
    }
    if list.len() > MAX_STROKES {
        return Err(LiveError::Invalid(format!(
            "at most {MAX_STROKES} strokes per call"
        )));
    }
    let total: usize = list.iter().map(|s| s.points.len()).sum();
    if total > MAX_POINTS {
        return Err(LiveError::Invalid(format!(
            "at most {MAX_POINTS} points per call"
        )));
    }
    let highlighter = match tool.as_deref().unwrap_or("pen") {
        "pen" => false,
        "highlighter" => true,
        other => {
            return Err(LiveError::Invalid(format!(
                "tool must be pen or highlighter, got `{other}`"
            )));
        }
    };
    let (idx, info) = page_ref(ctx.lesson, ctx.page_id, *page)?;
    let def_color = match color {
        Some(c) => parse_color(c)?,
        None if highlighter => 0xffe629ff,
        None => crate::apply::default_ink(info.bg_color),
    };
    let def_width = width
        .map(|w| positive("width", w))
        .transpose()?
        .unwrap_or(if highlighter { 22.0 } else { 4.0 });
    let def_brush = brush.as_deref().map(brush_key).transpose()?.flatten();
    let opacity = match opacity {
        Some(o) if o.is_finite() && (0.05..=1.0).contains(o) => *o as f32,
        Some(_) => {
            return Err(LiveError::Invalid(
                "opacity must be between 0.05 and 1".into(),
            ));
        }
        None if highlighter => 0.38,
        None => 1.0,
    };
    let mut add = Vec::with_capacity(list.len());
    for (si, s) in list.iter().enumerate() {
        if s.points.is_empty() {
            return Err(LiveError::Invalid(format!(
                "stroke {} has no points",
                si + 1
            )));
        }
        let c = match &s.color {
            Some(c) => parse_color(c)?,
            None => def_color,
        };
        let w = match s.width {
            Some(w) => positive("width", w)?,
            None => def_width,
        }
        .clamp(MIN_WIDTH, MAX_WIDTH) as f32;
        let b = match &s.brush {
            Some(b) => brush_key(b)?,
            None => def_brush.clone(),
        };
        let mut pts = Vec::with_capacity(s.points.len());
        let mut last: Option<(f64, f64)> = None;
        let mut t = 0u32;
        for (pi, p) in s.points.iter().enumerate() {
            if p.len() < 2 || p.len() > 3 {
                return Err(LiveError::Invalid(format!(
                    "stroke {} point {}: give [x, y] or [x, y, pressure]",
                    si + 1,
                    pi + 1
                )));
            }
            let x = finite("x", p[0])?;
            let y = finite("y", p[1])?;
            let pr = p.get(2).copied().unwrap_or(0.5);
            if !(pr.is_finite() && (0.0..=1.0).contains(&pr)) {
                return Err(LiveError::Invalid(format!(
                    "stroke {} point {}: pressure must be between 0 and 1",
                    si + 1,
                    pi + 1
                )));
            }
            // A steady hand: 1 ms per 1.5 px, so replay draws it naturally.
            if let Some((lx, ly)) = last {
                t += (((x - lx).hypot(y - ly)) / 1.5).clamp(1.0, 200.0) as u32;
            }
            last = Some((x, y));
            pts.push(StrokePoint {
                x: x as f32,
                y: y as f32,
                p: pr as f32,
                dt: t,
            });
        }
        add.push(NewItem {
            data: ItemData::Stroke(StrokeData {
                tool: if highlighter { "highlighter" } else { "pen" }.into(),
                color: c,
                width: w,
                opacity,
                points: pts,
                offset: [0.0, 0.0],
                shape: None,
                geom: None,
                brush: b,
            }),
            created: now + si as f64,
            group: None,
            z: None,
        });
    }
    let e = Edits {
        add,
        ..Default::default()
    };
    let ids = commit(ctx, &info.id, &e, msg)?;
    let added: Vec<Item> = ctx
        .lesson
        .items(&info.id)?
        .into_iter()
        .filter(|i| ids.contains(&i.id))
        .collect();
    Ok(Outcome::changed(json!({
        "page": idx + 1,
        "item_ids": ids,
        "bounds": union_bounds(added.iter()).map(|b| geometry_of(&b)),
    })))
}
