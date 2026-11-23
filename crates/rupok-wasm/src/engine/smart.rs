//! Smart Ink in the engine: shape snapping when the pen lifts (Auto) or after
//! a short hold while it is still down (Hold to snap), the two-stroke arrow,
//! scratch-out to erase, resizing shape strokes from their stored geometry,
//! and the ink → text replacement used by handwriting conversion.
//!
//! Undo model: the hand-drawn stroke is committed first and the snap is a
//! second change that rewrites it in place (same id, `z` and `created`), so
//! the first undo brings back the ink exactly as drawn. A scratch-out is one
//! change that removes what it covered (the scribble itself is never kept).

use std::collections::HashSet;

use rupok_core::{BoxGeom, Item, ItemData, ItemId, ShapeGeom, StrokeData, TextData};
use rupok_ink::StrokePoint;
use rupok_ink::recognize::{self, Context, Neighbour, Options, Proposal, Registry};

use super::{Engine, Gesture, InkSink, WetStroke};
use crate::shapes;

/// Smart shapes (Settings → Smart ink). Off by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SmartMode {
    #[default]
    Off,
    /// Snap when the pen lifts.
    Auto,
    /// Snap after holding the pen still; keep dragging to resize or rotate.
    Hold,
}

impl SmartMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "auto" => SmartMode::Auto,
            "hold" => SmartMode::Hold,
            _ => SmartMode::Off,
        }
    }
}

/// Smart Ink settings. Pen only: the highlighter never snaps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmartInk {
    pub mode: SmartMode,
    /// Scratch-out to erase (only while smart shapes are on).
    pub scratch: bool,
}

/// Hold the pen this still (ms) to snap while it is down.
pub const HOLD_MS: f64 = 500.0;
/// Movement (screen px) that still counts as holding still.
const HOLD_SLOP_PX: f32 = 6.0;
/// A stroke started this soon (ms) after a nearby writing-size stroke is
/// handwriting in progress, and never snaps on pen-up.
const WRITING_GAP_MS: f64 = 450.0;
/// "Writing size" for that guard: the previous stroke's larger side, px.
const WRITING_PX: f32 = 110.0;
/// Scratch-out flash duration, ms.
const FLASH_MS: f64 = 420.0;
/// Flash colour (Radix red 9).
const FLASH_COLOR: u32 = 0xe5484dff;
/// Share of a stroke's bounds a scratch-out must cover to erase it.
const SCRATCH_COVER: f32 = 0.6;

/// Hold-to-snap state of an ink gesture.
pub(super) struct Hold {
    /// Where (world) and when (wall clock) the pen last moved noticeably.
    pub still: ([f32; 2], f64),
    pub snapped: Option<Held>,
}

impl Hold {
    pub fn new(w: [f32; 2], now_ms: f64) -> Self {
        Self {
            still: (w, now_ms),
            snapped: None,
        }
    }
}

/// A shape snapped while the pen is still down.
pub(super) struct Held {
    pub shape: &'static str,
    base: ShapeGeom,
    pub current: ShapeGeom,
    pivot: [f64; 2],
    anchor: [f32; 2],
}

/// The last committed pen stroke (for the handwriting guard).
pub(super) struct LastInk {
    ended: f64,
    bounds: [f32; 4],
    size_px: f32,
    snapped: bool,
}

/// A fading flash in the wet layer.
pub(super) struct Flash {
    strokes: Vec<WetStroke>,
    start: f64,
}

fn median_pressure(pts: &[StrokePoint]) -> f32 {
    if pts.is_empty() {
        return shapes::SHAPE_PRESSURE;
    }
    let mut p: Vec<f32> = pts.iter().map(|q| q.p).collect();
    p.sort_by(f32::total_cmp);
    p[p.len() / 2]
}

/// Line or arrow from `a` to `b`, snapped to 0°/45°/90°… within 7°.
fn segment_geom(kind: &str, a: [f64; 2], b: [f64; 2]) -> ShapeGeom {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let th = dy.atan2(dx);
    let q = std::f64::consts::FRAC_PI_4;
    let target = (th / q).round() * q;
    let (w, h) = if (th - target).abs() <= f64::from(recognize::LINE_SNAP_DEG).to_radians() {
        let l = dx.hypot(dy);
        (l * target.cos(), l * target.sin())
    } else {
        (dx, dy)
    };
    ShapeGeom {
        kind: kind.into(),
        x: a[0],
        y: a[1],
        w,
        h,
        rot: 0.0,
        v: Vec::new(),
    }
}

