//! Inserting shapes and tables (the Shapes picker, `catalog.rs`): placing a
//! dragged or tapped shape, its wet preview, fill and dash on the
//! selection, table rows and columns, text in a table cell, and the AI's
//! `add_shape` / `add_table`. Every change is one undo step.

use rupok_core::Edits;

use super::*;
use crate::catalog::{self, Label, Placement, Preset};
use crate::shapes::{self, SHAPE_PRESSURE};

/// Picker state the engine needs while a shape tool is active.
#[derive(Clone, Debug, Default)]
pub struct ShapeOpts {
    /// Fill of new closed shapes (0xRRGGBBAA), or none.
    pub fill: Option<u32>,
    /// Shift or the lock: square boxes, 15° lines.
    pub lock: bool,
    /// Items the last placement added (taken by the app to select them or
    /// switch back to the pen).
    pub placed: Option<Vec<ItemId>>,
}

/// The picker entry behind a shape tool.
pub fn preset_of(kind: ShapeKind) -> Preset {
    match kind {
        ShapeKind::Rect => Preset::Rect,
        ShapeKind::Ellipse => Preset::Ellipse,
        ShapeKind::Line => Preset::Line,
        ShapeKind::Arrow => Preset::Arrow,
        ShapeKind::Preset(p) => p,
    }
}

/// How long a placed shape takes to draw in replay (ms).
const REPLAY_MS: u32 = 450;

impl<S: InkSink> Engine<S> {
    pub fn set_shape_fill(&mut self, fill: Option<u32>) {
        self.shape_opts.fill = fill;
    }

    pub fn set_shape_lock(&mut self, lock: bool) {
        self.shape_opts.lock = lock;
    }

    /// The items the last shape placement added, once.
    pub fn take_placed(&mut self) -> Option<Vec<ItemId>> {
        self.shape_opts.placed.take()
    }

    /// World size of a tapped shape: about 200 screen px.
    fn shape_unit(&self) -> f64 {
        f64::from(200.0 / self.camera.zoom.max(1e-3)).clamp(40.0, 4000.0)
    }

    /// The placement of a shape gesture: a drag, or a tap (default size).
    fn gesture_placement(&self, kind: ShapeKind, start: [f32; 2], end: [f32; 2]) -> Placement {
        let p = preset_of(kind);
        let f = |q: [f32; 2]| [f64::from(q[0]), f64::from(q[1])];
        let d = (end[0] - start[0]).hypot(end[1] - start[1]) * self.camera.zoom;
        let (a, b) = if d <= 3.0 {
            let (a, b) = catalog::tap_drag(p, f(start), self.shape_unit());
            self.onto_page(a, b)
        } else {
            (
                f(start),
                catalog::constrain(p, f(start), f(end), self.shape_opts.lock),
            )
        };
        catalog::place(p, a, b)
    }

    /// Shift a tapped box onto the slide when ink is kept on the page.
    fn onto_page(&self, a: [f64; 2], b: [f64; 2]) -> ([f64; 2], [f64; 2]) {
        let Some(r) = self.ink_bounds() else {
            return (a, b);
        };
        let r = r.map(f64::from);
        let shift = |lo: f64, hi: f64, min: f64, max: f64| {
            // Too big for the page, or off its low edge: align to the low edge.
            if hi - lo > max - min || lo < min {
                min - lo
            } else if hi > max {
                max - hi
            } else {
                0.0
            }
        };
        let dx = shift(a[0].min(b[0]), a[0].max(b[0]), r[0], r[2]);
        let dy = shift(a[1].min(b[1]), a[1].max(b[1]), r[1], r[3]);
        ([a[0] + dx, a[1] + dy], [b[0] + dx, b[1] + dy])
    }

    /// Wet preview of a shape being dragged (nothing for a tap yet).
    pub(super) fn shape_wet(
        &self,
        kind: ShapeKind,
        start: [f32; 2],
        end: [f32; 2],
    ) -> Vec<WetStroke> {
        let d = (end[0] - start[0]).hypot(end[1] - start[1]) * self.camera.zoom;
        if d <= 3.0 {
            return Vec::new();
        }
        let st = self.shape_style;
        let pl = self.gesture_placement(kind, start, end);
        let mut out = Vec::new();
        for part in &pl.parts {
            let Some(pts) = shapes::geom_points(&part.geom, st.width, SHAPE_PRESSURE) else {
                continue;
            };
            let runs = if part.geom.dash {
                catalog::dash_pieces(&pts, st.width)
            } else {
                vec![pts]
            };
            out.extend(runs.into_iter().map(|points| WetStroke {
                points,
                width: st.width,
                color: st.color,
                opacity: st.opacity,
            }));
        }
        out
    }

