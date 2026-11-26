//! Ink stays on the page. On a slide page (a page with a frame) the page is
//! the board: a stroke that starts outside it draws nothing, a stroke that
//! crosses the edge is cut at the edge (wet and dry), shapes end at the
//! edge, and placed or moved items stay inside. Infinite pages are never
//! clipped. Turned off with [`Engine::set_clip_ink`].
//!
//! Hooks in the pointer path: `pointer_down` stores the page rect in
//! `ink_clip` (one page read per gesture) and drops ink that starts outside;
//! `update_wet` passes wet ink through [`push_wet`]; `pointer_up` hands the
//! finished stroke to [`Engine::finish_clipped`]; shape ends and selection
//! moves are clamped with [`clamp_point`] and [`Engine::clamp_move`].

use super::smart::Held;
use super::*;

impl<S: InkSink> Engine<S> {
    pub fn set_clip_ink(&mut self, on: bool) {
        self.clip_ink = on;
    }

    pub fn clip_ink(&self) -> bool {
        self.clip_ink
    }

    /// The rect ink is kept in: the current slide page, when clipping is on.
    pub(super) fn ink_bounds(&self) -> Option<[f32; 4]> {
        if !self.clip_ink {
            return None;
        }
        let f = self.lesson.page(&self.page).ok()?.frame?;
        let (w, h) = (f.w as f32, f.h as f32);
        (w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0).then_some([0.0, 0.0, w, h])
    }

    /// A pen, highlighter or shape gesture starting outside the page.
    pub(super) fn starts_off_page(&self, w: [f32; 2]) -> bool {
        matches!(
            self.tool,
            ToolKind::Pen | ToolKind::Highlighter | ToolKind::Shape(_)
        ) && self.ink_clip.is_some_and(|r| !inside(w, r))
    }

    /// Commit a finished stroke cut to the page: one piece goes through
    /// Smart Ink as usual; a stroke that left and re-entered the page is
    /// stored as its pieces, in one undo step. `sampled` is the stroke as
    /// the pointer sampled it (`StrokeBuilder::sampled`), for Smart Ink.
    pub(super) fn finish_clipped(
        &mut self,
        stroke: StrokeData,
        sampled: Vec<StrokePoint>,
        started: f64,
        held: Option<Held>,
        now_ms: f64,
    ) {
        let Some(r) = self.ink_clip else {
            self.finish_ink(stroke, &sampled, started, held, now_ms);
            return;
        };
        let mut runs = clip_points(&stroke.points, r, edge_inset(stroke.width));
        match runs.len() {
            0 => {}
            1 => {
                let points = runs.pop().unwrap_or_default();
                let whole = points.len() == stroke.points.len();
                // An uncut stroke keeps Smart Ink's hold-to-snap result,
                // and is read as sampled; a cut one as what is left of it.
                let held = if whole { held } else { None };
                let seen = if whole { sampled } else { points.clone() };
                self.finish_ink(
                    StrokeData { points, ..stroke },
                    &seen,
                    started,
                    held,
                    now_ms,
                );
            }
            _ => {
                if self.read_only {
                    return;
                }
                let list: Vec<NewItem> = runs
                    .into_iter()
                    .map(|points| NewItem {
                        data: ItemData::Stroke(StrokeData {
                            points,
                            ..stroke.clone()
                        }),
                        created: started,
                        group: None,
                        z: None,
                    })
                    .collect();
                if self.lesson.add_items(&self.page.clone(), &list).is_ok() {
                    self.resync();
                }
            }
        }
    }

    /// Clamp a selection move so the selection stays on the page (one that
    /// is already partly off may still move, but not further out).
    pub(super) fn clamp_move(&self, total: [f32; 2]) -> [f32; 2] {
        let (Some(r), Some(b)) = (self.ink_clip, self.scene.bounds_of(&self.selection)) else {
            return total;
        };
        let axis = |t: f32, lo: f32, hi: f32, min: f32, max: f32| {
            let (a, z) = ((min - lo).min(0.0), (max - hi).max(0.0));
            if a > z { t } else { t.clamp(a, z) }
        };
        [
            axis(total[0], b[0], b[2], r[0], r[2]),
            axis(total[1], b[1], b[3], r[1], r[3]),
        ]
    }

    /// Where text placed at world point `w` goes: inside the page, with
    /// room for `text` at font size `size` (a rough width) below and right.
    pub(super) fn place_on_page(&self, w: [f32; 2], text: &str, size: f64) -> [f32; 2] {
        let Some(r) = self.ink_bounds() else {
            return w;
        };
        let size = size as f32;
        let line = (size * 1.5).clamp(0.0, (r[3] - r[1]) * 0.5);
        let wide =
            (text.chars().count().max(2) as f32 * size * 0.6).clamp(48.0, (r[2] - r[0]) * 0.5);
        [
            w[0].clamp(r[0], (r[2] - wide).max(r[0])),
            w[1].clamp(r[1], (r[3] - line).max(r[1])),
        ]
    }
}

fn inside(p: [f32; 2], r: [f32; 4]) -> bool {
    p[0] >= r[0] && p[0] <= r[2] && p[1] >= r[1] && p[1] <= r[3]
}

/// How far inside the page the centre line stays, so the ink's round end
/// sits at the edge rather than over it.
fn edge_inset(width: f32) -> f32 {
    (width * 0.5).clamp(0.0, 40.0)
}

fn shrink(r: [f32; 4], d: f32) -> [f32; 4] {
    let dx = d.min((r[2] - r[0]) * 0.5);
    let dy = d.min((r[3] - r[1]) * 0.5);
    [r[0] + dx, r[1] + dy, r[2] - dx, r[3] - dy]
}

