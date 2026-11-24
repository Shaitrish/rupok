//! `write_handwriting`: an agent writes on the board in handwriting.
//!
//! The lines (text with inline LaTeX) are synthesised by
//! [`rupok_ink::write`] into timed pen strokes, styled like the problem's
//! ink (colour, pen width, letter size), placed below (or right of) the
//! anchor where they cover nothing, and added as **one** commit: one undo
//! removes the whole solution, and the strokes are grouped so they select
//! and move together. Each stroke's `created` time is the moment it is drawn
//! in the performance (`now + start`), so replay writes it the same way the
//! board animates it.

use rupok_core::{Frame, Item, ItemData, NewItem, StrokeData};
use rupok_ink::StrokePoint;
use rupok_ink::write::{self, GlyphSet, WriteOptions, Written};
use serde_json::{Value, json};

use crate::apply::{
    BoardCtx, LiveError, Outcome, default_ink, finite, new_page_spec, page_ref, parse_color,
    positive, tag, union_bounds,
};
use crate::op::Op;

type Result<T> = std::result::Result<T, LiveError>;

/// Space kept free around the writing and from the slide edges, slide px.
const MARGIN: f64 = 40.0;
/// Most strokes one call may add.
const MAX_STROKES: usize = 4_000;
/// Default capital height when nothing on the board says otherwise.
const DEFAULT_CAP: f64 = 40.0;

/// How the problem is written, so the solution matches it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InkStyle {
    pub color: u32,
    pub width: f32,
    /// Height of a capital or digit, slide px.
    pub cap: f64,
}

/// Look at the problem's ink: the commonest opaque stroke colour, the median
/// pen width, and the letter height (the upper quartile of stroke heights:
/// a digit or a letter is usually one or two strokes; `=` and `−` are flat).
pub fn ink_style(items: &[&Item]) -> Option<InkStyle> {
    let mut colors: Vec<(u32, usize)> = Vec::new();
    let mut widths: Vec<f32> = Vec::new();
    let mut heights: Vec<f64> = Vec::new();
    let mut text_cap: Option<f64> = None;
    for it in items {
        match &it.data {
            ItemData::Stroke(s) if s.opacity >= 0.9 && s.shape.is_none() => {
                match colors.iter_mut().find(|(c, _)| *c == s.color) {
                    Some((_, n)) => *n += 1,
                    None => colors.push((s.color, 1)),
                }
                widths.push(s.width);
                let a = it.data.aabb();
                heights.push(f64::from(a[3] - a[1]));
            }
            ItemData::Text(t) => {
                text_cap.get_or_insert(t.size * 0.72);
                match colors.iter_mut().find(|(c, _)| *c == t.color) {
                    Some((_, n)) => *n += 1,
                    None => colors.push((t.color, 1)),
                }
            }
            _ => {}
        }
    }
    let color = colors.iter().max_by_key(|(_, n)| *n)?.0;
    widths.sort_by(f32::total_cmp);
    heights.sort_by(f64::total_cmp);
    let width = widths.get(widths.len() / 2).copied().unwrap_or(0.0);
    let cap = if heights.is_empty() {
        text_cap.unwrap_or(DEFAULT_CAP)
    } else {
        heights[(heights.len() * 3 / 4).min(heights.len() - 1)]
    };
    Some(InkStyle {
        color,
        width,
        cap: cap.clamp(16.0, 160.0),
    })
}