    /// Place the shape of a finished shape gesture (one undo step).
    pub(super) fn place_shape(
        &mut self,
        kind: ShapeKind,
        start: [f32; 2],
        end: [f32; 2],
        started: f64,
    ) {
        if self.read_only {
            return;
        }
        let mut pl = self.gesture_placement(kind, start, end);
        if preset_of(kind) == Preset::Connector {
            for part in &mut pl.parts {
                part.geom = self.connect_any(part.geom.clone(), "");
            }
        }
        let st = self.shape_style;
        let page = self.page.clone();
        let style = Style {
            color: st.color,
            width: st.width,
            opacity: st.opacity,
            fill: self.shape_opts.fill,
        };
        if let Some(ids) = self.insert_placement(&page, pl, &style, &[], started) {
            self.shape_opts.placed = Some(ids);
        }
    }

    /// Add a placement's strokes (and labels, and `texts` boxes) to `page`
    /// as one change; several items share a new group. Returns their ids.
    fn insert_placement(
        &mut self,
        page: &str,
        pl: Placement,
        style: &Style,
        texts: &[(BoxGeom, String, f64, u32)],
        created: f64,
    ) -> Option<Vec<ItemId>> {
        let mut add: Vec<ItemData> = Vec::new();
        for part in pl.parts {
            let mut g = part.geom;
            if catalog::fillable(&g.kind) && g.fill.is_none() {
                g.fill = style.fill;
            }
            let pts = shapes::geom_points(&g, style.width, SHAPE_PRESSURE)?;
            add.push(ItemData::Stroke(StrokeData {
                tool: "pen".into(),
                color: style.color,
                width: style.width,
                opacity: style.opacity,
                points: shapes::with_timing(pts, REPLAY_MS),
                offset: [0.0, 0.0],
                shape: Some(part.shape.into()),
                geom: Some(g),
            }));
        }
        for Label { text, at, size } in pl.labels {
            let chars = text.chars().count().max(1) as f64;
            let w = 0.62 * size * chars + 0.4 * size;
            add.push(ItemData::Text(TextData {
                latex: false,
                geom: BoxGeom {
                    x: at[0] - w / 2.0,
                    y: at[1] - 0.65 * size,
                    w,
                    h: 1.3 * size,
                    rot: 0.0,
                },
                text,
                size,
                color: style.color,
            }));
        }
        for (geom, text, size, color) in texts {
            add.push(ItemData::Text(TextData {
                latex: false,
                geom: *geom,
                text: text.clone(),
                size: *size,
                color: *color,
            }));
        }
        if add.is_empty() {
            return None;
        }
        let group = (add.len() > 1).then(|| self.lesson.new_group_id());
        let e = Edits {
            add: add
                .into_iter()
                .enumerate()
                .map(|(i, data)| NewItem {
                    data,
                    created: created + i as f64,
                    group: group.clone(),
                    z: None,
                })
                .collect(),
            ..Default::default()
        };
        let ids = self.lesson.apply_edits(page, &e).ok()?;
        if page == self.page {
            self.resync();
        }
        Some(ids)
    }

    // ------------------------------------------------------- selection

    /// Shape strokes in the selection (their ids and data).
    fn selected_shape_strokes(&self) -> Vec<(ItemId, StrokeData)> {
        self.selection
            .iter()
            .filter_map(|id| match self.scene.get(id).map(|it| &it.data) {
                Some(ItemData::Stroke(s)) if s.geom.as_ref().is_some_and(|g| g.is_valid()) => {
                    Some((id.clone(), s.clone()))
                }
                _ => None,
            })
            .collect()
    }