/// A shape end moved into the page rect, far enough in for a line of
/// `width` to end at the edge; unchanged without a rect.
pub(super) fn clamp_point(p: [f32; 2], r: Option<[f32; 4]>, width: f32) -> [f32; 2] {
    match r {
        Some(r) => {
            let r = shrink(r, edge_inset(width));
            [p[0].clamp(r[0], r[2]), p[1].clamp(r[1], r[3])]
        }
        None => p,
    }
}

/// Wet ink on its way to the renderer, cut to the page rect.
pub(super) fn push_wet(out: &mut Vec<WetStroke>, w: WetStroke, r: Option<[f32; 4]>) {
    let Some(r) = r else {
        out.push(w);
        return;
    };
    let runs = clip_points(&w.points, r, edge_inset(w.width));
    if runs.len() == 1 && runs[0].len() == w.points.len() {
        out.push(w);
        return;
    }
    for points in runs {
        out.push(WetStroke {
            points,
            ..w.clone()
        });
    }
}

/// Cut a polyline to the rect `r` shrunk by `inset`: the runs inside, with
/// points added exactly where it crosses the edge (Liang–Barsky).
pub(crate) fn clip_points(pts: &[StrokePoint], r: [f32; 4], inset: f32) -> Vec<Vec<StrokePoint>> {
    let r = shrink(r, inset);
    match pts {
        [] => return Vec::new(),
        [p] => {
            return if inside([p.x, p.y], r) {
                vec![vec![*p]]
            } else {
                Vec::new()
            };
        }
        _ => {}
    }
    let mut out: Vec<Vec<StrokePoint>> = Vec::new();
    let mut run: Vec<StrokePoint> = Vec::new();
    let close = |run: &mut Vec<StrokePoint>, out: &mut Vec<Vec<StrokePoint>>| {
        if run.len() >= 2 {
            out.push(std::mem::take(run));
        }
        run.clear();
    };
    for w in pts.windows(2) {
        let (p, q) = (w[0], w[1]);
        match segment_in(p, q, r) {
            None => close(&mut run, &mut out),
            Some((t0, t1)) => {
                if t0 > 0.0 || run.is_empty() {
                    close(&mut run, &mut out);
                    run.push(super::duster::lerp(p, q, t0));
                }
                run.push(if t1 >= 1.0 {
                    q
                } else {
                    super::duster::lerp(p, q, t1)
                });
                if t1 < 1.0 {
                    close(&mut run, &mut out);
                }
            }
        }
    }
    close(&mut run, &mut out);
    out
}

/// The part of segment p→q inside the rect, as parameters `(t0, t1)`.
fn segment_in(p: StrokePoint, q: StrokePoint, r: [f32; 4]) -> Option<(f32, f32)> {
    let (dx, dy) = (q.x - p.x, q.y - p.y);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (pp, qq) in [
        (-dx, p.x - r[0]),
        (dx, r[2] - p.x),
        (-dy, p.y - r[1]),
        (dy, r[3] - p.y),
    ] {
        if pp == 0.0 {
            if qq < 0.0 {
                return None;
            }
        } else {
            let t = qq / pp;
            if pp < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return None;
            }
        }
    }
    Some((t0, t1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(xy: &[(f32, f32)]) -> Vec<StrokePoint> {
        xy.iter()
            .map(|&(x, y)| StrokePoint {
                x,
                y,
                p: 0.5,
                dt: 0,
            })
            .collect()
    }

    #[test]
    fn a_stroke_crossing_the_edge_is_cut_exactly_at_it() {
        let r = [0.0, 0.0, 100.0, 100.0];
        let runs = clip_points(&pts(&[(50.0, 50.0), (150.0, 50.0)]), r, 0.0);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].len(), 2);
        assert!((runs[0][1].x - 100.0).abs() < 1e-4);
        // Out and back in: two pieces.
        let runs = clip_points(
            &pts(&[(50.0, 50.0), (150.0, 50.0), (150.0, 80.0), (50.0, 80.0)]),
            r,
            2.0,
        );
        assert_eq!(runs.len(), 2);
        assert!((runs[1][0].x - 98.0).abs() < 1e-4);
        // Entirely inside: unchanged.
        let inside_pts = pts(&[(10.0, 10.0), (20.0, 20.0), (30.0, 10.0)]);
        assert_eq!(clip_points(&inside_pts, r, 0.0), vec![inside_pts.clone()]);
        // Entirely outside: nothing.
        assert!(clip_points(&pts(&[(200.0, 0.0), (300.0, 0.0)]), r, 0.0).is_empty());
        // A segment cutting a corner from outside to outside.
        let runs = clip_points(&pts(&[(-10.0, 50.0), (50.0, -10.0)]), r, 0.0);
        assert_eq!(runs.len(), 1);
    }

    #[test]
    fn wet_ink_is_split_like_dry_ink() {
        let mut out = Vec::new();
        let w = WetStroke {
            points: pts(&[(50.0, 50.0), (150.0, 50.0), (150.0, 80.0), (50.0, 80.0)]),
            width: 4.0,
            color: 0,
            opacity: 1.0,
            brush: rupok_ink::Brush::Pen,
        };
        push_wet(&mut out, w.clone(), None);
        assert_eq!(out.len(), 1);
        out.clear();
        push_wet(&mut out, w, Some([0.0, 0.0, 100.0, 100.0]));
        assert_eq!(out.len(), 2);
        assert!(
            out.iter()
                .flat_map(|w| &w.points)
                .all(|p| p.x <= 98.0 + 1e-4)
        );
    }
}
