//! Editing a selection in place, from the floating selection toolbar:
//! recolour, re-thicken, change the pen style (brush) of, and scale ink,
//! shapes and text. Each change is one
//! undo step (`Lesson::apply_edits`). Shapes keep their geometry: they are
//! regenerated from it, so a scaled rectangle is still a rectangle.

use rupok_core::Edits;

use super::*;
use crate::shapes;

/// Thinnest and thickest stroke a restyle can make, world units.
const MIN_WIDTH: f32 = 0.5;
const MAX_WIDTH: f32 = 200.0;
/// Smallest side a scaled selection keeps, world units.
const MIN_SIDE: f64 = 4.0;

impl<S: InkSink> Engine<S> {
    /// The selection's shared colour (`#rrggbbaa`), stroke width (when every
    /// stroke has the same) and whether all of its ink is highlighter, for
    /// the selection toolbar.
    pub(super) fn selection_style(&self) -> serde_json::Value {
        let mut color: Option<u32> = None;
        let mut mixed_color = false;
        let mut width: Option<f32> = None;
        let mut mixed_width = false;
        let mut strokes = 0;
        let mut highlighter = 0;
        let mut brush: Option<&str> = None;
        let mut mixed_brush = false;
        for it in self.selection.iter().filter_map(|id| self.scene.get(id)) {
            let c = match &it.data {
                ItemData::Stroke(s) => {
                    strokes += 1;
                    if s.tool == "highlighter" {
                        highlighter += 1;
                    }
                    let b = s.brush().as_str();
                    match brush {
                        None => brush = Some(b),
                        Some(x) if x != b => mixed_brush = true,
                        _ => {}
                    }
                    match width {
                        None => width = Some(s.width),
                        Some(w) if (w - s.width).abs() > 1e-3 => mixed_width = true,
                        _ => {}
                    }
                    s.color
                }
                ItemData::Text(t) => t.color,
                _ => continue,
            };
            match color {
                None => color = Some(c),
                Some(x) if x >> 8 != c >> 8 => mixed_color = true,
                _ => {}
            }
        }
        serde_json::json!({
            "color": if mixed_color { None } else { color.map(css_color) },
            "width": if mixed_width { None } else { width },
            "highlighter": strokes > 0 && strokes == highlighter,
            "brush": if mixed_brush { None } else { brush },
        })
    }

    /// Restyle the selection as one undo step: `color` (0xRRGGBBAA) for
    /// strokes and text, and a stroke width, either absolute (`width`) or a
    /// factor of each stroke's own (`width_scale`), and a pen style
    /// (`brush`; magic ink is not a stored style and is ignored).
    /// Highlighter strokes keep their own opacity. Returns false when
    /// nothing changed.
    pub fn restyle_selection(
        &mut self,
        color: Option<u32>,
        width: Option<f32>,
        width_scale: Option<f32>,
        brush: Option<Brush>,
    ) -> bool {
        let brush = brush.filter(|b| *b != Brush::Magic);
        if self.read_only || self.selection.is_empty() {
            return false;
        }
        let width = width.filter(|w| w.is_finite() && *w > 0.0);
        let width_scale = width_scale.filter(|k| k.is_finite() && *k > 0.0);
        let mut e = Edits::default();
        for id in self.selection.clone() {
            let Some(it) = self.scene.get(&id) else {
                continue;
            };
            match &it.data {
                ItemData::Stroke(s) => {
                    let mut n = s.clone();
                    if let Some(c) = color {
                        n.color = if s.tool == "highlighter" {
                            (c & 0xffff_ff00) | 0xff
                        } else {
                            c
                        };
                    }
                    let w = match (width, width_scale) {
                        (Some(w), _) => w,
                        (None, Some(k)) => s.width * k,
                        _ => s.width,
                    }
                    .clamp(MIN_WIDTH, MAX_WIDTH);
                    if (w - s.width).abs() > 1e-4 {
                        n.width = w;
                        // Arrow heads and templates are sized by the width.
                        if let Some(g) = self.world_geom(&id) {
                            let p = s
                                .points
                                .first()
                                .map(|q| q.p)
                                .unwrap_or(shapes::SHAPE_PRESSURE);
                            let duration = s.points.last().map(|q| q.dt).unwrap_or(0);
                            if let Some(pts) = shapes::geom_points(&g, w, p) {
                                n.points = shapes::with_timing(pts, duration);
                                n.offset = [0.0, 0.0];
                                n.geom = Some(g);
                            }
                        }
                    }
                    if let Some(b) = brush {
                        n.brush = b.key().map(Into::into);
                    }
                    if n != *s {
                        e.strokes.push((id, n));
                    }
                }
                ItemData::Text(t) => {
                    if let Some(c) = color
                        && c != t.color
                    {
                        e.text_colors.push((id, c));
                    }
                }
                _ => {}
            }
        }
        if e.is_empty() {
            return false;
        }
        if self.lesson.apply_edits(&self.page.clone(), &e).is_err() {
            return false;
        }
        self.resync();
        true
    }