    /// Set (or clear, `None`) the fill of the selected closed shapes. One
    /// undo step; false when nothing could take a fill.
    pub fn set_selection_fill(&mut self, fill: Option<u32>) -> bool {
        self.restyle_shapes(|g| {
            if !catalog::fillable(&g.kind) || g.fill == fill {
                return false;
            }
            g.fill = fill;
            true
        })
    }

    /// Dash (or not) the selected shapes' outlines. One undo step.
    pub fn set_selection_dash(&mut self, dash: bool) -> bool {
        self.restyle_shapes(|g| {
            if g.dash == dash {
                return false;
            }
            g.dash = dash;
            true
        })
    }

    fn restyle_shapes(&mut self, mut f: impl FnMut(&mut ShapeGeom) -> bool) -> bool {
        if self.read_only {
            return false;
        }
        let mut strokes = Vec::new();
        for (id, mut s) in self.selected_shape_strokes() {
            if let Some(g) = s.geom.as_mut()
                && f(g)
            {
                strokes.push((id, s));
            }
        }
        if strokes.is_empty() {
            return false;
        }
        let page = self.page.clone();
        let e = Edits {
            strokes,
            ..Default::default()
        };
        if self.lesson.apply_edits(&page, &e).is_err() {
            return false;
        }
        self.resync();
        true
    }

    /// The selected table, if exactly one table stroke is selected (alone
    /// or with its group).
    fn selected_table(&self) -> Option<(ItemId, StrokeData)> {
        let mut tables = self
            .selected_shape_strokes()
            .into_iter()
            .filter(|(_, s)| s.geom.as_ref().is_some_and(|g| g.kind == "table"));
        let t = tables.next()?;
        tables.next().is_none().then_some(t)
    }

    /// Add or remove a row or column of the selected table: `op` is
    /// `row+`, `row-`, `col+` or `col-`. The table grows or shrinks by one
    /// cell (rows and columns stay even), keeping its top-left corner.
    pub fn table_edit(&mut self, op: &str) -> bool {
        if self.read_only {
            return false;
        }
        let Some((id, s)) = self.selected_table() else {
            return false;
        };
        let Some(g) = s.geom.clone() else {
            return false;
        };
        let (rows, cols) = catalog::table_size(&g);
        let (nr, nc) = match op {
            "row+" => (rows + 1, cols),
            "row-" => (rows.saturating_sub(1), cols),
            "col+" => (rows, cols + 1),
            "col-" => (rows, cols.saturating_sub(1)),
            _ => return false,
        };
        let max = usize::from(catalog::MAX_TABLE);
        if nr == 0 || nc == 0 || nr > max || nc > max || (nr, nc) == (rows, cols) {
            return false;
        }
        let ng = ShapeGeom {
            w: g.w * nc as f64 / cols as f64,
            h: g.h * nr as f64 / rows as f64,
            v: catalog::table_v(nr as u8, nc as u8),
            ..g
        };
        let p = s.points.first().map(|q| q.p).unwrap_or(SHAPE_PRESSURE);
        let duration = s.points.last().map(|q| q.dt).unwrap_or(0);
        let Some(pts) = shapes::geom_points(&ng, s.width, p) else {
            return false;
        };
        let e = Edits {
            strokes: vec![(
                id,
                StrokeData {
                    points: shapes::with_timing(pts, duration),
                    geom: Some(ng),
                    ..s
                },
            )],
            ..Default::default()
        };
        if self.lesson.apply_edits(&self.page.clone(), &e).is_err() {
            return false;
        }
        self.resync();
        true
    }

    /// Cell rects of the selected table, world units, row by row:
    /// `{"id", "rows", "cols", "cells": [[x, y, w, h], …]}`, or `null`.
    pub fn table_cells_json(&self) -> String {
        let Some((id, s)) = self.selected_table() else {
            return "null".into();
        };
        let Some(g) = s.geom.as_ref() else {
            return "null".into();
        };
        let (rows, cols) = catalog::table_size(g);
        serde_json::json!({
            "id": id,
            "rows": rows,
            "cols": cols,
            "rot": g.rot,
            "cells": cell_rects(g, s.offset),
        })
        .to_string()
    }

