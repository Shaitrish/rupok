//! The partial eraser, a board duster: rubbing removes only the ink under
//! the eraser and splits a stroke it crosses into the pieces either side.
//!
//! While the rub goes on, cut strokes leave the scene and the renderer at
//! once and their pieces take their place under temporary ids. When the rub
//! ends, [`Engine::commit_cut`] writes the whole rub as **one** change: the
//! stored strokes it touched are removed and the surviving pieces added at
//! the same stacking order, so one undo brings the strokes back whole.
//! Stored points never change: a piece is a new stroke made of a run of the
//! old points (plus points interpolated where the eraser edge falls).

use std::collections::BTreeMap;

use rupok_core::Edits;

use super::*;

/// What the eraser tool (and the palm duster) removes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EraserMode {
    /// Only the ink under the eraser; strokes split. The default.
    Partial,
    /// Every stroke the eraser touches, whole.
    Stroke,
}

impl EraserMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "stroke" => EraserMode::Stroke,
            _ => EraserMode::Partial,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            EraserMode::Partial => "partial",
            EraserMode::Stroke => "stroke",
        }
    }
}

/// Prefix of a piece's temporary id; stored ids never start with it.
const PIECE: &str = "~cut";
/// Most points interpolated into one segment near the eraser.
const MAX_SPLIT: usize = 256;

struct Piece {
    data: StrokeData,
    created: f64,
    group: Option<String>,
    z: f64,
}

/// A rub in progress.
#[derive(Default)]
pub(super) struct Cut {
    /// Stored strokes the rub cut or erased.
    removed: Vec<ItemId>,
    /// Pieces on the board now, by temporary id.
    pieces: BTreeMap<ItemId, Piece>,
    /// Strokes the rub has touched (for "erased N" feedback).
    touched: usize,
}

impl<S: InkSink> Engine<S> {
    pub fn set_eraser_mode(&mut self, m: EraserMode) {
        self.eraser_mode = m;
    }

    pub fn eraser_mode(&self) -> EraserMode {
        self.eraser_mode
    }

    /// Strokes the rub in progress has touched.
    pub(super) fn cut_touched(&self) -> usize {
        self.cut.as_ref().map_or(0, |c| c.touched)
    }

    /// Rub out the ink under a circle of world radius `r` swept from `a`
    /// to `b`. Returns how many strokes it cut.
    pub(super) fn cut_at(&mut self, a: [f32; 2], b: [f32; 2], r: f32) -> usize {
        if self.read_only || !(r.is_finite() && r > 0.0) {
            return 0;
        }
        let mut n = 0;
        for id in self.scene.erase_hits(a, b, r) {
            let Some(item) = self.scene.get(&id) else {
                continue;
            };
            let ItemData::Stroke(s) = &item.data else {
                continue;
            };
            let Some(parts) = cut_stroke(s, a, b, r) else {
                continue;
            };
            let (z, created, group) = (item.z, item.created, item.group.clone());
            self.scene.remove(&id);
            self.sink.remove_stroke(&id);
            let cut = self.cut.get_or_insert_with(Cut::default);
            // A piece cut again is simply replaced; a stored stroke is removed.
            if cut.pieces.remove(&id).is_none() {
                cut.removed.push(id.clone());
                cut.touched += 1;
            }
            for data in parts {
                self.cut_seq += 1;
                let pid = format!("{PIECE}{}", self.cut_seq);
                self.sink.add_stroke(&pid, &data);
                self.scene.insert(Item {
                    id: pid.clone(),
                    z,
                    created,
                    group: group.clone(),
                    data: ItemData::Stroke(data.clone()),
                });
                cut.pieces.insert(
                    pid,
                    Piece {
                        data,
                        created,
                        group: group.clone(),
                        z,
                    },
                );
            }
            n += 1;
        }
        if n > 0 {
            self.dirty = true;
        }
        n
    }

    /// Write the rub as one change (one undo step) and swap the pieces'
    /// temporary ids for stored ones.
    pub(super) fn commit_cut(&mut self) {
        let Some(cut) = self.cut.take() else {
            return;
        };
        if cut.removed.is_empty() {
            return;
        }
        if !self.read_only {
            let e = Edits {
                remove: cut.removed.clone(),
                add: cut
                    .pieces
                    .into_values()
                    .map(|p| NewItem {
                        data: ItemData::Stroke(p.data),
                        created: p.created,
                        group: p.group,
                        z: Some(p.z),
                    })
                    .collect(),
                ..Default::default()
            };
            let _ = self.lesson.apply_edits(&self.page.clone(), &e);
        }
        self.selection.retain(|s| !cut.removed.contains(s));
        self.resync();
    }
}