    /// Scale the selection so its bounds ([`Engine::selection_bounds`])
    /// become `to` (`[x0, y0, x1, y1]`, world units): ink points and widths
    /// scale, shapes are regenerated from their geometry, text scales its
    /// font, images and objects their box. `commit: false` previews the ink
    /// (the UI previews boxes); `true` is one undo step.
    pub fn scale_selection(&mut self, to: [f64; 4], commit: bool) -> bool {
        if self.read_only || !to.iter().all(|v| v.is_finite()) {
            return false;
        }
        let Some(b) = self.scene.bounds_of(&self.selection) else {
            return false;
        };
        let from = b.map(f64::from);
        let (fw, fh) = (from[2] - from[0], from[3] - from[1]);
        let (tw, th) = (to[2] - to[0], to[3] - to[1]);
        if tw < MIN_SIDE.min(fw) || th < MIN_SIDE.min(fh) {
            return false;
        }
        let sx = if fw < 1e-6 { 1.0 } else { tw / fw };
        let sy = if fh < 1e-6 { 1.0 } else { th / fh };
        // Widths and font sizes follow the mean scale.
        let k = (sx * sy).sqrt();
        let map = |p: [f64; 2]| [to[0] + (p[0] - from[0]) * sx, to[1] + (p[1] - from[1]) * sy];
        let mut e = Edits::default();
        for id in self.selection.clone() {
            let Some(it) = self.scene.get(&id) else {
                continue;
            };
            match &it.data {
                ItemData::Stroke(s) => {
                    let width = (s.width * k as f32).clamp(MIN_WIDTH, MAX_WIDTH);
                    let shaped = self.world_geom(&id).and_then(|g| {
                        let gb = shapes::geom_aabb(&g)?;
                        let (a, b) = (map([gb[0], gb[1]]), map([gb[2], gb[3]]));
                        let ng = shapes::map_box(&g, gb, [a[0], a[1], b[0], b[1]])?;
                        let p = s
                            .points
                            .first()
                            .map(|q| q.p)
                            .unwrap_or(shapes::SHAPE_PRESSURE);
                        let duration = s.points.last().map(|q| q.dt).unwrap_or(0);
                        let pts = shapes::geom_points(&ng, width, p)?;
                        Some((shapes::with_timing(pts, duration), ng))
                    });
                    let n = match shaped {
                        Some((points, geom)) => StrokeData {
                            points,
                            width,
                            offset: [0.0, 0.0],
                            geom: Some(geom),
                            ..s.clone()
                        },
                        None => {
                            let (ox, oy) = (f64::from(s.offset[0]), f64::from(s.offset[1]));
                            StrokeData {
                                points: s
                                    .points
                                    .iter()
                                    .map(|q| {
                                        let m = map([f64::from(q.x) + ox, f64::from(q.y) + oy]);
                                        StrokePoint {
                                            x: m[0] as f32,
                                            y: m[1] as f32,
                                            ..*q
                                        }
                                    })
                                    .collect(),
                                width,
                                offset: [0.0, 0.0],
                                // Ink that is not a known shape scales as ink.
                                geom: None,
                                ..s.clone()
                            }
                        }
                    };
                    e.strokes.push((id, n));
                }
                ItemData::Text(t) => {
                    let g = t.geom;
                    let a = map([g.x, g.y]);
                    e.boxes.push((
                        id,
                        BoxGeom {
                            x: a[0],
                            y: a[1],
                            w: (g.w * k).max(1.0),
                            h: (g.h * k).max(1.0),
                            rot: g.rot,
                        },
                        Some((t.size * k).clamp(4.0, 2000.0)),
                    ));
                }
                other => {
                    let Some(g) = box_geom(other) else {
                        continue;
                    };
                    let (a, b) = (map([g.x, g.y]), map([g.x + g.w, g.y + g.h]));
                    e.boxes.push((
                        id,
                        BoxGeom {
                            x: a[0],
                            y: a[1],
                            w: (b[0] - a[0]).max(1.0),
                            h: (b[1] - a[1]).max(1.0),
                            rot: g.rot,
                        },
                        None,
                    ));
                }
            }
        }
        if e.is_empty() {
            return false;
        }
        if !commit {
            for (id, d) in &e.strokes {
                self.sink.add_stroke(id, d);
            }
            self.dirty = true;
            return true;
        }
        if self.lesson.apply_edits(&self.page.clone(), &e).is_err() {
            return false;
        }
        self.resync();
        true
    }

    /// Scale the selection by `factor` about its centre (one undo step),
    /// keeping it on a slide page when it fits there.
    pub fn scale_selection_by(&mut self, factor: f64) -> bool {
        if !(factor.is_finite() && factor > 0.0) {
            return false;
        }
        let Some(b) = self.scene.bounds_of(&self.selection) else {
            return false;
        };
        let b = b.map(f64::from);
        let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
        let (hw, hh) = ((b[2] - b[0]) / 2.0 * factor, (b[3] - b[1]) / 2.0 * factor);
        let mut to = [cx - hw, cy - hh, cx + hw, cy + hh];
        if let Some(r) = self.ink_bounds().map(|r| r.map(f64::from)) {
            for (lo, hi) in [(0, 2), (1, 3)] {
                let size = to[hi] - to[lo];
                if size <= r[hi] - r[lo] {
                    let shift = (r[lo] - to[lo]).max(0.0) + (r[hi] - to[hi]).min(0.0);
                    to[lo] += shift;
                    to[hi] += shift;
                }
            }
        }
        self.scale_selection(to, true)
    }

    /// Drop a scale preview: draw the selected strokes as stored again.
    pub fn scale_cancel(&mut self) {
        for id in self.selection.clone() {
            if let Some(Item {
                data: ItemData::Stroke(s),
                ..
            }) = self.scene.get(&id)
            {
                let s = s.clone();
                self.sink.add_stroke(&id, &s);
            }
        }
        self.dirty = true;
    }
}

fn box_geom(d: &ItemData) -> Option<BoxGeom> {
    match d {
        ItemData::Image(i) => Some(i.geom),
        ItemData::Object(o) => Some(o.geom),
        ItemData::Text(t) => Some(t.geom),
        _ => None,
    }
}