    /// A text item in cell (`row`, `col`) of the selected table, grouped
    /// with it, in one undo step. Returns its id (the app starts editing).
    pub fn table_cell_text(
        &mut self,
        row: usize,
        col: usize,
        text: &str,
        now_ms: f64,
    ) -> Option<ItemId> {
        if self.read_only {
            return None;
        }
        let (id, s) = self.selected_table()?;
        let g = s.geom.as_ref()?;
        let (rows, cols) = catalog::table_size(g);
        if row >= rows || col >= cols {
            return None;
        }
        let r = cell_rects(g, s.offset).get(row * cols + col).copied()?;
        let size = cell_text_size(r[3]);
        let pad = (size * 0.25).min(r[2] * 0.1);
        let geom = BoxGeom {
            x: r[0] + pad,
            y: r[1] + (r[3] - 1.3 * size) / 2.0,
            w: (r[2] - 2.0 * pad).max(size),
            h: 1.3 * size,
            rot: 0.0,
        };
        let page = self.page.clone();
        let mut e = Edits::default();
        let group = match self.scene.get(&id).and_then(|it| it.group.clone()) {
            Some(gid) => gid,
            None => {
                let gid = self.lesson.new_group_id();
                e.groups.push((id.clone(), Some(gid.clone())));
                gid
            }
        };
        e.add.push(NewItem {
            data: ItemData::Text(TextData {
                latex: false,
                geom,
                text: text.into(),
                size,
                color: s.color,
            }),
            created: now_ms,
            group: Some(group),
            z: None,
        });
        let added = self.lesson.apply_edits(&page, &e).ok()?;
        self.resync();
        added.into_iter().next()
    }

    /// Style of the selected shapes for the selection toolbar:
    /// `{"fill": "#rrggbbaa" | null, "fillable", "dash", "table": [rows, cols] | null}`,
    /// or `null` without shapes.
    pub fn shape_style_meta(&self) -> serde_json::Value {
        let shapes = self.selected_shape_strokes();
        if shapes.is_empty() {
            return serde_json::Value::Null;
        }
        let geoms: Vec<&ShapeGeom> = shapes.iter().filter_map(|(_, s)| s.geom.as_ref()).collect();
        let fillable: Vec<&&ShapeGeom> = geoms
            .iter()
            .filter(|g| catalog::fillable(&g.kind))
            .collect();
        let fill = fillable
            .first()
            .and_then(|g| g.fill)
            .filter(|f| fillable.iter().all(|g| g.fill == Some(*f)));
        serde_json::json!({
            "fill": fill.map(css_color),
            "fillable": !fillable.is_empty(),
            "dash": geoms.iter().all(|g| g.dash),
            "table": self.selected_table().and_then(|(_, s)| {
                s.geom.as_ref().map(|g| {
                    let (r, c) = catalog::table_size(g);
                    [r, c]
                })
            }),
        })
    }

    /// A filled shape whose inside holds world point `w` (topmost), so a
    /// tap inside a filled shape selects it.
    pub(super) fn filled_hit(&self, w: [f32; 2]) -> Option<ItemId> {
        let q = [f64::from(w[0]), f64::from(w[1])];
        let hits = self.scene.query([w[0], w[1], w[0], w[1]]);
        hits.into_iter()
            .filter_map(|it| match &it.data {
                ItemData::Stroke(s) => {
                    let poly = catalog::fill_outline(s.geom.as_ref()?, &s.points, s.offset)?;
                    point_in(&poly, q).then(|| (it.z, it.id.clone()))
                }
                _ => None,
            })
            .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
            .map(|(_, id)| id)
    }

    // -------------------------------------------------------------- AI