impl Held {
    /// The pen dragged to `pen` after the snap: lines follow it with their
    /// end; closed shapes scale and rotate about their centre.
    fn drag(&mut self, pen: [f32; 2]) {
        let p = [f64::from(pen[0]), f64::from(pen[1])];
        if matches!(self.base.kind.as_str(), "line" | "arrow") {
            self.current = segment_geom(&self.base.kind, self.pivot, p);
            return;
        }
        let a = [
            f64::from(self.anchor[0]) - self.pivot[0],
            f64::from(self.anchor[1]) - self.pivot[1],
        ];
        let v = [p[0] - self.pivot[0], p[1] - self.pivot[1]];
        let (la, lv) = (a[0].hypot(a[1]), v[0].hypot(v[1]));
        if la < 1e-3 {
            return;
        }
        let s = (lv / la).clamp(0.05, 50.0);
        let mut rot = v[1].atan2(v[0]) - a[1].atan2(a[0]);
        rot = (rot + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
        // A dead zone keeps small wobbles from tilting the shape.
        if rot.abs() < 5f64.to_radians() || self.shape == "circle" {
            rot = 0.0;
        }
        self.current = shapes::scale_rotate(&self.base, self.pivot, s, rot);
    }
}

impl<S: InkSink> Engine<S> {
    pub fn set_smart_ink(&mut self, s: SmartInk) {
        self.smart = s;
        if s.mode == SmartMode::Off {
            self.last_ink = None;
        }
    }

    pub fn smart_ink(&self) -> SmartInk {
        self.smart
    }

    /// `pointer_move` for ink in Hold mode: track stillness, or drag the
    /// shape already snapped.
    pub(super) fn hold_move(hold: &mut Hold, w: [f32; 2], now_ms: f64, zoom: f32) {
        match &mut hold.snapped {
            Some(h) => h.drag(w),
            None => {
                let d = (w[0] - hold.still.0[0]).hypot(w[1] - hold.still.0[1]);
                if d * zoom > HOLD_SLOP_PX {
                    hold.still = (w, now_ms);
                }
            }
        }
    }

    /// Per frame: snap a pen that has been held still long enough. Returns
    /// true when a shape appeared.
    pub(super) fn hold_tick(&mut self, now_ms: f64) -> bool {
        if self.smart.mode != SmartMode::Hold || self.read_only {
            return false;
        }
        let zoom = self.camera.zoom;
        let mut changed = false;
        for g in self.gestures.values_mut() {
            let Gesture::Ink { builder, hold, .. } = g else {
                continue;
            };
            if hold.snapped.is_some()
                || builder.style.tool != rupok_ink::Tool::Pen
                || now_ms - hold.still.1 < HOLD_MS
            {
                continue;
            }
            let opts = Options {
                px_per_unit: zoom,
                deliberate: true,
            };
            match recognize::recognize(builder.points(), opts) {
                Some(r) => {
                    let (shape, geom) = shapes::from_recognized(&r);
                    let pivot = if matches!(geom.kind.as_str(), "line" | "arrow") {
                        [geom.x, geom.y]
                    } else {
                        shapes::geom_aabb(&geom)
                            .map(|b| [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0])
                            .unwrap_or([geom.x, geom.y])
                    };
                    hold.snapped = Some(Held {
                        shape,
                        base: geom.clone(),
                        current: geom,
                        pivot,
                        anchor: hold.still.0,
                    });
                    changed = true;
                }
                // Nothing to snap to: try again after another full hold.
                None => hold.still.1 = now_ms,
            }
        }
        changed
    }

    /// Wet preview of a held shape.
    pub(super) fn held_wet(builder: &rupok_ink::StrokeBuilder, h: &Held) -> Option<WetStroke> {
        let st = builder.style;
        Some(WetStroke {
            points: shapes::geom_points(&h.current, st.width, median_pressure(builder.points()))?,
            width: st.width,
            color: st.color,
            opacity: st.opacity,
        })
    }

    /// Commit a finished pen or highlighter stroke, applying Smart Ink.
    pub(super) fn finish_ink(
        &mut self,
        stroke: StrokeData,
        started: f64,
        held: Option<Held>,
        now_ms: f64,
    ) {
        let smart = self.smart.mode != SmartMode::Off && stroke.tool == "pen" && !self.read_only;
        if !smart {
            self.commit_stroke(stroke, started);
            return;
        }
        let opts = Options {
            px_per_unit: self.camera.zoom,
            deliberate: false,
        };
        if held.is_none() && self.smart.scratch && recognize::is_scratch_out(&stroke.points, opts) {
            let covered = self.scratch_covered(&stroke);
            if !covered.is_empty() {
                self.scratch_erase(&stroke, covered, now_ms);
                return;
            }
        }
        let snap = match held {
            Some(h) => Some((h.shape, h.current)),
            None => {
                // Standalone shapes snap on pen-up only in Auto, and never in
                // the middle of handwriting; composites may always fire.
                let standalone =
                    self.smart.mode == SmartMode::Auto && !self.writing_context(&stroke, started);
                let ctx = Context {
                    opts,
                    neighbours: self.neighbours(&stroke, started),
                };
                match Registry::standard().recognize(&stroke.points, &ctx, standalone) {
                    Some(p) if !p.absorbs.is_empty() => {
                        self.absorb(&ctx, &p, stroke, started);
                        return;
                    }
                    Some(p) => Some(shapes::from_recognized(&p.recognized)),
                    None => None,
                }
            }
        };
        let bounds = stroke.aabb();
        let Some(id) = self.commit_stroke(stroke.clone(), started) else {
            return;
        };
        let snapped = match snap {
            Some((name, geom)) => self.snap_stroke(&id, &stroke, name, geom),
            None => false,
        };
        self.last_ink = Some(LastInk {
            ended: now_ms,
            bounds,
            size_px: (bounds[2] - bounds[0]).max(bounds[3] - bounds[1]) * self.camera.zoom,
            snapped,
        });
    }

