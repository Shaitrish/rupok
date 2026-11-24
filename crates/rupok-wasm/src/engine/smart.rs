//! Smart Ink in the engine: shape snapping when the pen lifts (Auto) or after
//! a short hold while it is still down (Hold to snap), the two-stroke arrow,
//! scratch-out to erase, resizing shape strokes from their stored geometry,
//! and the ink → text replacement used by handwriting conversion.
//!
//! Undo model: the hand-drawn stroke is committed first and the snap is a
//! second change that rewrites it in place (same id, `z` and `created`), so
//! the first undo brings back the ink exactly as drawn. A scratch-out is one
//! change that removes what it covered (the scribble itself is never kept).

use std::collections::{HashMap, HashSet};

use rupok_core::{
    BoxGeom, Edits, Item, ItemData, ItemId, NewItem, ShapeGeom, StrokeData, TextData,
};
use rupok_ink::recognize::{
    self, Context, Neighbour, Options, Proposal, Registry, ShapeKind, family,
};
use rupok_ink::{Brush, StrokePoint};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SmartInk {
    pub mode: SmartMode,
    /// Scratch-out to erase (only while smart shapes are on).
    pub scratch: bool,
    /// Enabled template families ([`family`] bits).
    pub families: u32,
}

impl Default for SmartInk {
    fn default() -> Self {
        Self {
            mode: SmartMode::Off,
            scratch: true,
            families: family::DEFAULT,
        }
    }
}

/// Deferred recognisers (a number line) run once the pen has rested this
/// long (ms) after the last stroke.
pub const DEFER_MS: f64 = 1100.0;
/// A connector's end attaches to a shape within this many screen px.
const ATTACH_PX: f32 = 28.0;

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
    /// The stroke as drawn, when it snapped to a line: a small "+" crossing
    /// it next puts it back (see [`recognize::small_cross`]).
    raw: Option<(ItemId, StrokeData)>,
}

/// A fading flash in the wet layer.
pub(super) struct Flash {
    strokes: Vec<WetStroke>,
    start: f64,
}

/// A stroke that is a line shape, not moved since it snapped.
fn stroke_is_line(it: Option<&Item>) -> bool {
    matches!(it, Some(Item { data: ItemData::Stroke(s), .. })
        if s.shape.as_deref() == Some("line") && s.offset == [0.0, 0.0])
}

fn median_pressure(pts: &[StrokePoint]) -> f32 {
    if pts.is_empty() {
        return shapes::SHAPE_PRESSURE;
    }
    let mut p: Vec<f32> = pts.iter().map(|q| q.p).collect();
    p.sort_by(f32::total_cmp);
    p[p.len() / 2]
}