    /// Run the AI's `add_shape` / `add_table` (see [`rupok_live::Op`]) as
    /// one change tagged with the agent. Returns the result JSON, or the
    /// error JSON the agent should see.
    pub fn ai_insert(
        &mut self,
        op: &rupok_live::Op,
        agent: &str,
        now_ms: f64,
    ) -> Result<String, String> {
        use rupok_live::{LiveError, Op, apply};
        let err = |e: LiveError| e.to_json().to_string();
        let invalid = |m: &str| err(LiveError::Invalid(m.into()));
        if self.read_only {
            return Err(err(LiveError::ReadOnly));
        }
        let finite = |name: &str, v: f64| {
            if v.is_finite() && v.abs() < 1e6 {
                Ok(v)
            } else {
                Err(invalid(&format!(
                    "{name} must be a finite number of slide pixels"
                )))
            }
        };
        let color = |c: &Option<String>, def: u32| match c {
            Some(c) => apply::parse_color(c).map_err(err),
            None => Ok(def),
        };
        let page_arg = match op {
            Op::AddShape { page, .. } | Op::AddTable { page, .. } => *page,
            _ => return Err(invalid("not a shape operation")),
        };
        let current = self.page.clone();
        let (idx, info) = apply::page_ref(&self.lesson, &current, page_arg).map_err(err)?;
        let ink = apply::default_ink(info.bg_color);
        let msg = rupok_live::commit_message(agent, op.name());
        let (pl, style, texts, label) = match op {
            Op::AddShape {
                shape,
                x,
                y,
                w,
                h,
                color: c,
                fill,
                width,
                dashed,
                text,
                ..
            } => {
                let preset = Preset::parse(shape.trim()).ok_or_else(|| {
                    invalid(&format!(
                        "unknown shape `{shape}`; use one of: {}",
                        catalog::IDS.join(", ")
                    ))
                })?;
                let (x, y, w, h) = (
                    finite("x", *x)?,
                    finite("y", *y)?,
                    finite("w", *w)?,
                    finite("h", *h)?,
                );
                if !preset.is_segment() && (w <= 0.0 || h <= 0.0) {
                    return Err(invalid("w and h must be positive"));
                }
                let width = width.unwrap_or(4.0);
                if !(width.is_finite() && (0.5..=60.0).contains(&width)) {
                    return Err(invalid("width must be between 0.5 and 60"));
                }
                let mut pl = catalog::place(preset, [x, y], [x + w, y + h]);
                if dashed.unwrap_or(false) {
                    pl.parts.iter_mut().for_each(|p| p.geom.dash = true);
                }
                let color = color(c, ink)?;
                let mut texts = Vec::new();
                if let Some(t) = text.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
                    if t.chars().count() > 400 {
                        return Err(invalid("text is longer than 400 characters"));
                    }
                    let (bx, by, bw, bh) = (x.min(x + w), y.min(y + h), w.abs(), h.abs());
                    let lines = t.lines().count().max(1) as f64;
                    let size = (bh / (1.4 * lines + 0.6)).clamp(18.0, 48.0);
                    let tw = (bw * 0.86).max(size);
                    texts.push((
                        BoxGeom {
                            x: bx + (bw - tw) / 2.0,
                            y: by + (bh - 1.3 * size * lines) / 2.0,
                            w: tw,
                            h: 1.3 * size * lines,
                            rot: 0.0,
                        },
                        t.to_string(),
                        size,
                        color,
                    ));
                }
                let style = Style {
                    color,
                    width: width as f32,
                    opacity: 1.0,
                    fill: fill
                        .as_ref()
                        .map(|f| apply::parse_color(f).map_err(err))
                        .transpose()?,
                };
                (pl, style, texts, preset.shape_name())
            }
            Op::AddTable {
                x,
                y,
                rows,
                cols,
                w,
                h,
                cells,
                color: c,
                header_fill,
                text_size,
                ..
            } => {
                let max = u32::from(catalog::MAX_TABLE);
                if !(1..=max).contains(rows) || !(1..=max).contains(cols) {
                    return Err(invalid("rows and cols must be between 1 and 10"));
                }
                let (x, y) = (finite("x", *x)?, finite("y", *y)?);
                let w = finite("w", w.unwrap_or(f64::from(*cols) * 220.0))?;
                let h = finite("h", h.unwrap_or(f64::from(*rows) * 72.0))?;
                if w <= 0.0 || h <= 0.0 {
                    return Err(invalid("w and h must be positive"));
                }
                let preset = Preset::Table {
                    rows: *rows as u8,
                    cols: *cols as u8,
                };
                let mut pl = catalog::place(preset, [x, y], [x + w, y + h]);
                let color = color(c, ink)?;
                let g = pl.parts[0].geom.clone();
                let rects = cell_rects(&g, [0.0, 0.0]);
                let size = match text_size {
                    Some(s) if s.is_finite() && (8.0..=200.0).contains(s) => *s,
                    Some(_) => return Err(invalid("text_size must be between 8 and 200")),
                    None => cell_text_size(h / f64::from(*rows)),
                };
                let mut texts = Vec::new();
                for (r, row) in cells.iter().flatten().enumerate().take(*rows as usize) {
                    for (ci, t) in row.iter().enumerate().take(*cols as usize) {
                        let t = t.trim();
                        if t.is_empty() {
                            continue;
                        }
                        if t.chars().count() > 200 {
                            return Err(invalid("a cell's text is longer than 200 characters"));
                        }
                        let rc = rects[r * *cols as usize + ci];
                        let pad = (size * 0.25).min(rc[2] * 0.1);
                        texts.push((
                            BoxGeom {
                                x: rc[0] + pad,
                                y: rc[1] + (rc[3] - 1.3 * size) / 2.0,
                                w: (rc[2] - 2.0 * pad).max(size),
                                h: 1.3 * size,
                                rot: 0.0,
                            },
                            t.to_string(),
                            size,
                            color,
                        ));
                    }
                }
                // A header fill is a filled rectangle under the first row.
                if let Some(hf) = header_fill {
                    let f = apply::parse_color(hf).map_err(err)?;
                    let r0 = rects[0];
                    let mut head = catalog::place(Preset::Rect, [x, y], [x + w, r0[1] + r0[3]]);
                    head.parts[0].geom.fill = Some(f);
                    head.parts[0].shape = "table-header";
                    pl.parts.insert(0, head.parts.swap_remove(0));
                }
                let style = Style {
                    color,
                    width: 3.0,
                    opacity: 1.0,
                    fill: None,
                };
                (pl, style, texts, "table")
            }
            _ => return Err(invalid("not a shape operation")),
        };
        let page_id = info.id.clone();
        self.lesson.loro().set_next_commit_message(&msg);
        let ids = self.insert_placement(&page_id, pl, &style, &texts, now_ms);
        self.lesson.loro().clear_next_commit_options();
        let ids = ids.ok_or_else(|| invalid("the shape could not be drawn"))?;
        Ok(serde_json::json!({
            "page": idx + 1,
            "shape": label,
            "item_ids": ids,
            "group": ids.len() > 1,
        })
        .to_string())
    }
}