    /// Strokes near `s` for contextual recognisers, most recent first, with
    /// offsets applied and clean geometry for shapes.
    fn neighbours(&self, s: &StrokeData, started: f64) -> Vec<Neighbour> {
        const MAX: usize = 24;
        let b = s.aabb();
        let pad = (0.5 * (b[2] - b[0]).max(b[3] - b[1])).max(80.0 / self.camera.zoom);
        let mut near: Vec<&Item> = self
            .scene
            .query([b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad])
            .into_iter()
            .filter(|it| matches!(it.data, ItemData::Stroke(_)))
            .collect();
        near.sort_by(|a, b| b.created.total_cmp(&a.created).then(b.id.cmp(&a.id)));
        near.truncate(MAX);
        near.into_iter()
            .filter_map(|it| {
                let ItemData::Stroke(st) = &it.data else {
                    return None;
                };
                let (ox, oy) = (st.offset[0], st.offset[1]);
                let ended = it.created + f64::from(st.points.last().map(|q| q.dt).unwrap_or(0));
                Some(Neighbour {
                    id: it.id.clone(),
                    points: st.points.iter().map(|q| [q.x + ox, q.y + oy]).collect(),
                    shape: st.shape.clone(),
                    geometry: st.geom.as_ref().and_then(|g| {
                        shapes::to_geometry(&shapes::translate(g, f64::from(ox), f64::from(oy)))
                    }),
                    bounds: it.data.aabb(),
                    gap_ms: started - ended,
                })
            })
            .collect()
    }

    /// Apply a composite: the result replaces the first absorbed neighbour
    /// (keeping its style and replay time); the new stroke and the other
    /// absorbed neighbours go. The new stroke is committed first, so the
    /// first undo brings it back as drawn.
    fn absorb(&mut self, ctx: &Context, p: &Proposal, stroke: StrokeData, started: f64) {
        let target_id = ctx.neighbours[p.absorbs[0]].id.clone();
        let Some(Item {
            data: ItemData::Stroke(target),
            ..
        }) = self.scene.get(&target_id).cloned()
        else {
            self.commit_stroke(stroke, started);
            return;
        };
        let (name, geom) = shapes::from_recognized(&p.recognized);
        let pressure = target
            .points
            .first()
            .map(|q| q.p)
            .unwrap_or(shapes::SHAPE_PRESSURE);
        let duration = target.points.last().map(|q| q.dt).unwrap_or(0);
        let Some(pts) = shapes::geom_points(&geom, target.width, pressure) else {
            self.commit_stroke(stroke, started);
            return;
        };
        let data = StrokeData {
            points: shapes::with_timing(pts, duration),
            offset: [0.0, 0.0],
            shape: Some(name.into()),
            geom: Some(geom),
            ..target
        };
        let Some(raw_id) = self.commit_stroke(stroke, started) else {
            return;
        };
        let mut remove = vec![raw_id];
        remove.extend(p.absorbs[1..].iter().map(|&i| ctx.neighbours[i].id.clone()));
        self.replace_stroke(&target_id, data, &remove);
        self.last_ink = None;
    }

    /// Handwriting in progress: the previous stroke was writing-size, ended
    /// moments ago and sits right next to this one.
    fn writing_context(&self, s: &StrokeData, started: f64) -> bool {
        let Some(li) = &self.last_ink else {
            return false;
        };
        if li.snapped || started - li.ended > WRITING_GAP_MS || li.size_px > WRITING_PX {
            return false;
        }
        let (b, l) = (s.aabb(), li.bounds);
        let gap = (l[0] - b[2])
            .max(b[0] - l[2])
            .max(l[1] - b[3])
            .max(b[1] - l[3]);
        let side = (b[2] - b[0]).max(b[3] - b[1]);
        gap <= (0.5 * side).max(60.0 / self.camera.zoom)
    }

    /// Rewrite stroke `id` (just committed from `raw`) as the clean shape.
    fn snap_stroke(&mut self, id: &str, raw: &StrokeData, name: &str, geom: ShapeGeom) -> bool {
        let duration = raw.points.last().map(|q| q.dt).unwrap_or(0);
        let Some(pts) = shapes::geom_points(&geom, raw.width, median_pressure(&raw.points)) else {
            return false;
        };
        let data = StrokeData {
            points: shapes::with_timing(pts, duration),
            offset: [0.0, 0.0],
            shape: Some(name.into()),
            geom: Some(geom),
            ..raw.clone()
        };
        self.replace_stroke(id, data, &[])
    }