/// Cut a stroke with an eraser circle of radius `r` swept from `a` to `b`
/// (world units). `None` when the eraser misses the ink; otherwise the
/// pieces that survive (possibly none). Pieces are plain ink: a cut shape
/// loses its geometry.
pub(crate) fn cut_stroke(
    s: &StrokeData,
    a: [f32; 2],
    b: [f32; 2],
    r: f32,
) -> Option<Vec<StrokeData>> {
    let (ox, oy) = (s.offset[0], s.offset[1]);
    // Work in the stroke's own coordinates.
    let a = [a[0] - ox, a[1] - oy];
    let b = [b[0] - ox, b[1] - oy];
    let reach = |p: &StrokePoint| r + rupok_ink::point_radius(p.p, s.width);
    let hit = |p: &StrokePoint| seg_point_dist(a, b, [p.x, p.y]) <= reach(p);
    let pts = &s.points;
    match pts.len() {
        0 => return None,
        1 => return hit(&pts[0]).then(Vec::new),
        _ => {}
    }
    // Interpolate segments near the eraser so the cut follows its edge.
    let step = (r * 0.35).max(0.25);
    let lo = [a[0].min(b[0]), a[1].min(b[1])];
    let hi = [a[0].max(b[0]), a[1].max(b[1])];
    let mut dense: Vec<StrokePoint> = Vec::with_capacity(pts.len() + 16);
    dense.push(pts[0]);
    for w in pts.windows(2) {
        let (p, q) = (w[0], w[1]);
        let m = reach(&p).max(reach(&q));
        let near = p.x.min(q.x) <= hi[0] + m
            && p.x.max(q.x) >= lo[0] - m
            && p.y.min(q.y) <= hi[1] + m
            && p.y.max(q.y) >= lo[1] - m;
        let len = ((q.x - p.x).powi(2) + (q.y - p.y).powi(2)).sqrt();
        if near && len > step {
            let n = ((len / step).ceil() as usize).clamp(2, MAX_SPLIT);
            for k in 1..n {
                dense.push(lerp(p, q, k as f32 / n as f32));
            }
        }
        dense.push(q);
    }
    let mask: Vec<bool> = dense.iter().map(hit).collect();
    if !mask.iter().any(|&m| m) {
        return None;
    }
    let mut out = Vec::new();
    let mut run: Vec<StrokePoint> = Vec::new();
    let mut flush = |run: &mut Vec<StrokePoint>| {
        if run.len() >= 2 {
            out.push(StrokeData {
                points: std::mem::take(run),
                shape: None,
                geom: None,
                ..s.clone()
            });
        }
        run.clear();
    };
    for (p, m) in dense.into_iter().zip(mask) {
        if m {
            flush(&mut run);
        } else {
            run.push(p);
        }
    }
    flush(&mut run);
    Some(out)
}

pub(super) fn lerp(p: StrokePoint, q: StrokePoint, t: f32) -> StrokePoint {
    StrokePoint {
        x: p.x + (q.x - p.x) * t,
        y: p.y + (q.y - p.y) * t,
        p: p.p + (q.p - p.p) * t,
        dt: (p.dt as f64 + (q.dt as f64 - p.dt as f64) * f64::from(t)).round() as u32,
    }
}

fn seg_point_dist(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    let (vx, vy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = vx * vx + vy * vy;
    let t = if l2 <= f32::EPSILON {
        0.0
    } else {
        (((p[0] - a[0]) * vx + (p[1] - a[1]) * vy) / l2).clamp(0.0, 1.0)
    };
    ((a[0] + t * vx - p[0]).powi(2) + (a[1] + t * vy - p[1]).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(x0: f32, x1: f32, y: f32, n: usize) -> StrokeData {
        StrokeData {
            tool: "pen".into(),
            color: 0x000000ff,
            width: 4.0,
            opacity: 1.0,
            points: (0..n)
                .map(|i| StrokePoint {
                    x: x0 + (x1 - x0) * i as f32 / (n - 1) as f32,
                    y,
                    p: 0.5,
                    dt: i as u32 * 10,
                })
                .collect(),
            offset: [0.0, 0.0],
            shape: None,
            geom: None,
            brush: None,
        }
    }

    #[test]
    fn a_rub_across_the_middle_leaves_two_pieces() {
        // Sparse points: the cut must still fall at the eraser's edge.
        let s = line(0.0, 400.0, 100.0, 3);
        let parts = cut_stroke(&s, [200.0, 50.0], [200.0, 150.0], 20.0).unwrap();
        assert_eq!(parts.len(), 2);
        let left_end = parts[0].points.last().unwrap().x;
        let right_start = parts[1].points[0].x;
        assert!(left_end < 180.0 && left_end > 160.0, "{left_end}");
        assert!(right_start > 220.0 && right_start < 240.0, "{right_start}");
        assert_eq!(parts[0].points[0].x, 0.0);
        assert_eq!(parts[1].points.last().unwrap().x, 400.0);
        // Timing is kept (interpolated), so replay still flows.
        assert!(parts[1].points[0].dt > 0);
    }

    #[test]
    fn a_miss_changes_nothing_and_a_cover_erases_all() {
        let s = line(0.0, 100.0, 0.0, 10);
        assert!(cut_stroke(&s, [0.0, 200.0], [100.0, 200.0], 10.0).is_none());
        let all = cut_stroke(&s, [-10.0, 0.0], [110.0, 0.0], 10.0).unwrap();
        assert!(all.is_empty());
    }

    #[test]
    fn offsets_are_respected_and_shapes_become_ink() {
        let mut s = line(0.0, 100.0, 0.0, 20);
        s.offset = [1000.0, 0.0];
        s.shape = Some("line".into());
        assert!(cut_stroke(&s, [50.0, 0.0], [50.0, 0.0], 10.0).is_none());
        let parts = cut_stroke(&s, [1050.0, 0.0], [1050.0, 0.0], 10.0).unwrap();
        assert_eq!(parts.len(), 2);
        assert!(
            parts
                .iter()
                .all(|p| p.shape.is_none() && p.offset == [1000.0, 0.0])
        );
    }
}