fn seg_dist(q: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (vx, vy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = vx * vx + vy * vy;
    let t = if l2 <= 1e-12 {
        0.0
    } else {
        (((q[0] - a[0]) * vx + (q[1] - a[1]) * vy) / l2).clamp(0.0, 1.0)
    };
    (a[0] + t * vx - q[0]).hypot(a[1] + t * vy - q[1])
}

/// A connector's ends: on each attached shape, the attach point nearest
/// the other end (twice, so both ends settle); a free end stays put.
fn route(
    sa: Option<&ShapeGeom>,
    a: [f64; 2],
    sb: Option<&ShapeGeom>,
    b: [f64; 2],
) -> ([f64; 2], [f64; 2]) {
    let nearest = |g: &ShapeGeom, to: [f64; 2], or: [f64; 2]| {
        shapes::attach_points(g)
            .into_iter()
            .min_by(|p, q| {
                let d = |x: [f64; 2]| (x[0] - to[0]).hypot(x[1] - to[1]);
                d(*p).total_cmp(&d(*q))
            })
            .unwrap_or(or)
    };
    let centre = |g: &ShapeGeom| {
        shapes::geom_aabb(g).map_or([g.x, g.y], |b| [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0])
    };
    let (mut a, mut b) = (a, b);
    // Start from the other shape's centre, so a box below another links
    // bottom to top.
    let (ta, tb) = (sb.map_or(b, centre), sa.map_or(a, centre));
    if let Some(g) = sa {
        a = nearest(g, ta, a);
    }
    if let Some(g) = sb {
        b = nearest(g, tb, b);
    }
    for _ in 0..2 {
        if let Some(g) = sa {
            a = nearest(g, b, a);
        }
        if let Some(g) = sb {
            b = nearest(g, a, b);
        }
    }
    (a, b)
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
        ..Default::default()
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
                families: self.smart.families,
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
            brush: st.brush,
        })
    }

    fn opts(&self, deliberate: bool) -> Options {
        Options {
            px_per_unit: self.camera.zoom,
            deliberate,
            families: self.smart.families,
        }
    }

    /// Commit a finished pen or highlighter stroke, applying Smart Ink.
    ///
    /// The stroke is always committed as drawn first; a snap or composite
    /// is a second change, so the first undo brings the ink back.
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
        let opts = self.opts(false);
        if held.is_none() && self.smart.scratch && recognize::is_scratch_out(&stroke.points, opts) {
            let covered = self.scratch_covered(&stroke);
            if !covered.is_empty() {
                self.scratch_erase(&stroke, covered, now_ms);
                return;
            }
        }
        // Standalone shapes snap on pen-up only in Auto, and never in the
        // middle of handwriting; composites may always fire.
        let standalone =
            self.smart.mode == SmartMode::Auto && !self.writing_context(&stroke, started);
        let ctx = Context {
            opts,
            neighbours: self.neighbours(&stroke, started, None),
        };
        let prop = match held {
            Some(_) => None,
            None => Registry::standard().recognize(&stroke.points, &ctx, standalone),
        };
        let bounds = stroke.aabb();
        let unsnap = if prop.is_none() {
            self.unsnap_for_plus(&stroke, &ctx)
        } else {
            None
        };
        let Some(id) = self.commit_stroke(stroke.clone(), started) else {
            return;
        };
        if let Some((rid, raw)) = unsnap {
            self.replace_stroke(&rid, raw, &[]);
        }
        let (snapped, composite) = match (held, prop) {
            (Some(h), _) => {
                let geom = self.connect(h.current, &id);
                (self.snap_stroke(&id, &stroke, h.shape, geom), false)
            }
            (None, Some(p)) => {
                let composite = !p.absorbs.is_empty()
                    || p.join.is_some()
                    || !p.extra.is_empty()
                    || !p.labels.is_empty();
                (self.apply(&ctx, &p, &id, &stroke), composite)
            }
            (None, None) => (false, false),
        };
        self.last_ink = (!composite).then(|| LastInk {
            ended: now_ms,
            bounds,
            size_px: (bounds[2] - bounds[0]).max(bounds[3] - bounds[1]) * self.camera.zoom,
            snapped,
            raw: (snapped && stroke_is_line(self.scene.get(&id)))
                .then(|| (id.clone(), stroke.clone())),
        });
        // Read it again once the pen rests (number lines).
        self.pending = (!snapped && Registry::standard().has_deferred(&opts))
            .then_some((id, now_ms + DEFER_MS));
    }

    /// The second stroke of a small "+" (`s`, not yet committed): the line
    /// its first stroke snapped to a moment ago goes back to ink, so the
    /// whole "+" stays writing.
    fn unsnap_for_plus(&self, s: &StrokeData, ctx: &Context) -> Option<(ItemId, StrokeData)> {
        let (rid, raw) = self.last_ink.as_ref()?.raw.clone()?;
        let xy: Vec<[f32; 2]> = s.points.iter().map(|q| [q.x, q.y]).collect();
        let i = recognize::small_cross(&xy, ctx)?;
        (ctx.neighbours[i].id == rid && stroke_is_line(self.scene.get(&rid))).then_some((rid, raw))
    }

    /// Per frame: run the deferred recognisers once the pen has rested.
    pub(super) fn deferred_tick(&mut self, now_ms: f64) -> bool {
        let Some((id, due)) = self.pending.clone() else {
            return false;
        };
        if now_ms < due
            || self
                .gestures
                .values()
                .any(|g| matches!(g, Gesture::Ink { .. }))
        {
            return false;
        }
        self.pending = None;
        if self.read_only || self.smart.mode == SmartMode::Off {
            return false;
        }
        let Some(Item {
            data: ItemData::Stroke(s),
            created,
            ..
        }) = self.scene.get(&id).cloned()
        else {
            return false;
        };
        let pts: Vec<[f32; 2]> = s
            .points
            .iter()
            .map(|q| [q.x + s.offset[0], q.y + s.offset[1]])
            .collect();
        let ctx = Context {
            opts: self.opts(false),
            neighbours: self.neighbours(&s, created, Some(&id)),
        };
        match Registry::standard().recognize_deferred(&pts, &ctx) {
            Some(p) => {
                let world = StrokeData {
                    points: s
                        .points
                        .iter()
                        .map(|q| StrokePoint {
                            x: q.x + s.offset[0],
                            y: q.y + s.offset[1],
                            ..*q
                        })
                        .collect(),
                    offset: [0.0, 0.0],
                    ..s
                };
                self.apply(&ctx, &p, &id, &world)
            }
            None => false,
        }
    }

    /// Strokes near `s` for contextual recognisers, most recent first, with
    /// offsets applied and clean geometry for shapes. `skip` leaves out the
    /// stroke itself when it is already on the board.
    fn neighbours(&self, s: &StrokeData, started: f64, skip: Option<&ItemId>) -> Vec<Neighbour> {
        // A stroke already on the board is read again for a composite that
        // spans the screen (a number line's ticks): look wider.
        let (max, reach) = if skip.is_some() {
            (64, 900.0)
        } else {
            (32, 80.0)
        };
        let b = s.aabb();
        let pad = (0.5 * (b[2] - b[0]).max(b[3] - b[1])).max(reach / self.camera.zoom);
        let mut near: Vec<&Item> = self
            .scene
            .query([b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad])
            .into_iter()
            .filter(|it| matches!(it.data, ItemData::Stroke(_)) && Some(&it.id) != skip)
            .collect();
        near.sort_by(|a, b| b.created.total_cmp(&a.created).then(b.id.cmp(&a.id)));
        near.truncate(max);
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

    /// Apply a proposal for the stroke `raw_id` (already committed, `raw`
    /// in world coordinates) as one change: the result replaces the first
    /// absorbed neighbour (keeping its style and replay time) or the stroke
    /// itself; the other absorbed strokes go; extra parts and labels join a
    /// group with it.
    fn apply(&mut self, ctx: &Context, p: &Proposal, raw_id: &ItemId, raw: &StrokeData) -> bool {
        let onto_raw = p.absorbs.is_empty() || p.onto_stroke;
        let target_id = if onto_raw {
            raw_id.clone()
        } else {
            ctx.neighbours[p.absorbs[0]].id.clone()
        };
        let Some(Item {
            data: ItemData::Stroke(target),
            created,
            group: target_group,
            ..
        }) = self.scene.get(&target_id).cloned()
        else {
            return false;
        };
        let (name, mut geom) = shapes::from_recognized(&p.recognized);
        if onto_raw
            && p.extra.is_empty()
            && p.labels.is_empty()
            && p.join.is_none()
            && p.absorbs.is_empty()
        {
            geom = self.connect(geom, raw_id);
        } else if !onto_raw {
            // A connector that became an arrow keeps its links.
            let old = target.geom.as_ref().map(|g| {
                shapes::translate(g, f64::from(target.offset[0]), f64::from(target.offset[1]))
            });
            if let Some(o) = old.filter(|o| o.from.is_some() || o.to.is_some())
                && matches!(geom.kind.as_str(), "line" | "arrow")
            {
                let same = (geom.x - o.x).abs() < 1e-3 && (geom.y - o.y).abs() < 1e-3;
                (geom.from, geom.to) = if same { (o.from, o.to) } else { (o.to, o.from) };
            }
        }
        let pressure = if onto_raw {
            median_pressure(&raw.points)
        } else {
            target
                .points
                .first()
                .map(|q| q.p)
                .unwrap_or(shapes::SHAPE_PRESSURE)
        };
        let src = if onto_raw { raw } else { &target };
        let duration = src.points.last().map(|q| q.dt).unwrap_or(0);
        let make = |g: ShapeGeom, name: &str, dur: u32| -> Option<StrokeData> {
            Some(StrokeData {
                points: shapes::with_timing(shapes::geom_points(&g, src.width, pressure)?, dur),
                offset: [0.0, 0.0],
                shape: Some(name.into()),
                geom: Some(g),
                ..src.clone()
            })
        };
        let Some(main) = make(geom, name, duration) else {
            return false;
        };
        let mut e = Edits::default();
        e.strokes.push((target_id.clone(), main));
        if target_id != *raw_id {
            e.remove.push(raw_id.clone());
        }
        for &i in &p.absorbs {
            let id = &ctx.neighbours[i].id;
            if *id != target_id && !e.remove.contains(id) {
                e.remove.push(id.clone());
            }
        }
        // Labels a result redraws.
        for &i in &p.relabel {
            if let Some(g) = self
                .scene
                .get(&ctx.neighbours[i].id)
                .and_then(|it| it.group.clone())
            {
                e.remove.extend(self.group_labels(&g));
            }
        }
        let grouped = !p.extra.is_empty() || !p.labels.is_empty() || p.join.is_some();
        let group = if !grouped {
            None
        } else if let Some(j) = p.join {
            let jid = ctx.neighbours[j].id.clone();
            match self.scene.get(&jid).and_then(|it| it.group.clone()) {
                Some(g) => Some(g),
                None => {
                    let g = self.lesson.new_group_id();
                    e.groups.push((jid, Some(g.clone())));
                    Some(g)
                }
            }
        } else {
            Some(self.lesson.new_group_id())
        };
        if group.is_some() && group != target_group {
            e.groups.push((target_id.clone(), group.clone()));
        }
        let mut t = created;
        for r in &p.extra {
            let (n, g) = shapes::from_recognized(r);
            t += 1.0;
            if let Some(d) = make(g, n, duration / 2) {
                e.add.push(NewItem {
                    data: ItemData::Stroke(d),
                    created: t,
                    group: group.clone(),
                    z: None,
                });
            }
        }
        for l in &p.labels {
            t += 1.0;
            let size = f64::from(l.size);
            let chars = l.text.chars().count().max(1) as f64;
            let w = 0.62 * size * chars + 0.4 * size;
            e.add.push(NewItem {
                data: ItemData::Text(TextData {
                    latex: false,
                    geom: BoxGeom {
                        x: f64::from(l.at[0]) - 0.55 * size * chars / 2.0 - 0.1 * size,
                        y: f64::from(l.at[1]) - 0.65 * size,
                        w,
                        h: 1.3 * size,
                        rot: 0.0,
                    },
                    text: l.text.clone(),
                    size,
                    color: src.color,
                    stamp: None,
                }),
                created: t,
                group: group.clone(),
                z: None,
            });
        }
        let page = self.page.clone();
        let Ok(added) = self.lesson.apply_edits(&page, &e) else {
            return false;
        };
        let mut changed: Vec<ItemId> = e.strokes.iter().map(|(id, _)| id.clone()).collect();
        changed.extend(e.groups.iter().map(|(id, _)| id.clone()));
        changed.extend(added);
        self.refresh(&changed, &e.remove);
        true
    }

    /// Text items in group `g` (labels Smart Ink added with a template).
    fn group_labels(&self, g: &str) -> Vec<ItemId> {
        self.scene
            .ordered()
            .iter()
            .filter(|it| it.group.as_deref() == Some(g) && matches!(it.data, ItemData::Text(_)))
            .map(|it| it.id.clone())
            .collect()
    }

    /// Bring scene and renderer in line after a change to a few items.
    fn refresh(&mut self, changed: &[ItemId], removed: &[ItemId]) {
        for r in removed {
            self.scene.remove(r);
            self.sink.remove_stroke(r);
        }
        self.selection.retain(|s| !removed.contains(s));
        let page = self.page.clone();
        for id in changed {
            if removed.contains(id) {
                continue;
            }
            if let Ok(it) = self.lesson.item(&page, id) {
                if let ItemData::Stroke(s) = &it.data {
                    self.sink.add_stroke(id, s);
                }
                self.scene.insert(it);
            }
        }
        self.bump();
    }

    // --------------------------------------------------------- connectors

    /// World geometry of a stroke's shape, if it has one.
    pub(super) fn world_geom(&self, id: &str) -> Option<ShapeGeom> {
        let ItemData::Stroke(s) = &self.scene.get(id)?.data else {
            return None;
        };
        let g = s.geom.as_ref()?;
        Some(shapes::translate(
            g,
            f64::from(s.offset[0]),
            f64::from(s.offset[1]),
        ))
    }

    /// The shape an end at `p` attaches to: one within [`ATTACH_PX`] of
    /// its outline, or that `p` is inside.
    fn attach_target(&self, p: [f64; 2], skip: &str) -> Option<(ItemId, ShapeGeom)> {
        let r = f64::from(ATTACH_PX / self.camera.zoom);
        let (x, y) = (p[0] as f32, p[1] as f32);
        let rr = r as f32;
        let mut best: Option<(f64, ItemId, ShapeGeom)> = None;
        for it in self.scene.query([x - rr, y - rr, x + rr, y + rr]) {
            if it.id == skip {
                continue;
            }
            let Some(g) = self.world_geom(&it.id) else {
                continue;
            };
            if shapes::attach_points(&g).is_empty() {
                continue;
            }
            let Some(out) = shapes::geom_points(&g, 1.0, 0.5) else {
                continue;
            };
            let b = shapes::geom_aabb(&g)?;
            let inside = p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3];
            let d = out
                .windows(2)
                .map(|w| {
                    seg_dist(
                        p,
                        [f64::from(w[0].x), f64::from(w[0].y)],
                        [f64::from(w[1].x), f64::from(w[1].y)],
                    )
                })
                .fold(f64::INFINITY, f64::min);
            let score = if inside { d.min(r) * 0.5 } else { d };
            if (inside || d <= r) && best.as_ref().is_none_or(|(s, _, _)| score < *s) {
                best = Some((score, it.id.clone(), g));
            }
        }
        best.map(|(_, id, g)| (id, g))
    }

    /// Snap a fresh line or arrow's ends to the shapes they touch and link
    /// them (Flowchart family).
    fn connect(&self, g: ShapeGeom, self_id: &str) -> ShapeGeom {
        if !matches!(g.kind.as_str(), "line" | "arrow")
            || self.smart.families & family::FLOWCHART == 0
        {
            return g;
        }
        let (a, b) = ([g.x, g.y], [g.x + g.w, g.y + g.h]);
        let ta = self.attach_target(a, self_id);
        let tb = self.attach_target(b, self_id);
        if ta.is_none() && tb.is_none()
            || ta
                .as_ref()
                .zip(tb.as_ref())
                .is_some_and(|(x, y)| x.0 == y.0)
        {
            return g;
        }
        let (na, nb) = route(ta.as_ref().map(|t| &t.1), a, tb.as_ref().map(|t| &t.1), b);
        ShapeGeom {
            x: na[0],
            y: na[1],
            w: nb[0] - na[0],
            h: nb[1] - na[1],
            from: ta.map(|t| t.0),
            to: tb.map(|t| t.0),
            ..g
        }
    }

    /// Rewrites of the connectors attached to shapes whose world geometry
    /// changes to `moved` (a move or a resize). `carried` are items moved
    /// along with them: a carried connector whose shape stays put lets go.
    pub(super) fn connector_updates(
        &self,
        moved: &HashMap<ItemId, ShapeGeom>,
        carried: &HashMap<ItemId, [f64; 2]>,
    ) -> Vec<(ItemId, StrokeData)> {
        if moved.is_empty() && carried.is_empty() {
            return Vec::new();
        }
        let mut seen: HashSet<ItemId> = HashSet::new();
        let mut out = Vec::new();
        let mut cands: Vec<ItemId> = Vec::new();
        for id in moved.keys().chain(carried.keys()) {
            if let Some(it) = self.scene.get(id) {
                let b = it.data.aabb();
                let pad = 8.0 / self.camera.zoom;
                cands.extend(
                    self.scene
                        .query([b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad])
                        .into_iter()
                        .map(|c| c.id.clone()),
                );
            }
        }
        for cid in cands {
            if !seen.insert(cid.clone()) {
                continue;
            }
            let Some(Item {
                data: ItemData::Stroke(s),
                ..
            }) = self.scene.get(&cid)
            else {
                continue;
            };
            let Some(g) = s
                .geom
                .as_ref()
                .filter(|g| g.from.is_some() || g.to.is_some())
            else {
                continue;
            };
            let delta = carried.get(&cid).copied();
            let linked_moves = |l: &Option<String>| {
                l.as_ref()
                    .is_some_and(|i| moved.contains_key(i) || carried.contains_key(i))
            };
            if delta.is_none() && !linked_moves(&g.from) && !linked_moves(&g.to) {
                continue;
            }
            let mut w = shapes::translate(g, f64::from(s.offset[0]), f64::from(s.offset[1]));
            if let Some(d) = delta {
                w = shapes::translate(&w, d[0], d[1]);
            }
            // Where each linked shape is now.
            let shape_of = |l: &Option<String>| -> Option<ShapeGeom> {
                let id = l.as_ref()?;
                if let Some(g) = moved.get(id) {
                    return Some(g.clone());
                }
                let mut g = self.world_geom(id)?;
                if let Some(d) = carried.get(id) {
                    g = shapes::translate(&g, d[0], d[1]);
                }
                Some(g)
            };
            let keep = |l: &Option<String>| match (delta, l) {
                // Dragged away on its own: let go of shapes left behind.
                (Some(_), Some(i)) => moved.contains_key(i) || carried.contains_key(i),
                (_, Some(i)) => self.scene.get(i).is_some(),
                _ => false,
            };
            let (from, to) = (
                g.from.clone().filter(|_| keep(&g.from)),
                g.to.clone().filter(|_| keep(&g.to)),
            );
            let (sa, sb) = (shape_of(&from), shape_of(&to));
            let (a, b) = route(sa.as_ref(), [w.x, w.y], sb.as_ref(), [w.x + w.w, w.y + w.h]);
            let ng = ShapeGeom {
                x: a[0],
                y: a[1],
                w: b[0] - a[0],
                h: b[1] - a[1],
                from,
                to,
                ..w
            };
            let p = s
                .points
                .first()
                .map(|q| q.p)
                .unwrap_or(shapes::SHAPE_PRESSURE);
            let duration = s.points.last().map(|q| q.dt).unwrap_or(0);
            if let Some(pts) = shapes::geom_points(&ng, s.width, p) {
                out.push((
                    cid.clone(),
                    StrokeData {
                        points: shapes::with_timing(pts, duration),
                        offset: [0.0, 0.0],
                        geom: Some(ng),
                        ..s.clone()
                    },
                ));
            }
        }
        out
    }

    /// Connector rewrites for moving `ids` by `d`.
    pub(super) fn connectors_for_move(
        &self,
        ids: &[ItemId],
        d: [f64; 2],
    ) -> Vec<(ItemId, StrokeData)> {
        let carried: HashMap<ItemId, [f64; 2]> = ids.iter().map(|id| (id.clone(), d)).collect();
        self.connector_updates(&HashMap::new(), &carried)
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
            brush: Brush::Pen,
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
        let b = shapes::geom_aabb(&self.world_geom(id)?)?;
        Some((id.clone(), b))
    }

    /// A selection that resizes as shapes: only shape strokes (with stored
    /// geometry) and text labels, at least one shape. Its bounds (geometry
    /// and text boxes) in world units.
    pub fn selected_shapes(&self) -> Option<[f64; 4]> {
        let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        let mut any = false;
        for id in &self.selection {
            let it = self.scene.get(id)?;
            let r = match &it.data {
                ItemData::Stroke(_) => {
                    any = true;
                    shapes::geom_aabb(&self.world_geom(id)?)?
                }
                ItemData::Text(t) => [t.geom.x, t.geom.y, t.geom.x + t.geom.w, t.geom.y + t.geom.h],
                _ => return None,
            };
            b = [
                b[0].min(r[0]),
                b[1].min(r[1]),
                b[2].max(r[2]),
                b[3].max(r[3]),
            ];
        }
        any.then_some(b)
    }

    /// Resize a shape stroke so its geometric bounds become `to`: the points
    /// are regenerated from `geom`, so the line width stays the same.
    /// `commit: false` only previews (renderer), `true` is one undo step.
    pub fn resize_shape(&mut self, id: &str, to: [f64; 4], commit: bool) -> bool {
        if self.selection.len() == 1 && self.selection[0] == id {
            return self.resize_shapes(to, commit);
        }
        let keep = std::mem::replace(&mut self.selection, vec![id.to_string()]);
        let ok = self.resize_shapes(to, commit);
        if !commit {
            self.selection = keep;
        }
        ok
    }

    /// Resize the selected shapes (see [`Engine::selected_shapes`]) so their
    /// bounds become `to`: every shape is regenerated from its geometry in
    /// the new box, labels move with it (keeping their size), and attached
    /// connectors follow. One undo step on `commit`.
    pub fn resize_shapes(&mut self, to: [f64; 4], commit: bool) -> bool {
        if self.read_only || !to.iter().all(|v| v.is_finite()) || to[2] < to[0] || to[3] < to[1] {
            return false;
        }
        let Some(from) = self.selected_shapes() else {
            return false;
        };
        let sx = if from[2] - from[0] < 1e-6 {
            1.0
        } else {
            (to[2] - to[0]) / (from[2] - from[0])
        };
        let sy = if from[3] - from[1] < 1e-6 {
            1.0
        } else {
            (to[3] - to[1]) / (from[3] - from[1])
        };
        let map = |p: [f64; 2]| [to[0] + (p[0] - from[0]) * sx, to[1] + (p[1] - from[1]) * sy];
        let mut strokes: Vec<(ItemId, StrokeData)> = Vec::new();
        let mut boxes: Vec<(ItemId, BoxGeom, Option<f64>)> = Vec::new();
        let mut moved: HashMap<ItemId, ShapeGeom> = HashMap::new();
        for id in self.selection.clone() {
            match &self.scene.get(&id).map(|it| it.data.clone()) {
                Some(ItemData::Stroke(s)) => {
                    let Some(world) = self.world_geom(&id) else {
                        return false;
                    };
                    let Some(gb) = shapes::geom_aabb(&world) else {
                        return false;
                    };
                    let (a, b) = (map([gb[0], gb[1]]), map([gb[2], gb[3]]));
                    let Some(ng) = shapes::map_box(&world, gb, [a[0], a[1], b[0], b[1]]) else {
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
                    moved.insert(id.clone(), ng.clone());
                    strokes.push((
                        id,
                        StrokeData {
                            points: shapes::with_timing(pts, duration),
                            offset: [0.0, 0.0],
                            geom: Some(ng),
                            ..s.clone()
                        },
                    ));
                }
                Some(ItemData::Text(t)) => {
                    let c = map([t.geom.x + t.geom.w / 2.0, t.geom.y + t.geom.h / 2.0]);
                    boxes.push((
                        id,
                        BoxGeom {
                            x: c[0] - t.geom.w / 2.0,
                            y: c[1] - t.geom.h / 2.0,
                            ..t.geom
                        },
                        None,
                    ));
                }
                _ => return false,
            }
        }
        let sel: HashSet<ItemId> = self.selection.iter().cloned().collect();
        let conn: Vec<(ItemId, StrokeData)> = self
            .connector_updates(&moved, &HashMap::new())
            .into_iter()
            .filter(|(id, _)| !sel.contains(id))
            .collect();
        if !commit {
            for (id, d) in strokes.iter().chain(conn.iter()) {
                self.sink.add_stroke(id, d);
            }
            self.dirty = true;
            return true;
        }
        strokes.extend(conn);
        let changed: Vec<ItemId> = strokes
            .iter()
            .map(|(id, _)| id.clone())
            .chain(boxes.iter().map(|(id, _, _)| id.clone()))
            .collect();
        let e = Edits {
            strokes,
            boxes,
            ..Default::default()
        };
        if self.lesson.apply_edits(&self.page.clone(), &e).is_err() {
            return false;
        }
        self.refresh(&changed, &[]);
        true
    }

    /// Drop a resize preview: draw the selection (and its connectors) as
    /// stored again.
    pub fn resize_shape_cancel(&mut self, id: &str) {
        let mut ids: Vec<ItemId> = self.selection.clone();
        ids.push(id.to_string());
        for id in ids {
            if let Some(Item {
                data: ItemData::Stroke(s),
                ..
            }) = self.scene.get(&id)
            {
                let s = s.clone();
                self.sink.add_stroke(&id, &s);
            }
        }
        // Connectors previewed elsewhere snap back too.
        for (cid, _) in self.connectors_for_move(&self.selection.clone(), [0.0, 0.0]) {
            if let Some(Item {
                data: ItemData::Stroke(s),
                ..
            }) = self.scene.get(&cid)
            {
                let s = s.clone();
                self.sink.add_stroke(&cid, &s);
            }
        }
        self.dirty = true;
    }

    // ------------------------------------------------------ make it live

    /// The selected graph, for "Make it live": `{"fam": "quad" | "trig",
    /// "params": {...}, "step": n, "ids": [...]}` for an axes shape and one
    /// fitted line, parabola or sine curve in the selection (exponentials
    /// have no live preset yet); `null` otherwise.
    pub fn graph_live_json(&self) -> String {
        let mut axes = None;
        let mut curve = None;
        for id in &self.selection {
            let Some(Item {
                data: ItemData::Stroke(s),
                ..
            }) = self.scene.get(id)
            else {
                continue;
            };
            match s.shape.as_deref() {
                Some("axes") => axes = self.world_geom(id),
                Some(k @ ("graph-line" | "graph-quad" | "graph-sine")) if curve.is_none() => {
                    curve = Some((
                        k,
                        self.world_geom(id),
                        s.points
                            .iter()
                            .map(|q| [q.x + s.offset[0], q.y + s.offset[1]])
                            .collect::<Vec<_>>(),
                    ));
                }
                _ => {}
            }
        }
        let (Some(ax), Some((kind, _, pts))) = (axes, curve) else {
            return "null".into();
        };
        let Some(frame) = shapes::to_geometry(&ax).and_then(|g| recognize::graph::Frame::of(&g))
        else {
            return "null".into();
        };
        let k = match kind {
            "graph-line" => ShapeKind::GraphLine,
            "graph-quad" => ShapeKind::GraphQuad,
            _ => ShapeKind::GraphSine,
        };
        let Some(fit) = recognize::graph::refit(&frame, &pts, k) else {
            return "null".into();
        };
        let p = &fit.params;
        let (fam, params, step) = match k {
            ShapeKind::GraphLine => (
                "quad",
                serde_json::json!({"a": 0.0, "b": p[0], "c": p[1]}),
                5,
            ),
            ShapeKind::GraphQuad => (
                "quad",
                serde_json::json!({"a": p[0], "b": p[1], "c": p[2]}),
                5,
            ),
            _ => (
                "trig",
                serde_json::json!({"a": p[0], "b": p[1], "c": p[2], "d": p[3]}),
                0,
            ),
        };
        serde_json::json!({
            "fam": fam,
            "params": params,
            "step": step,
            "ids": self.selection,
        })
        .to_string()
    }

    /// Replace items with a Smart Object (single-file HTML) at the centre
    /// of the view, at build step `step`, with saved state `state`: one undo
    /// step brings the items back. Returns the object's id.
    #[allow(clippy::too_many_arguments)]
    pub fn replace_with_object_html(
        &mut self,
        remove: &[ItemId],
        html: &str,
        title: &str,
        w: f64,
        h: f64,
        steps: u32,
        step: u32,
        state: Option<&str>,
        now_ms: f64,
    ) -> Result<ItemId, String> {
        if self.read_only {
            return Err("read-only".into());
        }
        let bundle = rupok_format::ObjectBundle::from_single_html(html, title, w, h, steps);
        let errors: Vec<String> = bundle
            .validate()
            .into_iter()
            .filter(|d| d.severity == rupok_format::Severity::Error)
            .map(|d| d.message)
            .collect();
        if !errors.is_empty() {
            return Err(errors.join("\n"));
        }
        let slug = super::slug(title, "object");
        let mut bundle_id = slug.clone();
        let mut n = 2;
        while self.pkg.objects.contains_key(&bundle_id) {
            bundle_id = format!("{slug}-{n}");
            n += 1;
        }
        self.pkg.objects.insert(bundle_id.clone(), bundle);
        let (c, _) = self.visible_area();
        let (cx, cy) = self.camera.screen_to_world(c[0], c[1]);
        let data = ItemData::Object(rupok_core::ObjectData {
            geom: BoxGeom {
                x: f64::from(cx) - w / 2.0,
                y: f64::from(cy) - h / 2.0,
                w,
                h,
                rot: 0.0,
            },
            bundle: bundle_id,
            title: title.into(),
            step: step.min(steps),
            state: state.filter(|s| s.len() <= 256 * 1024).map(str::to_string),
        });
        let present: Vec<ItemId> = remove
            .iter()
            .filter(|id| self.scene.get(id).is_some())
            .cloned()
            .collect();
        let id = self
            .lesson
            .replace_items(&self.page.clone(), &present, &data, now_ms)
            .map_err(|e| e.to_string())?;
        self.resync();
        self.selection = vec![id.clone()];
        self.bump();
        Ok(id)
    }

    // ------------------------------------------------- maths preview

    /// The handwriting around the last pen stroke, for the maths live
    /// preview: plain (unsnapped) pen strokes of writing size, chained by
    /// closeness, written within two minutes, in writing order.
    /// `{"ids", "bounds": [x0, y0, x1, y1], "xy": [x, y, …], "counts",
    /// "color"}` (world units, offsets applied), or `null`.
    pub fn math_cluster_json(&self) -> String {
        const MAX: usize = 60;
        const SMALL_PX: f32 = 220.0;
        let zoom = self.camera.zoom.max(1e-4);
        let plain = |it: &Item| -> Option<StrokeData> {
            let ItemData::Stroke(s) = &it.data else {
                return None;
            };
            let b = s.aabb();
            let big = (b[2] - b[0]).max(b[3] - b[1]) * zoom > SMALL_PX;
            (s.tool == "pen" && s.shape.is_none() && !big).then(|| s.clone())
        };
        let Some(start) = self.last_pen.as_ref().and_then(|id| self.scene.get(id)) else {
            return "null".into();
        };
        let Some(first) = plain(start) else {
            return "null".into();
        };
        let since = start.created - 120_000.0;
        let mut picked: Vec<(f64, ItemId, StrokeData)> =
            vec![(start.created, start.id.clone(), first)];
        let mut seen: HashSet<ItemId> = HashSet::from([start.id.clone()]);
        let mut i = 0;
        while i < picked.len() && picked.len() < MAX {
            let b = picked[i].2.aabb();
            let h = (b[3] - b[1]).max(b[2] - b[0]);
            let pad = (0.9 * h).max(36.0 / zoom);
            for it in self
                .scene
                .query([b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad])
            {
                if seen.contains(&it.id) || it.created < since {
                    continue;
                }
                if let Some(s) = plain(it) {
                    seen.insert(it.id.clone());
                    picked.push((it.created, it.id.clone(), s));
                }
            }
            i += 1;
        }
        picked.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        let (mut xy, mut counts) = (Vec::new(), Vec::new());
        for (_, _, s) in &picked {
            let b = s.aabb();
            bounds = [
                bounds[0].min(b[0]),
                bounds[1].min(b[1]),
                bounds[2].max(b[2]),
                bounds[3].max(b[3]),
            ];
            counts.push(s.points.len());
            for q in &s.points {
                xy.push(q.x + s.offset[0]);
                xy.push(q.y + s.offset[1]);
            }
        }
        serde_json::json!({
            "ids": picked.iter().map(|p| &p.1).collect::<Vec<_>>(),
            "bounds": bounds,
            "xy": xy,
            "counts": counts,
            "color": super::css_color(picked[0].2.color),
        })
        .to_string()
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
        self.replace_selection_with(text, false, x, y, w, size, now_ms)
    }

    /// [`Engine::replace_selection_with_text`] for typeset maths: the item's
    /// text is LaTeX, shown with KaTeX.
    pub fn replace_selection_with_latex(
        &mut self,
        latex: &str,
        x: f64,
        y: f64,
        w: f64,
        size: f64,
        now_ms: f64,
    ) -> Option<ItemId> {
        self.replace_selection_with(latex, true, x, y, w, size, now_ms)
    }

    #[allow(clippy::too_many_arguments)]
    fn replace_selection_with(
        &mut self,
        text: &str,
        latex: bool,
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
            latex,
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
            stamp: None,
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
            families: family::ALL,
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
    fn a_snapped_shape_keeps_the_pen_style() {
        let mut e = engine(SmartMode::Auto);
        e.set_brush(Brush::Chalk);
        draw(&mut e, &circle([500.0, 400.0], 120.0), 1000.0);
        let s = strokes(&e);
        assert_eq!(s[0].shape.as_deref(), Some("circle"));
        assert_eq!(s[0].brush.as_deref(), Some("chalk"));
        // Magic ink never snaps (nor is it saved).
        let mut e = engine(SmartMode::Auto);
        e.set_brush(Brush::Magic);
        draw(&mut e, &circle([500.0, 400.0], 120.0), 1000.0);
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
            families: family::ALL,
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

    fn texts(e: &Engine<MockSink>) -> Vec<String> {
        e.scene
            .ordered()
            .iter()
            .filter_map(|i| match &i.data {
                ItemData::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect()
    }

    fn by_shape(e: &Engine<MockSink>, name: &str) -> Option<Item> {
        e.scene
            .ordered()
            .into_iter()
            .find(|i| matches!(&i.data, ItemData::Stroke(s) if s.shape.as_deref() == Some(name)))
            .cloned()
    }

    #[test]
    fn a_plus_makes_axes_and_a_curve_joins_them_with_its_equation() {
        let mut e = engine(SmartMode::Auto);
        let end = draw(
            &mut e,
            &densify(&[[150.0, 450.0], [850.0, 452.0]], 5.0),
            1000.0,
        );
        let end = draw(
            &mut e,
            &densify(&[[400.0, 800.0], [402.0, 120.0]], 5.0),
            end + 300.0,
        );
        let axes = by_shape(&e, "axes").expect("axes");
        assert_eq!(strokes(&e).len(), 1, "both strokes became one axes shape");
        assert_eq!(texts(&e), ["x", "y"]);
        let g = axes.group.clone().expect("grouped with its labels");
        // First undo: the two lines as they were (the first snapped).
        assert!(e.undo());
        assert_eq!(strokes(&e).len(), 2);
        assert!(texts(&e).is_empty());
        assert!(e.redo());
        // A parabola sketched over them: y = x² in tick units.
        let ItemData::Stroke(axs) = &axes.data else {
            panic!()
        };
        let f =
            recognize::graph::Frame::of(&shapes::to_geometry(axs.geom.as_ref().unwrap()).unwrap())
                .unwrap();
        let pts: Vec<[f32; 2]> = (0..=80)
            .map(|i| {
                let x = -2.5 + 5.0 * i as f32 / 80.0;
                [f.o[0] + x * f.ux, f.o[1] - x * x * f.uy]
            })
            .collect();
        draw(&mut e, &pts, end + 3000.0);
        let c = by_shape(&e, "graph-quad").expect("fitted");
        assert_eq!(c.group.as_deref(), Some(g.as_str()));
        assert!(texts(&e).contains(&"y = x²".to_string()));
        // Make it live: the quadratic preset at its last step.
        e.select(vec![c.id.clone()]);
        let live: serde_json::Value = serde_json::from_str(&e.graph_live_json()).unwrap();
        assert_eq!(live["fam"], "quad");
        assert_eq!(live["step"], 5);
        assert!((live["params"]["a"].as_f64().unwrap() - 1.0).abs() < 0.01);
        let ids: Vec<ItemId> = serde_json::from_value(live["ids"].clone()).unwrap();
        assert_eq!(ids.len(), 5, "axes, curve and three labels");
        let html = "<!doctype html><html><head><title>t</title></head><body>hi</body></html>";
        let state = r#"{"fam":"quad","quad":{"a":1,"b":0,"c":0}}"#;
        let id = e
            .replace_with_object_html(
                &ids,
                html,
                "Function grapher",
                1920.0,
                1080.0,
                5,
                5,
                Some(state),
                9e9,
            )
            .expect("object");
        let items = e.lesson.items(&e.page.clone()).unwrap();
        assert_eq!(items.len(), 1);
        let ItemData::Object(o) = &items[0].data else {
            panic!()
        };
        assert_eq!((o.step, o.state.as_deref()), (5, Some(state)));
        assert_eq!(items[0].id, id);
        assert!(e.undo());
        assert_eq!(e.lesson.items(&e.page.clone()).unwrap().len(), 5);
    }

    #[test]
    fn a_number_line_is_read_when_the_pen_rests() {
        let mut e = engine(SmartMode::Auto);
        let mut t = draw(
            &mut e,
            &densify(&[[100.0, 400.0], [900.0, 401.0]], 5.0),
            1000.0,
        );
        for x in [300.0, 400.0, 500.0, 600.0, 700.0] {
            t = draw(
                &mut e,
                &densify(&[[x, 384.0], [x + 1.0, 416.0]], 3.0),
                t + 250.0,
            );
        }
        assert_eq!(strokes(&e).len(), 6, "nothing yet");
        e.tick(t + 500.0);
        assert_eq!(strokes(&e).len(), 6, "still drawing time");
        e.tick(t + 1200.0);
        let s = strokes(&e);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].shape.as_deref(), Some("numberline"));
        assert_eq!(texts(&e), ["−3", "−2", "−1", "0", "1", "2", "3"]);
        // One undo: the line and all five ticks as drawn.
        assert!(e.undo());
        assert_eq!(strokes(&e).len(), 6);
        assert!(texts(&e).is_empty());
    }

    #[test]
    fn connectors_stick_to_shapes_and_follow_them() {
        let mut e = engine(SmartMode::Off);
        e.set_tool(ToolKind::Shape(crate::shapes::ShapeKind::Rect));
        for (a, b) in [
            ([100.0, 100.0], [300.0, 200.0]),
            ([100.0, 400.0], [300.0, 500.0]),
        ] {
            e.pointer_down(1, a[0], a[1], 0.5, 0.0, PointerKind::Mouse, 0, 1.0);
            e.pointer_move(1, &[b[0], b[1], 0.5, 10.0], &[], 2.0);
            e.pointer_up(1, 3.0);
        }
        let boxes: Vec<ItemId> = e.scene.ordered().iter().map(|i| i.id.clone()).collect();
        e.set_tool(ToolKind::Pen);
        e.set_smart_ink(SmartInk {
            mode: SmartMode::Auto,
            scratch: true,
            families: family::ALL,
        });
        // An arrow from near the top box's bottom to near the other's top.
        let mut path = densify(&[[205.0, 215.0], [198.0, 388.0]], 4.0);
        path.extend(densify(&[[198.0, 388.0], [185.0, 365.0]], 3.0));
        path.extend(densify(
            &[[185.0, 365.0], [198.0, 388.0], [212.0, 366.0]],
            3.0,
        ));
        draw(&mut e, &path, 5000.0);
        let conn = by_shape(&e, "arrow").expect("arrow");
        let ItemData::Stroke(cs) = &conn.data else {
            panic!()
        };
        let g = cs.geom.clone().unwrap();
        assert_eq!(
            (g.from.as_ref(), g.to.as_ref()),
            (Some(&boxes[0]), Some(&boxes[1]))
        );
        assert_eq!(
            (g.x, g.y, g.x + g.w, g.y + g.h),
            (200.0, 200.0, 200.0, 400.0),
            "anchored s → n"
        );
        // Move the lower box right: the arrow follows, in the same undo step.
        e.move_items(std::slice::from_ref(&boxes[1]), 300.0, 0.0);
        let ItemData::Stroke(cs) = &e.scene.get(&conn.id).unwrap().data else {
            panic!()
        };
        let g2 = cs.geom.clone().unwrap();
        assert!(
            (g2.x + g2.w - 500.0).abs() < 1e-6 || (g2.x + g2.w - 400.0).abs() < 1e-6,
            "{g2:?}"
        );
        assert_eq!(g2.to.as_ref(), Some(&boxes[1]));
        assert!(e.undo());
        let ItemData::Stroke(cs) = &e.scene.get(&conn.id).unwrap().data else {
            panic!()
        };
        assert_eq!(cs.geom.clone().unwrap(), g);
        // Resizing a box drags its connector end too.
        e.select(vec![boxes[0].clone()]);
        assert!(e.resize_shapes([100.0, 50.0, 300.0, 150.0], true));
        let ItemData::Stroke(cs) = &e.scene.get(&conn.id).unwrap().data else {
            panic!()
        };
        assert_eq!(cs.geom.clone().unwrap().y, 150.0);
        // Dragged away alone, the arrow lets go.
        e.move_items(std::slice::from_ref(&conn.id), 500.0, 0.0);
        let ItemData::Stroke(cs) = &e.scene.get(&conn.id).unwrap().data else {
            panic!()
        };
        let g3 = cs.geom.clone().unwrap();
        assert_eq!((g3.from, g3.to), (None, None));
    }

    #[test]
    fn a_group_of_shapes_and_labels_resizes_together() {
        let mut e = engine(SmartMode::Auto);
        let end = draw(
            &mut e,
            &densify(&[[150.0, 450.0], [850.0, 452.0]], 5.0),
            1000.0,
        );
        draw(
            &mut e,
            &densify(&[[400.0, 800.0], [402.0, 120.0]], 5.0),
            end + 300.0,
        );
        let axes = by_shape(&e, "axes").unwrap();
        e.select(vec![axes.id.clone()]);
        assert_eq!(e.selection().len(), 3, "the axes and their labels");
        let b = e.selected_shapes().expect("resizable");
        let to = [b[0], b[1], b[0] + (b[2] - b[0]) * 2.0, b[3]];
        assert!(e.resize_shapes(to, true));
        let nb = e.selected_shapes().unwrap();
        // Labels keep their size, so the box grows a little less.
        assert!(nb[2] - nb[0] > 1.8 * (b[2] - b[0]), "{nb:?} vs {to:?}");
        let ItemData::Stroke(s) = &e.scene.get(&axes.id).unwrap().data else {
            panic!()
        };
        assert_eq!(s.width, e.pen.width, "width unchanged");
        assert!(e.undo());
        let back = e.selected_shapes();
        e.select(vec![axes.id.clone()]);
        assert!(back.is_some());
        assert!((e.selected_shapes().unwrap()[2] - b[2]).abs() < 1e-6);
    }

    #[test]
    fn families_off_keep_templates_out() {
        let mut e = engine(SmartMode::Auto);
        e.set_smart_ink(SmartInk {
            families: family::ALL & !family::GRAPHS,
            ..e.smart_ink()
        });
        let end = draw(
            &mut e,
            &densify(&[[150.0, 450.0], [850.0, 452.0]], 5.0),
            1000.0,
        );
        draw(
            &mut e,
            &densify(&[[400.0, 800.0], [402.0, 120.0]], 5.0),
            end + 300.0,
        );
        assert!(by_shape(&e, "axes").is_none());
        assert_eq!(strokes(&e).len(), 2);
    }

    #[test]
    fn handwriting_cluster_and_latex_replace() {
        let mut e = engine(SmartMode::Auto);
        // Two small strokes side by side (a "+" and a "1"), and a far one.
        let mut t = draw(
            &mut e,
            &densify(&[[300.0, 320.0], [340.0, 320.0]], 3.0),
            1000.0,
        );
        t = draw(
            &mut e,
            &densify(&[[320.0, 300.0], [320.0, 340.0]], 3.0),
            t + 200.0,
        );
        t = draw(
            &mut e,
            &densify(&[[1200.0, 700.0], [1210.0, 740.0]], 3.0),
            t + 200.0,
        );
        let c: serde_json::Value = serde_json::from_str(&e.math_cluster_json()).unwrap();
        assert_eq!(c["ids"].as_array().unwrap().len(), 1, "only the far stroke");
        draw(
            &mut e,
            &densify(&[[360.0, 300.0], [362.0, 340.0]], 3.0),
            t + 200.0,
        );
        let c: serde_json::Value = serde_json::from_str(&e.math_cluster_json()).unwrap();
        let ids: Vec<ItemId> = serde_json::from_value(c["ids"].clone()).unwrap();
        assert_eq!(ids.len(), 3, "the + and the 1, in writing order");
        assert_eq!(c["counts"].as_array().unwrap().len(), 3);
        e.select(ids);
        let id = e
            .replace_selection_with_latex("+1", 300.0, 300.0, 80.0, 40.0, t + 1000.0)
            .expect("maths item");
        let ItemData::Text(td) = &e.scene.get(&id).unwrap().data else {
            panic!()
        };
        assert!(td.latex && td.text == "+1");
        let dom: serde_json::Value = serde_json::from_str(&e.dom_items_json()).unwrap();
        assert_eq!(dom[0]["latex"], true);
        assert!(e.undo());
        assert_eq!(strokes(&e).len(), 4, "one undo brings the ink back");
    }
    #[test]
    fn a_small_plus_stays_ink() {
        let mut e = engine(SmartMode::Auto);
        let end = draw(
            &mut e,
            &densify(&[[350.0, 400.0], [450.0, 401.0]], 3.0),
            1000.0,
        );
        assert_eq!(strokes(&e)[0].shape.as_deref(), Some("line"));
        draw(
            &mut e,
            &densify(&[[401.0, 350.0], [399.0, 450.0], [409.0, 440.0]], 3.0),
            end + 250.0,
        );
        let s = strokes(&e);
        assert_eq!(s.len(), 2);
        assert!(s.iter().all(|s| s.shape.is_none()), "both strokes are ink");
        assert!(s.iter().all(|s| s.geom.is_none()));
        // A short arrow still works.
        let end = draw(
            &mut e,
            &densify(&[[600.0, 400.0], [700.0, 400.0]], 3.0),
            end + 5000.0,
        );
        draw(
            &mut e,
            &densify(&[[665.0, 370.0], [701.0, 400.0], [665.0, 430.0]], 3.0),
            end + 250.0,
        );
        assert_eq!(strokes(&e).last().unwrap().shape.as_deref(), Some("arrow"));
    }
}