    /// One change: rewrite a stroke in place and remove other strokes; the
    /// scene and renderer follow incrementally (no page re-read on the pen
    /// path).
    fn replace_stroke(&mut self, id: &str, data: StrokeData, remove: &[ItemId]) -> bool {
        let page = self.page.clone();
        let Some(old) = self.scene.get(id).cloned() else {
            return false;
        };
        if self
            .lesson
            .edit_strokes(&page, &[(id.to_string(), data.clone())], remove)
            .is_err()
        {
            return false;
        }
        for r in remove {
            self.scene.remove(r);
            self.sink.remove_stroke(r);
        }
        self.selection.retain(|s| !remove.contains(s));
        self.sink.add_stroke(id, &data);
        self.scene.insert(Item {
            data: ItemData::Stroke(data),
            ..old
        });
        self.bump();
        true
    }

    /// Ink a scratch-out covers: strokes its path touches whose bounds lie
    /// mostly (≥ 60 %) inside its own (slightly padded) bounds.
    fn scratch_covered(&self, s: &StrokeData) -> Vec<ItemId> {
        let b = s.aabb();
        let pad = 0.12 * (b[2] - b[0]).max(b[3] - b[1]);
        let zone = [b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad];
        let r = (s.width * 0.5).max(2.0 / self.camera.zoom);
        let mut hits: HashSet<ItemId> = HashSet::new();
        for w in s.points.windows(2) {
            hits.extend(self.scene.erase_hits([w[0].x, w[0].y], [w[1].x, w[1].y], r));
        }
        let mut covered: Vec<ItemId> = hits
            .into_iter()
            .filter(|id| {
                let Some(it) = self.scene.get(id) else {
                    return false;
                };
                let ib = it.data.aabb();
                let (w, h) = ((ib[2] - ib[0]).max(1e-3), (ib[3] - ib[1]).max(1e-3));
                let ix = (ib[2].min(zone[2]) - ib[0].max(zone[0])).max(0.0);
                let iy = (ib[3].min(zone[3]) - ib[1].max(zone[1])).max(0.0);
                ix * iy >= SCRATCH_COVER * w * h
            })
            .collect();
        // Merged ink erases as one unit, as with the eraser.
        if covered
            .iter()
            .any(|h| self.scene.get(h).is_some_and(|i| i.group.is_some()))
        {
            covered = self
                .expand_groups(&covered)
                .into_iter()
                .filter(|id| {
                    self.scene
                        .get(id)
                        .is_some_and(|i| matches!(i.data, ItemData::Stroke(_)))
                })
                .collect();
        }
        covered.sort();
        covered
    }

    fn scratch_erase(&mut self, scratch: &StrokeData, covered: Vec<ItemId>, now_ms: f64) {
        let wet = |s: &StrokeData, opacity: f32| WetStroke {
            points: s
                .points
                .iter()
                .map(|q| StrokePoint {
                    x: q.x + s.offset[0],
                    y: q.y + s.offset[1],
                    ..*q
                })
                .collect(),
            width: s.width,
            color: FLASH_COLOR,
            opacity,
        };
        let mut strokes = vec![wet(scratch, 0.55)];
        for id in &covered {
            if let Some(Item {
                data: ItemData::Stroke(s),
                ..
            }) = self.scene.get(id)
            {
                strokes.push(wet(s, 0.7));
            }
        }
        self.flashes.push(Flash {
            strokes,
            start: now_ms,
        });
        for id in &covered {
            self.scene.remove(id);
            self.sink.remove_stroke(id);
        }
        let _ = self.lesson.remove_items(&self.page.clone(), &covered);
        self.selection.retain(|s| !covered.contains(s));
        self.last_ink = None;
        self.bump();
    }

    /// Push the live flashes (fading out) and drop finished ones.
    pub(super) fn flash_wet(&mut self, now_ms: f64, wet: &mut Vec<WetStroke>) {
        self.flashes.retain(|f| now_ms - f.start < FLASH_MS);
        for f in &self.flashes {
            let k = (1.0 - (now_ms - f.start) / FLASH_MS).clamp(0.0, 1.0) as f32;
            for s in &f.strokes {
                wet.push(WetStroke {
                    opacity: s.opacity * k,
                    ..s.clone()
                });
            }
        }
    }

    // ------------------------------------------------------------- resize

    /// The single selected shape stroke and its geometric bounds in world
    /// units (`[x0, y0, x1, y1]`, without the stroke's thickness).
    pub fn selected_shape(&self) -> Option<(ItemId, [f64; 4])> {
        let [id] = self.selection.as_slice() else {
            return None;
        };
        let ItemData::Stroke(s) = &self.scene.get(id)?.data else {
            return None;
        };
        let b = shapes::geom_aabb(s.geom.as_ref()?)?;
        let (ox, oy) = (f64::from(s.offset[0]), f64::from(s.offset[1]));
        Some((id.clone(), [b[0] + ox, b[1] + oy, b[2] + ox, b[3] + oy]))
    }