fn rect_overlap(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

/// The top-most free spot for a `w × h` block starting at (x, y) and moving
/// down past obstacles; `None` when it would leave the slide.
fn find_free(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    frame: Frame,
    obstacles: &[[f64; 4]],
) -> Option<(f64, f64)> {
    let mut y = y.max(MARGIN * 0.5);
    for _ in 0..200 {
        if y + h > frame.h - MARGIN * 0.5 || x + w > frame.w - MARGIN * 0.25 {
            return None;
        }
        let r = [x - 6.0, y - 6.0, x + w + 6.0, y + h + 6.0];
        match obstacles
            .iter()
            .filter(|o| rect_overlap(r, **o))
            .map(|o| o[3])
            .reduce(f64::max)
        {
            Some(bottom) => y = bottom + 12.0,
            None => return Some((x, y)),
        }
    }
    None
}

fn check_lines(lines: &[String]) -> Result<()> {
    if lines.iter().all(|l| l.trim().is_empty()) {
        return Err(LiveError::Invalid(
            "lines must contain at least one line to write, e.g. [\"$2x = 4$\", \"$x = 2$\"]"
                .into(),
        ));
    }
    if lines.len() > write::MAX_LINES {
        return Err(LiveError::Invalid(format!(
            "at most {} lines per call; split a long solution into steps",
            write::MAX_LINES
        )));
    }
    if let Some(l) = lines
        .iter()
        .find(|l| l.chars().count() > write::parse::MAX_LINE_CHARS)
    {
        return Err(LiveError::Invalid(format!(
            "a line is longer than {} characters; keep solution lines short ({}…)",
            write::parse::MAX_LINE_CHARS,
            l.chars().take(40).collect::<String>()
        )));
    }
    Ok(())
}

fn neat_glyphs() -> GlyphSet {
    GlyphSet::new()
}

/// Run a `write_handwriting` op.
pub fn run(
    ctx: &mut BoardCtx<'_>,
    op: &Op,
    msg: &str,
    now: f64,
    glyphs: Option<&GlyphSet>,
) -> Result<Outcome> {
    let Op::WriteHandwriting {
        page,
        lines,
        anchor,
        style,
        speed,
        color,
        size,
        step_mode,
    } = op
    else {
        return Err(LiveError::Invalid("not a write_handwriting op".into()));
    };
    check_lines(lines)?;
    let step_mode = match step_mode.as_deref() {
        None | Some("all") => "all",
        Some("one-line-per-click") | Some("one_line_per_click") | Some("step") => {
            "one-line-per-click"
        }
        Some(other) => {
            return Err(LiveError::Invalid(format!(
                "step_mode must be \"all\" or \"one-line-per-click\", not {other:?}"
            )));
        }
    };
    let speed = match speed {
        Some(s) => finite("speed", *s)?.clamp(0.25, 4.0),
        None => 1.0,
    };
    let own;
    let glyphs = match glyphs {
        Some(g) => g,
        None => {
            own = neat_glyphs();
            &own
        }
    };
    let teacher_available = glyphs.user_chars() > 0;
    let prefer_user = match style.as_deref() {
        None | Some("teacher") | Some("mine") => true,
        Some("neat") => false,
        Some(other) => {
            return Err(LiveError::Invalid(format!(
                "style must be \"teacher\" or \"neat\", not {other:?}"
            )));
        }
    };

    let (idx, info) = page_ref(ctx.lesson, ctx.page_id, *page)?;
    let frame = info.frame.unwrap_or(Frame::SLIDE_16_9);
    let items = ctx.lesson.items(&info.id)?;
    let anchor = anchor.clone().unwrap_or_default();

    // What we write next to: the anchor's items, else its region, else the
    // teacher's selection (on the board's page).
    let mut target_items: Vec<&Item> = Vec::new();
    if let Some(ids) = &anchor.item_ids {
        let missing: Vec<&str> = ids
            .iter()
            .filter(|id| !items.iter().any(|i| &i.id == *id))
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            return Err(LiveError::NotFound(format!(
                "not on page {}: {}; call get_current_page or get_selection for item ids",
                idx + 1,
                missing.join(", ")
            )));
        }
        target_items = items.iter().filter(|i| ids.contains(&i.id)).collect();
    } else if anchor.region.is_none() && anchor.x.is_none() && info.id == ctx.page_id {
        target_items = items
            .iter()
            .filter(|i| ctx.selection.contains(&i.id))
            .collect();
    }
    let target = match (&anchor.region, target_items.is_empty()) {
        (Some(r), _) => Some([r.x, r.y, r.x + r.w, r.y + r.h]),
        (None, false) => {
            union_bounds(target_items.iter().copied()).map(|b| [b.x, b.y, b.x + b.w, b.y + b.h])
        }
        _ => None,
    };
    for v in target.iter().flatten() {
        if !v.is_finite() {
            return Err(LiveError::Invalid(
                "the anchor region must be finite numbers".into(),
            ));
        }
    }

    // Match the problem's ink.
    let ink = ink_style(&target_items);
    let cap = match size {
        Some(s) => positive("size", *s)?.clamp(12.0, 200.0),
        None => ink.map_or(DEFAULT_CAP, |i| i.cap),
    };
    let x_height = (cap * 14.0 / 21.0) as f32;
    let color = match color {
        Some(c) => parse_color(c)?,
        None => ink.map_or_else(|| default_ink(info.bg_color), |i| i.color),
    };
    let width = ink
        .map(|i| i.width)
        .filter(|w| *w > 0.5)
        .unwrap_or((x_height * 0.16).clamp(2.5, 8.0));

    let joined = lines.join("\n");
    let synth = |max_w: f64, xh: f32| -> Written {
        write::write(
            lines,
            glyphs,
            &WriteOptions {
                x_height: xh,
                max_width: Some(max_w as f32),
                prefer_user,
                speed: speed as f32,
                seed: joined.len() as u64,
                ..WriteOptions::default()
            },
        )
    };

    let obstacles: Vec<[f64; 4]> = items
        .iter()
        .map(|i| {
            let a = i.data.aabb();
            [
                f64::from(a[0]),
                f64::from(a[1]),
                f64::from(a[2]),
                f64::from(a[3]),
            ]
        })
        .collect();

    // Where to write: an explicit point, else below/right of the target
    // (whichever has room, the asked side first), else a new page.
    let gap = f64::from(x_height) * 1.2;
    let mut placed: Option<(f64, f64, Written)> = None;
    let mut new_page = false;
    if let (Some(x), Some(y)) = (anchor.x, anchor.y) {
        let x = finite("anchor.x", x)?;
        let y = finite("anchor.y", y)?;
        let w = synth(
            (frame.w - x - MARGIN).max(f64::from(x_height) * 6.0),
            x_height,
        );
        placed = Some((x, y, w));
    } else {
        let t = target.unwrap_or([
            MARGIN * 2.0,
            MARGIN * 2.0 - gap,
            MARGIN * 2.0,
            MARGIN * 2.0 - gap,
        ]);
        let below = (t[0].clamp(MARGIN, frame.w - MARGIN * 4.0), t[3] + gap);
        let right = (t[2] + gap * 1.5, t[1]);
        let sides = if anchor.side.as_deref() == Some("right") {
            [right, below]
        } else {
            [below, right]
        };
        'search: for scale in [1.0f32, 0.8, 0.65] {
            for (x, y) in sides {
                let max_w = frame.w - x - MARGIN;
                if max_w < f64::from(x_height) * 8.0 {
                    continue;
                }
                let w = synth(max_w, x_height * scale);
                if let Some((fx, fy)) = find_free(
                    x,
                    y,
                    f64::from(w.width),
                    f64::from(w.height),
                    frame,
                    &obstacles,
                ) {
                    placed = Some((fx, fy, w));
                    break 'search;
                }
            }
        }
        if placed.is_none() {
            new_page = true;
        }
    }

    let mut page_id = info.id.clone();
    let mut page_no = idx + 1;
    if new_page {
        // No room on this page: continue on a new page after it.
        let spec = new_page_spec(ctx.lesson, &info.id, &Some("Solution".into()), None, &None)?;
        tag(ctx.lesson, msg);
        page_id = ctx.lesson.add_page(Some(idx + 1), spec)?;
        page_no = idx + 2;
        let w = synth(frame.w - MARGIN * 4.0, x_height);
        placed = Some((MARGIN * 2.0, MARGIN * 2.0, w));
    }
    let Some((x0, y0, written)) = placed else {
        return Err(LiveError::Invalid("could not place the writing".into()));
    };
    if written.strokes.len() > MAX_STROKES {
        return Err(LiveError::Invalid(format!(
            "that is {} strokes; write at most about 20 short lines per call",
            written.strokes.len()
        )));
    }

    let group = ctx.lesson.new_group_id();
    let (ox, oy) = (x0 as f32, y0 as f32);
    let list: Vec<NewItem> = written
        .strokes
        .iter()
        .map(|s| NewItem {
            data: ItemData::Stroke(StrokeData {
                tool: "pen".into(),
                color,
                width,
                opacity: 1.0,
                points: s
                    .points
                    .iter()
                    .map(|p| StrokePoint {
                        x: p.x + ox,
                        y: p.y + oy,
                        ..*p
                    })
                    .collect(),
                offset: [0.0, 0.0],
                shape: None,
                geom: None,
                brush: None,
            }),
            created: now + s.start_ms,
            group: Some(group.clone()),
            z: None,
        })
        .collect();
    tag(ctx.lesson, msg);
    let ids = ctx.lesson.add_items(&page_id, &list)?;

    let mut by_line = vec![0usize; lines.len()];
    for s in &written.strokes {
        if let Some(n) = by_line.get_mut(s.line) {
            *n += 1;
        }
    }
    let used_teacher = prefer_user && teacher_available;
    let mut result = json!({
        "page": page_no,
        "item_ids": ids,
        "group": group,
        "lines": lines.len(),
        "strokes_by_line": by_line,
        "bounds": {
            "x": (x0 * 10.0).round() / 10.0,
            "y": (y0 * 10.0).round() / 10.0,
            "w": (f64::from(written.width) * 10.0).round() / 10.0,
            "h": (f64::from(written.height) * 10.0).round() / 10.0,
        },
        "duration_ms": written.duration_ms.round(),
        "style": if used_teacher { "teacher" } else { "neat" },
        "step_mode": step_mode,
        "speed": speed,
        "ink": { "color": crate::apply::color_hex(color), "width": width, "letter_height": (cap * 10.0).round() / 10.0 },
    });
    let mut notes: Vec<String> = Vec::new();
    if new_page {
        notes.push(format!(
            "there was no free space on page {}, so the solution is on a new page {page_no}",
            idx + 1
        ));
    }
    if prefer_user && !teacher_available {
        notes.push(
            "the teacher has not taught Rupok their handwriting yet; written in the neat hand"
                .into(),
        );
    }
    if !written.missing.is_empty() {
        let m: String = written.missing.iter().collect();
        notes.push(format!("no handwriting glyph for {m:?}; written as ?"));
    }
    if step_mode == "one-line-per-click" {
        notes.push("the board reveals one line each time the teacher presses Next".into());
    }
    if !notes.is_empty() {
        result["note"] = Value::String(notes.join("; "));
    }
    let mut out = Outcome::changed(result);
    if new_page {
        out.show_page = Some(page_no - 1);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{apply, apply_with};
    use rupok_core::Lesson;
    use rupok_format::Package;

    struct Board {
        pkg: Package,
        lesson: Lesson,
        page: String,
        selection: Vec<String>,
    }

    impl Board {
        fn new() -> Self {
            let lesson = Lesson::new("Live", 1_000.0);
            let pkg = Package::from_lesson(&lesson, "test", 1_000.0);
            let page = lesson.page_id_at(0).unwrap();
            Board {
                pkg,
                lesson,
                page,
                selection: Vec::new(),
            }
        }
        fn run_with(&mut self, op: &Value, glyphs: Option<&GlyphSet>) -> Result<Outcome> {
            let op: Op = serde_json::from_value(op.clone()).unwrap();
            apply_with(
                BoardCtx {
                    pkg: &mut self.pkg,
                    lesson: &mut self.lesson,
                    page_id: &self.page,
                    selection: &self.selection,
                    read_only: false,
                },
                &op,
                "Claude",
                10_000.0,
                glyphs,
            )
        }
        fn run(&mut self, op: Value) -> Result<Outcome> {
            self.run_with(&op, None)
        }
        /// Handwritten "2x+3=7"-sized strokes at (x, y): 4 strokes 50 px tall.
        fn problem(&mut self, x: f32, y: f32) -> Vec<String> {
            let mut ids = Vec::new();
            for k in 0..4 {
                let pts = (0..10)
                    .map(|i| StrokePoint {
                        x: x + k as f32 * 40.0 + i as f32 * 2.0,
                        y: y + i as f32 * 5.0,
                        p: 0.5,
                        dt: i * 8,
                    })
                    .collect();
                let d = ItemData::Stroke(StrokeData {
                    tool: "pen".into(),
                    color: 0x3e63ddff,
                    width: 5.0,
                    opacity: 1.0,
                    points: pts,
                    offset: [0.0, 0.0],
                    shape: None,
                    geom: None,
                    brush: None,
                });
                ids.push(self.lesson.add_item(&self.page, &d, 2_000.0).unwrap());
            }
            ids
        }
    }

    #[test]
    fn writes_below_the_problem_in_its_ink_as_one_undo_step() {
        let mut b = Board::new();
        let ids = b.problem(200.0, 150.0);
        let before = b.lesson.items(&b.page).unwrap().len();
        let out = b
            .run(json!({"op": "write_handwriting", "lines": ["$2x = 4$", "$x = 2$"], "anchor": {"item_ids": ids}}))
            .unwrap();
        assert!(out.changed);
        let r = out.result;
        let new_ids = r["item_ids"].as_array().unwrap();
        assert!(new_ids.len() > 6);
        assert_eq!(r["strokes_by_line"].as_array().unwrap().len(), 2);
        assert_eq!(r["style"], "neat");
        let items = b.lesson.items(&b.page).unwrap();
        assert_eq!(items.len(), before + new_ids.len());
        // Matches the problem: colour, width, letter height (~45 px).
        let written: Vec<&Item> = items.iter().filter(|i| i.created > 9_000.0).collect();
        for it in &written {
            let ItemData::Stroke(s) = &it.data else {
                panic!()
            };
            assert_eq!(s.color, 0x3e63ddff);
            assert_eq!(s.width, 5.0);
            assert_eq!(it.group.as_deref(), r["group"].as_str());
            // Below the problem (which ends at y ≈ 195), left-aligned with it.
            let a = it.data.aabb();
            assert!(a[1] > 195.0, "{a:?}");
            assert!(a[0] >= 195.0);
        }
        // created follows the pen: increasing, starting now.
        let mut c: Vec<f64> = written.iter().map(|i| i.created).collect();
        let sorted = {
            let mut s = c.clone();
            s.sort_by(f64::total_cmp);
            s
        };
        assert_eq!(c.first().copied(), Some(10_000.0));
        c.sort_by(f64::total_cmp);
        assert_eq!(c, sorted);
        let agents = crate::agent_items(&b.lesson, &b.page);
        assert_eq!(
            agents.get(new_ids[0].as_str().unwrap()).map(String::as_str),
            Some("Claude")
        );
        let h = r["bounds"]["h"].as_f64().unwrap();
        assert!(h > 60.0 && h < 300.0, "{h}");
        // One undo removes the whole solution.
        assert!(b.lesson.undo().unwrap());
        assert_eq!(b.lesson.items(&b.page).unwrap().len(), before);
    }

    #[test]
    fn avoids_overlaps_and_spills_to_a_new_page() {
        let mut b = Board::new();
        let ids = b.problem(200.0, 100.0);
        // Something right under the problem: the writing goes below it.
        let blocker = b.problem(200.0, 230.0);
        let out = b
            .run(json!({"op": "write_handwriting", "lines": ["$x = 2$"], "anchor": {"item_ids": ids}}))
            .unwrap();
        let y = out.result["bounds"]["y"].as_f64().unwrap();
        assert!(y > 275.0, "below the blocker: {y}");
        let _ = blocker;

        // A problem at the very bottom with a long solution: a new page.
        let mut b = Board::new();
        let ids = b.problem(1500.0, 950.0);
        let lines: Vec<String> = (0..12)
            .map(|i| format!("Step {i}: $x_{i} = {i}$"))
            .collect();
        let out = b
            .run(json!({"op": "write_handwriting", "lines": lines, "anchor": {"item_ids": ids}}))
            .unwrap();
        assert!(out.result["note"].as_str().unwrap().contains("new page"));
        assert_eq!(out.result["page"], 2);
        assert_eq!(out.show_page, Some(1));
    }

    #[test]
    fn explicit_point_selection_and_teacher_hand() {
        let mut b = Board::new();
        let out = b
            .run(json!({"op": "write_handwriting", "lines": ["Hi"], "anchor": {"x": 300, "y": 400}, "size": 60, "color": "#e03131"}))
            .unwrap();
        assert_eq!(out.result["bounds"]["x"], 300.0);
        assert_eq!(out.result["ink"]["color"], "#e03131");
        // Selection is the default anchor.
        let ids = b.problem(900.0, 100.0);
        b.selection = ids;
        let out = b
            .run(json!({"op": "write_handwriting", "lines": ["ok"]}))
            .unwrap();
        let x = out.result["bounds"]["x"].as_f64().unwrap();
        assert!((x - 900.0).abs() < 4.0, "{x}");

        let mut g = GlyphSet::new();
        g.add_user(
            'o',
            &[vec![
                rupok_ink::math::geom::Pt::new(0.0, -14.0),
                rupok_ink::math::geom::Pt::new(9.0, 0.0),
            ]],
        );
        let out = b
            .run_with(
                &json!({"op": "write_handwriting", "lines": ["o"], "anchor": {"x": 10, "y": 900}}),
                Some(&g),
            )
            .unwrap();
        assert_eq!(out.result["style"], "teacher");
        let out = b
            .run_with(
                &json!({"op": "write_handwriting", "lines": ["o"], "style": "neat", "anchor": {"x": 10, "y": 900}}),
                Some(&g),
            )
            .unwrap();
        assert_eq!(out.result["style"], "neat");
    }

    #[test]
    fn bad_input_is_an_error_and_changes_nothing() {
        let mut b = Board::new();
        let v0 = b.lesson.version();
        for bad in [
            json!({"op": "write_handwriting", "lines": []}),
            json!({"op": "write_handwriting", "lines": ["  "]}),
            json!({"op": "write_handwriting", "lines": ["x"], "style": "comic"}),
            json!({"op": "write_handwriting", "lines": ["x"], "step_mode": "random"}),
            json!({"op": "write_handwriting", "lines": ["x"], "anchor": {"item_ids": ["nope"]}}),
            json!({"op": "write_handwriting", "lines": ["x"], "size": -3}),
            json!({"op": "write_handwriting", "lines": ["x"], "color": "blue-ish"}),
            json!({"op": "write_handwriting", "lines": ["x".repeat(500)]}),
            json!({"op": "write_handwriting", "lines": vec!["x"; 41]}),
        ] {
            assert!(b.run(bad.clone()).is_err(), "{bad}");
        }
        assert_eq!(b.lesson.version(), v0);
        // Student view refuses it like every write.
        let op: Op =
            serde_json::from_value(json!({"op": "write_handwriting", "lines": ["x"]})).unwrap();
        let err = apply(
            BoardCtx {
                pkg: &mut b.pkg,
                lesson: &mut b.lesson,
                page_id: &b.page,
                selection: &[],
                read_only: true,
            },
            &op,
            "Claude",
            1.0,
        );
        assert!(matches!(err, Err(LiveError::ReadOnly)));
    }
}