/// Stroke style of inserted shapes.
struct Style {
    color: u32,
    width: f32,
    opacity: f32,
    fill: Option<u32>,
}

/// Cell rects `[x, y, w, h]` (world, unrotated box, offset applied) of a
/// table, row by row.
fn cell_rects(g: &ShapeGeom, offset: [f32; 2]) -> Vec<[f64; 4]> {
    let (xs, ys) = catalog::table_edges(g);
    let (ox, oy) = (g.x + f64::from(offset[0]), g.y + f64::from(offset[1]));
    let mut out = Vec::new();
    for r in ys.windows(2) {
        for c in xs.windows(2) {
            out.push([
                ox + c[0] * g.w,
                oy + r[0] * g.h,
                (c[1] - c[0]) * g.w,
                (r[1] - r[0]) * g.h,
            ]);
        }
    }
    out
}

/// Text size that fits a cell `h` tall.
fn cell_text_size(h: f64) -> f64 {
    (h * 0.5).clamp(20.0, 40.0)
}

/// Even-odd point in polygon.
fn point_in(poly: &[[f64; 2]], q: [f64; 2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a[1] > q[1]) != (b[1] > q[1])
            && q[0] < (b[0] - a[0]) * (q[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::super::tests::{MockSink, draw, engine, select_rect};
    use super::*;

    fn items(e: &Engine<MockSink>) -> Vec<Item> {
        e.lesson.items(&e.page.clone()).unwrap()
    }

    fn tool(e: &mut Engine<MockSink>, id: &str) {
        e.set_tool(ToolKind::parse(&format!("shape:{id}")).unwrap());
    }

    fn tap(e: &mut Engine<MockSink>, at: [f32; 2]) {
        e.pointer_down(1, at[0], at[1], 0.5, 0.0, PointerKind::Mouse, 0, 0.0);
        e.pointer_up(1, 1.0);
    }

    #[test]
    fn every_picker_shape_places_by_tap_and_by_drag_and_undoes() {
        let mut e = engine();
        for (i, id) in catalog::IDS.iter().enumerate() {
            tool(&mut e, id);
            let n = items(&e).len();
            tap(&mut e, [300.0, 300.0]);
            let placed = e.take_placed().unwrap_or_else(|| panic!("{id} tap"));
            assert!(!placed.is_empty(), "{id}");
            assert_eq!(items(&e).len(), n + placed.len());
            draw(
                &mut e,
                2,
                PointerKind::Pen,
                [100.0, 100.0 + i as f32],
                [500.0, 380.0],
            );
            assert!(e.take_placed().is_some(), "{id} drag");
            // One undo removes the whole placement.
            assert!(e.lesson.undo().unwrap());
            e.resync();
            assert_eq!(items(&e).len(), n + placed.len(), "{id} undo");
        }
        assert!(e.take_placed().is_none());
    }

    #[test]
    fn a_drag_with_shift_draws_a_square_and_the_fill_lands_on_closed_shapes() {
        let mut e = engine();
        tool(&mut e, "rect");
        e.set_shape_lock(true);
        e.set_shape_fill(Some(0xa7f3d080));
        draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [400.0, 200.0]);
        let it = &items(&e)[0];
        let ItemData::Stroke(s) = &it.data else {
            panic!()
        };
        let g = s.geom.as_ref().unwrap();
        assert_eq!((g.w, g.h), (300.0, 300.0));
        assert_eq!(g.fill, Some(0xa7f3d080));
        // The fill is a DOM item under the ink, keyed apart from the stroke.
        let dom: serde_json::Value = serde_json::from_str(&e.dom_items_json()).unwrap();
        assert_eq!(dom[0]["kind"], "fill");
        assert_eq!(dom[0]["id"], format!("{}#fill", it.id));
        assert!(dom[0]["path"].as_str().unwrap().starts_with("M0 0"));
        // Lines take no fill.
        tool(&mut e, "line");
        draw(&mut e, 1, PointerKind::Pen, [100.0, 500.0], [400.0, 500.0]);
        let ItemData::Stroke(l) = &items(&e)[1].data else {
            panic!()
        };
        assert_eq!(l.geom.as_ref().unwrap().fill, None);
    }

    #[test]
    fn fill_and_dash_restyle_the_selection_in_one_undo_step_and_survive_resize() {
        let mut e = engine();
        tool(&mut e, "ellipse");
        draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [300.0, 200.0]);
        select_rect(&mut e, [50.0, 50.0], [350.0, 250.0]);
        assert!(e.set_selection_fill(Some(0xffd60a66)));
        assert!(e.set_selection_dash(true));
        let meta = e.shape_style_meta();
        assert_eq!(meta["fill"], "#ffd60a66");
        assert_eq!(meta["dash"], true);
        // The renderer gets the outline as dashes.
        let id = items(&e)[0].id.clone();
        assert!(
            !e.sink.strokes.contains_key(&id) || e.sink.strokes[&id].geom.as_ref().unwrap().dash
        );
        assert!(e.resize_shapes([100.0, 100.0, 500.0, 300.0], true));
        let ItemData::Stroke(s) = &items(&e)[0].data else {
            panic!()
        };
        let g = s.geom.as_ref().unwrap();
        assert_eq!((g.fill, g.dash), (Some(0xffd60a66), true));
        assert!((g.w - 400.0).abs() < 1e-6);
        e.lesson.undo().unwrap(); // resize
        e.lesson.undo().unwrap(); // dash
        e.resync();
        let ItemData::Stroke(s) = &items(&e)[0].data else {
            panic!()
        };
        assert!(!s.geom.as_ref().unwrap().dash);
        assert_eq!(s.geom.as_ref().unwrap().fill, Some(0xffd60a66));
        assert!(e.set_selection_fill(None));
    }

    #[test]
    fn a_tap_inside_a_filled_shape_selects_it() {
        let mut e = engine();
        tool(&mut e, "rect");
        e.set_shape_fill(Some(0xa7f3d0ff));
        draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [400.0, 300.0]);
        e.set_tool(ToolKind::Lasso);
        tap(&mut e, [250.0, 200.0]);
        assert_eq!(e.selection.len(), 1);
    }

    #[test]
    fn tables_grow_and_shrink_and_take_cell_text() {
        let mut e = engine();
        tool(&mut e, "table:3x4");
        draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [500.0, 250.0]);
        select_rect(&mut e, [50.0, 50.0], [600.0, 300.0]);
        assert_eq!(e.shape_style_meta()["table"], serde_json::json!([3, 4]));
        assert!(e.table_edit("row+"));
        let meta = e.shape_style_meta();
        assert_eq!(meta["table"], serde_json::json!([4, 4]));
        let ItemData::Stroke(s) = &items(&e)[0].data else {
            panic!()
        };
        assert!(
            (s.geom.as_ref().unwrap().h - 200.0).abs() < 1e-6,
            "one more row of the same height"
        );
        assert!(e.table_edit("col-"));
        assert_eq!(e.shape_style_meta()["table"], serde_json::json!([4, 3]));
        assert!(!e.table_edit("nonsense"));
        let cells: serde_json::Value = serde_json::from_str(&e.table_cells_json()).unwrap();
        assert_eq!(cells["cells"].as_array().unwrap().len(), 12);
        let t = e.table_cell_text(1, 2, "Na", 5.0).unwrap();
        let all = items(&e);
        let table = all
            .iter()
            .find(|i| matches!(i.data, ItemData::Stroke(_)))
            .unwrap();
        let text = all.iter().find(|i| i.id == t).unwrap();
        assert!(
            table.group.is_some() && table.group == text.group,
            "grouped with the table"
        );
        let ItemData::Text(td) = &text.data else {
            panic!()
        };
        let c = cells["cells"][3 + 2].as_array().unwrap();
        assert!(td.geom.x >= c[0].as_f64().unwrap() && td.geom.y >= c[1].as_f64().unwrap());
        assert!(e.table_cell_text(9, 0, "x", 6.0).is_none());
        // Undo takes the text away in one step.
        e.lesson.undo().unwrap();
        e.resync();
        assert!(items(&e).iter().all(|i| i.id != t));
    }

    #[test]
    fn the_ai_draws_shapes_and_tables_as_one_attributed_step() {
        let mut e = engine();
        let op: rupok_live::Op = serde_json::from_str(
            r##"{"op":"add_shape","shape":"decision","x":100,"y":100,"w":200,"h":120,"fill":"#a7f3d0","text":"x > 0?"}"##,
        )
        .unwrap();
        let out: serde_json::Value =
            serde_json::from_str(&e.ai_insert(&op, "Claude", 10.0).unwrap()).unwrap();
        assert_eq!(out["item_ids"].as_array().unwrap().len(), 2);
        assert_eq!(out["shape"], "decision");
        let op: rupok_live::Op = serde_json::from_str(
            r##"{"op":"add_table","x":100,"y":400,"rows":3,"cols":2,"cells":[["Metal","Symbol"],["Sodium","Na"]],"header_fill":"#d0ebff"}"##,
        )
        .unwrap();
        let out: serde_json::Value =
            serde_json::from_str(&e.ai_insert(&op, "Claude", 20.0).unwrap()).unwrap();
        // Header rect + table + 4 cell texts.
        assert_eq!(out["item_ids"].as_array().unwrap().len(), 6);
        let made = rupok_live::agent_items(&e.lesson, &e.page.clone());
        assert_eq!(made.len(), 8, "attributed to the agent: {made:?}");
        e.lesson.undo().unwrap();
        e.resync();
        assert_eq!(items(&e).len(), 2);
        // Bad input is an error, not a panic.
        for bad in [
            r#"{"op":"add_shape","shape":"blob","x":0,"y":0,"w":10,"h":10}"#,
            r#"{"op":"add_shape","shape":"rect","x":0,"y":0,"w":-10,"h":10}"#,
            r#"{"op":"add_table","x":0,"y":0,"rows":11,"cols":2}"#,
            r#"{"op":"add_shape","shape":"rect","x":0,"y":0,"w":10,"h":10,"color":"nope"}"#,
        ] {
            let op: rupok_live::Op = serde_json::from_str(bad).unwrap();
            assert!(e.ai_insert(&op, "Claude", 30.0).is_err(), "{bad}");
        }
        e.set_read_only(true);
        assert!(e.ai_insert(&op, "Claude", 40.0).is_err());
    }
}