    /// Resize a shape stroke so its geometric bounds become `to`: the points
    /// are regenerated from `geom`, so the line width stays the same.
    /// `commit: false` only previews (renderer), `true` is one undo step.
    pub fn resize_shape(&mut self, id: &str, to: [f64; 4], commit: bool) -> bool {
        if self.read_only || !to.iter().all(|v| v.is_finite()) || to[2] < to[0] || to[3] < to[1] {
            return false;
        }
        let Some(Item {
            data: ItemData::Stroke(s),
            ..
        }) = self.scene.get(id)
        else {
            return false;
        };
        let Some(g) = s.geom.as_ref() else {
            return false;
        };
        let world = shapes::translate(g, f64::from(s.offset[0]), f64::from(s.offset[1]));
        let Some(from) = shapes::geom_aabb(&world) else {
            return false;
        };
        let Some(ng) = shapes::map_box(&world, from, to) else {
            return false;
        };
        let p = s
            .points
            .first()
            .map(|q| q.p)
            .unwrap_or(shapes::SHAPE_PRESSURE);
        let duration = s.points.last().map(|q| q.dt).unwrap_or(0);
        let Some(pts) = shapes::geom_points(&ng, s.width, p) else {
            return false;
        };
        let data = StrokeData {
            points: shapes::with_timing(pts, duration),
            offset: [0.0, 0.0],
            geom: Some(ng),
            ..s.clone()
        };
        if commit {
            self.replace_stroke(id, data, &[])
        } else {
            self.sink.add_stroke(id, &data);
            self.dirty = true;
            true
        }
    }

    /// Drop a resize preview: draw the stroke as stored again.
    pub fn resize_shape_cancel(&mut self, id: &str) {
        if let Some(Item {
            data: ItemData::Stroke(s),
            ..
        }) = self.scene.get(id)
        {
            let s = s.clone();
            self.sink.add_stroke(id, &s);
            self.dirty = true;
        }
    }

    // --------------------------------------------------- ink → text

    /// The selected ink for handwriting recognition, in writing order:
    /// `{"bounds": [x0, y0, x1, y1], "strokes": [{"color": "#rrggbbaa",
    /// "width", "pts": [x, y, t, …]}]}` with offsets applied and `t` in ms
    /// since the first stroke began; `null` when no ink is selected.
    pub fn selection_ink_json(&self) -> String {
        let mut strokes: Vec<(&Item, &StrokeData)> = self
            .selection
            .iter()
            .filter_map(|id| self.scene.get(id))
            .filter_map(|it| match &it.data {
                ItemData::Stroke(s) => Some((it, s)),
                _ => None,
            })
            .collect();
        if strokes.is_empty() {
            return "null".into();
        }
        strokes.sort_by(|a, b| {
            a.0.created
                .total_cmp(&b.0.created)
                .then(a.0.id.cmp(&b.0.id))
        });
        let t0 = strokes[0].0.created;
        let ids: Vec<ItemId> = strokes.iter().map(|(it, _)| it.id.clone()).collect();
        let bounds = self.scene.bounds_of(&ids);
        let list: Vec<serde_json::Value> = strokes
            .iter()
            .map(|(it, s)| {
                let pts: Vec<f64> = s
                    .points
                    .iter()
                    .flat_map(|q| {
                        [
                            f64::from(q.x + s.offset[0]),
                            f64::from(q.y + s.offset[1]),
                            (it.created - t0).max(0.0) + f64::from(q.dt),
                        ]
                    })
                    .collect();
                serde_json::json!({
                    "color": super::css_color(s.color),
                    "width": s.width,
                    "pts": pts,
                })
            })
            .collect();
        serde_json::json!({ "bounds": bounds, "strokes": list }).to_string()
    }

    /// Replace the selected ink with a text item at `(x, y)`, `w` wide, font
    /// `size`, in the ink's colour: one undo step brings the ink back. The
    /// text becomes the selection. Returns its id.
    pub fn replace_selection_with_text(
        &mut self,
        text: &str,
        x: f64,
        y: f64,
        w: f64,
        size: f64,
        now_ms: f64,
    ) -> Option<ItemId> {
        if self.read_only
            || text.trim().is_empty()
            || ![x, y, w, size].iter().all(|v| v.is_finite())
            || size <= 0.0
        {
            return None;
        }
        let ink: Vec<(&ItemId, &StrokeData)> = self
            .selection
            .iter()
            .filter_map(|id| match &self.scene.get(id)?.data {
                ItemData::Stroke(s) => Some((id, s)),
                _ => None,
            })
            .collect();
        if ink.is_empty() {
            return None;
        }
        // The colour most of the ink is written in.
        let mut weight: Vec<(u32, usize)> = Vec::new();
        for (_, s) in &ink {
            match weight.iter_mut().find(|(c, _)| *c == s.color) {
                Some(e) => e.1 += s.points.len(),
                None => weight.push((s.color, s.points.len())),
            }
        }
        let color = weight
            .iter()
            .max_by_key(|e| e.1)
            .map(|e| e.0)
            .unwrap_or(0x1c2024ff);
        let remove: Vec<ItemId> = ink.iter().map(|(id, _)| (*id).clone()).collect();
        let size = size.clamp(4.0, 2000.0);
        let lines = text.lines().count().max(1) as f64;
        let data = ItemData::Text(TextData {
            geom: BoxGeom {
                x,
                y,
                w: w.max(size),
                h: size * 1.3 * lines,
                rot: 0.0,
            },
            text: text.to_string(),
            size,
            color,
        });
        let id = self
            .lesson
            .replace_items(&self.page.clone(), &remove, &data, now_ms)
            .ok()?;
        self.resync();
        self.selection = vec![id.clone()];
        self.bump();
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::MockSink;
    use super::super::{PointerKind, ToolKind};
    use super::*;

    fn engine(mode: SmartMode) -> Engine<MockSink> {
        let mut e = Engine::new(MockSink::default(), "Smart", 0.0);
        e.set_view(1600.0, 900.0);
        e.set_smart_ink(SmartInk {
            mode,
            scratch: true,
        });
        e
    }

    /// Draw a pen stroke through `pts` (screen = world at 100 %), starting
    /// at wall-clock `t0`; 8 ms per sample.
    fn draw(e: &mut Engine<MockSink>, pts: &[[f32; 2]], t0: f64) -> f64 {
        e.pointer_down(1, pts[0][0], pts[0][1], 0.5, t0, PointerKind::Pen, 0, t0);
        let mut t = t0;
        for p in &pts[1..] {
            t += 8.0;
            e.pointer_move(1, &[f64::from(p[0]), f64::from(p[1]), 0.5, t], &[], t);
        }
        e.pointer_up(1, t);
        t
    }

    fn circle(c: [f32; 2], r: f32) -> Vec<[f32; 2]> {
        (0..=90)
            .map(|i| {
                let a = i as f32 / 90.0 * std::f32::consts::TAU * 1.03;
                [c[0] + r * a.cos(), c[1] + r * 0.97 * a.sin()]
            })
            .collect()
    }

    fn densify(p: &[[f32; 2]], step: f32) -> Vec<[f32; 2]> {
        let mut out = vec![p[0]];
        for w in p.windows(2) {
            let n = ((w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]) / step)
                .ceil()
                .max(1.0) as usize;
            for k in 1..=n {
                let t = k as f32 / n as f32;
                out.push([
                    w[0][0] + t * (w[1][0] - w[0][0]),
                    w[0][1] + t * (w[1][1] - w[0][1]),
                ]);
            }
        }
        out
    }

    fn strokes(e: &Engine<MockSink>) -> Vec<StrokeData> {
        e.scene
            .ordered()
            .iter()
            .filter_map(|i| match &i.data {
                ItemData::Stroke(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn auto_snaps_on_pen_up_and_first_undo_restores_the_ink() {
        let mut e = engine(SmartMode::Auto);
        draw(&mut e, &circle([500.0, 400.0], 120.0), 1000.0);
        let s = strokes(&e);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].shape.as_deref(), Some("circle"));
        let g = s[0].geom.clone().expect("geom stored");
        assert_eq!(g.kind, "ellipse");
        assert!((g.w - 240.0).abs() < 20.0, "{g:?}");
        // Pen colour and width are kept; the renderer shows the shape.
        assert_eq!((s[0].color, s[0].width), (e.pen.color, e.pen.width));
        let id = e.scene.ordered()[0].id.clone();
        assert_eq!(e.sink.strokes[&id].shape.as_deref(), Some("circle"));
        // Replay draws the shape over the time the circle took to draw.
        assert!(s[0].points.last().unwrap().dt >= 700);
        assert!(e.undo());
        let s = strokes(&e);
        assert_eq!(s.len(), 1, "the hand-drawn circle is back");
        assert_eq!((s[0].shape.clone(), s[0].geom.clone()), (None, None));
        assert!(e.undo());
        assert!(strokes(&e).is_empty());
    }

    #[test]
    fn off_and_highlighter_never_snap() {
        let mut e = engine(SmartMode::Off);
        draw(&mut e, &circle([500.0, 400.0], 120.0), 1000.0);
        assert_eq!(strokes(&e)[0].shape, None);
        let mut e = engine(SmartMode::Auto);
        e.set_tool(ToolKind::Highlighter);
        draw(&mut e, &circle([500.0, 400.0], 120.0), 1000.0);
        assert_eq!(strokes(&e)[0].shape, None);
    }

    #[test]
    fn handwriting_in_progress_never_snaps() {
        let mut e = engine(SmartMode::Auto);
        // A writing-size stroke, then a big loop right after and next to it.
        let end = draw(
            &mut e,
            &densify(&[[300.0, 300.0], [330.0, 360.0], [350.0, 300.0]], 4.0),
            1000.0,
        );
        draw(&mut e, &circle([480.0, 340.0], 110.0), end + 200.0);
        assert!(strokes(&e).iter().all(|s| s.shape.is_none()));
        // After a pause it is a shape again.
        draw(&mut e, &circle([900.0, 340.0], 110.0), end + 5000.0);
        assert_eq!(strokes(&e).last().unwrap().shape.as_deref(), Some("circle"));
    }

    #[test]
    fn a_v_right_after_a_line_makes_an_arrow() {
        let mut e = engine(SmartMode::Auto);
        let end = draw(
            &mut e,
            &densify(&[[200.0, 300.0], [600.0, 304.0]], 5.0),
            1000.0,
        );
        assert_eq!(strokes(&e)[0].shape.as_deref(), Some("line"));
        draw(
            &mut e,
            &densify(&[[560.0, 265.0], [602.0, 301.0], [558.0, 338.0]], 3.0),
            end + 400.0,
        );
        let s = strokes(&e);
        assert_eq!(s.len(), 1, "the V merged into the line");
        assert_eq!(s[0].shape.as_deref(), Some("arrow"));
        let g = s[0].geom.clone().unwrap();
        assert!(
            g.w > 380.0 && g.h.abs() < 1.0,
            "tip at the right end: {g:?}"
        );
        // First undo brings the hand-drawn V back next to the line.
        assert!(e.undo());
        let s = strokes(&e);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].shape.as_deref(), Some("line"));
        assert_eq!(s[1].shape, None);
        // Too late: a V after 1.2 s stays ink.
        let mut e = engine(SmartMode::Auto);
        let end = draw(
            &mut e,
            &densify(&[[200.0, 300.0], [600.0, 304.0]], 5.0),
            1000.0,
        );
        draw(
            &mut e,
            &densify(&[[560.0, 265.0], [602.0, 301.0], [558.0, 338.0]], 3.0),
            end + 2000.0,
        );
        assert_eq!(strokes(&e).len(), 2);
    }

    fn zigzag(c: [f32; 2], along: f32, across: f32, passes: usize) -> Vec<[f32; 2]> {
        let pts: Vec<[f32; 2]> = (0..=passes)
            .map(|i| {
                let a = if i % 2 == 0 {
                    -along / 2.0
                } else {
                    along / 2.0
                };
                [
                    c[0] + a,
                    c[1] - across / 2.0 + across * i as f32 / passes as f32,
                ]
            })
            .collect();
        densify(&pts, 4.0)
    }

    #[test]
    fn scratch_out_erases_what_it_covers_in_one_undo_step() {
        let mut e = engine(SmartMode::Auto);
        // A "word" and a far-away stroke.
        let end = draw(
            &mut e,
            &densify(
                &[
                    [300.0, 300.0],
                    [340.0, 320.0],
                    [380.0, 295.0],
                    [420.0, 318.0],
                    [460.0, 300.0],
                ],
                4.0,
            ),
            1000.0,
        );
        let end = draw(
            &mut e,
            &densify(&[[900.0, 600.0], [960.0, 640.0]], 4.0),
            end + 1000.0,
        );
        assert_eq!(strokes(&e).len(), 2);
        let up = draw(
            &mut e,
            &zigzag([380.0, 308.0], 200.0, 40.0, 8),
            end + 1000.0,
        );
        let s = strokes(&e);
        assert_eq!(s.len(), 1, "the word is gone, the scribble is not kept");
        assert!(s[0].points[0].x > 800.0);
        assert!(!e.flashes.is_empty() && e.has_animation(), "a brief flash");
        e.tick(up + 1000.0);
        assert!(e.flashes.is_empty());
        assert!(e.undo());
        assert_eq!(strokes(&e).len(), 2);
        // Over empty board a zig-zag is just ink; with scratch-out off too.
        draw(
            &mut e,
            &zigzag([1200.0, 300.0], 200.0, 40.0, 8),
            end + 9000.0,
        );
        assert_eq!(strokes(&e).len(), 3);
        let mut e = engine(SmartMode::Auto);
        e.set_smart_ink(SmartInk {
            mode: SmartMode::Auto,
            scratch: false,
        });
        let end = draw(
            &mut e,
            &densify(&[[300.0, 300.0], [460.0, 300.0]], 4.0),
            1000.0,
        );
        draw(
            &mut e,
            &zigzag([380.0, 300.0], 200.0, 40.0, 8),
            end + 1000.0,
        );
        assert_eq!(strokes(&e).len(), 2);
    }

    #[test]
    fn hold_to_snap_then_drag_to_resize() {
        let mut e = engine(SmartMode::Hold);
        let pts = densify(
            &[
                [300.0, 300.0],
                [600.0, 300.0],
                [600.0, 480.0],
                [300.0, 480.0],
                [300.0, 310.0],
            ],
            5.0,
        );
        e.pointer_down(
            1,
            pts[0][0],
            pts[0][1],
            0.5,
            0.0,
            PointerKind::Pen,
            0,
            1000.0,
        );
        let mut t = 1000.0;
        for p in &pts[1..] {
            t += 8.0;
            e.pointer_move(1, &[f64::from(p[0]), f64::from(p[1]), 0.5, t], &[], t);
        }
        // Still pen: nothing before 500 ms, a rectangle after.
        e.tick(t + 300.0);
        assert!(e.sink.wet[0].points.len() > 10, "still raw ink");
        e.tick(t + 520.0);
        assert_eq!(
            e.sink.wet[0].points.len(),
            5,
            "snapped while the pen is down"
        );
        // Drag outwards from the centre: bigger.
        e.pointer_move(1, &[380.0, 120.0, 0.5, t + 600.0], &[], t + 600.0);
        e.pointer_move(1, &[150.0, 510.0, 0.5, t + 620.0], &[], t + 620.0);
        e.pointer_up(1, t + 700.0);
        let s = strokes(&e);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].shape.as_deref(), Some("rect"));
        let g = s[0].geom.clone().unwrap();
        assert!(g.w > 450.0, "dragged bigger: {g:?}");
        assert!(e.undo());
        assert_eq!(strokes(&e)[0].shape, None);
        // Without the hold, Hold mode never snaps on pen-up.
        let mut e = engine(SmartMode::Hold);
        draw(&mut e, &circle([500.0, 400.0], 120.0), 1000.0);
        assert_eq!(strokes(&e)[0].shape, None);
    }

    #[test]
    fn shapes_resize_from_geometry_at_constant_width() {
        let mut e = engine(SmartMode::Off);
        e.set_tool(ToolKind::Shape(crate::shapes::ShapeKind::Rect));
        e.pointer_down(1, 100.0, 100.0, 0.5, 0.0, PointerKind::Mouse, 0, 1.0);
        e.pointer_move(1, &[300.0, 200.0, 0.5, 10.0], &[], 2.0);
        e.pointer_up(1, 3.0);
        let id = e.scene.ordered()[0].id.clone();
        assert_eq!(e.selected_shape(), None, "nothing selected");
        e.select(vec![id.clone()]);
        let (sid, b) = e.selected_shape().expect("a resizable shape");
        assert_eq!(
            (sid.as_str(), b),
            (id.as_str(), [100.0, 100.0, 300.0, 200.0])
        );
        let meta: serde_json::Value = serde_json::from_str(&e.selection_meta_json()).unwrap();
        assert_eq!(
            meta["shape"],
            serde_json::json!([100.0, 100.0, 300.0, 200.0])
        );
        let width = strokes(&e)[0].width;
        // Preview leaves the document alone.
        assert!(e.resize_shape(&id, [100.0, 100.0, 500.0, 300.0], false));
        assert_eq!(e.sink.strokes[&id].points[2].x, 500.0);
        assert_eq!(strokes(&e)[0].points[2].x, 300.0);
        e.resize_shape_cancel(&id);
        assert_eq!(e.sink.strokes[&id].points[2].x, 300.0);
        // Commit: one undo step, width unchanged, still selected.
        assert!(e.resize_shape(&id, [100.0, 100.0, 500.0, 300.0], true));
        let s = &strokes(&e)[0];
        assert_eq!(
            (s.points[2].x, s.points[2].y, s.width),
            (500.0, 300.0, width)
        );
        assert_eq!(e.selection(), std::slice::from_ref(&id));
        assert!(e.undo());
        assert_eq!(strokes(&e)[0].points[2].x, 300.0);
        // A moved shape resizes from where it is now.
        e.move_items(std::slice::from_ref(&id), 50.0, 0.0);
        e.select(vec![id.clone()]);
        assert_eq!(e.selected_shape().unwrap().1, [150.0, 100.0, 350.0, 200.0]);
        assert!(e.resize_shape(&id, [150.0, 100.0, 450.0, 200.0], true));
        assert_eq!(strokes(&e)[0].offset, [0.0, 0.0]);
        assert_eq!(e.selected_shape().unwrap().1, [150.0, 100.0, 450.0, 200.0]);
        // Read-only and bad boxes are refused.
        assert!(!e.resize_shape(&id, [f64::NAN, 0.0, 1.0, 1.0], true));
        e.set_read_only(true);
        assert!(!e.resize_shape(&id, [0.0, 0.0, 10.0, 10.0], true));
    }

    #[test]
    fn ink_becomes_text_in_one_undo_step() {
        let mut e = engine(SmartMode::Off);
        e.set_color(0x3e63ddff);
        let end = draw(
            &mut e,
            &densify(&[[300.0, 300.0], [330.0, 360.0], [350.0, 300.0]], 4.0),
            1000.0,
        );
        draw(
            &mut e,
            &densify(&[[370.0, 300.0], [400.0, 360.0]], 4.0),
            end + 300.0,
        );
        e.select_all();
        let ink: serde_json::Value = serde_json::from_str(&e.selection_ink_json()).unwrap();
        assert_eq!(ink["strokes"].as_array().unwrap().len(), 2);
        assert_eq!(ink["strokes"][0]["pts"][2], serde_json::json!(0.0));
        let id = e
            .replace_selection_with_text("Hi", 300.0, 300.0, 100.0, 48.0, end + 1000.0)
            .expect("text added");
        assert_eq!(e.selection(), std::slice::from_ref(&id));
        let items = e.lesson.items(&e.page.clone()).unwrap();
        assert_eq!(items.len(), 1);
        let ItemData::Text(t) = &items[0].data else {
            panic!()
        };
        assert_eq!((t.text.as_str(), t.size, t.color), ("Hi", 48.0, 0x3e63ddff));
        assert!(e.undo());
        assert_eq!(strokes(&e).len(), 2);
        assert_eq!(e.selection_ink_json(), "null");
    }
}
