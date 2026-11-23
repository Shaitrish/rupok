//! Stroke grouping: which strokes make up one symbol.
//!
//! 1. **Scale**: `unit` is the median stroke size, ignoring dots.
//! 2. **Slant**: the writing slant is estimated from near-vertical ink and
//!    sheared out, so templates (upright) match italic writers.
//! 3. **Delayed strokes**: a stroke that lands on an earlier symbol but not
//!    on the stroke just before it (the dot of an `i` added later, a `t`
//!    crossed at the end, `−` turned into `+`) is moved next to that symbol.
//! 4. **Dynamic programming** over the (reordered) time sequence: a symbol
//!    is a run of 1–4 consecutive strokes that are close together and
//!    overlap horizontally; its cost is the classifier distance plus a
//!    per-symbol constant, so splitting `=` into two `−` loses to one `=`
//!    while gluing `1` and `+` into a nonsense symbol loses to two good
//!    ones. The k best segmentations are kept for the n-best list.

use std::collections::HashMap;

use super::classify::{Guess, Library};
use super::geom::{BBox, Pt, estimate_slant};
use super::symbols::id;

/// Most strokes in one symbol (`E`, `≠`, `÷` need 3–4).
pub const MAX_GROUP: usize = 4;
/// A stroke smaller than this × unit is a dot.
pub const DOT: f32 = 0.2;
/// Classifier distance → cost.
pub const TAU: f32 = 0.006;
/// Cost of each symbol (favours fewer, larger symbols).
pub const LAMBDA: f32 = 1.0;
/// Cost of a lone dot as a symbol (`.`, `·`), before [`LAMBDA`].
const DOT_COST: f32 = 0.4;
/// Cost per unit of the largest gap inside a group.
const GAP_COST: f32 = 1.0;
/// Guesses kept per group.
pub const TOP_K: usize = 6;

/// Strokes ready for grouping: deslanted, with boxes and scale.
pub struct Prepared {
    pub strokes: Vec<Vec<Pt>>,
    pub boxes: Vec<BBox>,
    pub unit: f32,
    /// Shear that was removed, quantised (cache key).
    pub slant_key: i32,
    /// Strokes in grouping order (time order with delayed strokes moved).
    pub seq: Vec<usize>,
}

impl Prepared {
    pub fn is_dot(&self, i: usize) -> bool {
        self.boxes[i].size() < DOT * self.unit
    }
}

fn median(v: &mut [f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f32::total_cmp);
    v[v.len() / 2]
}

/// Typical symbol size: the median stroke size, ignoring dots.
pub fn unit_of(boxes: &[BBox]) -> f32 {
    let mut sizes: Vec<f32> = boxes
        .iter()
        .map(|b| b.size())
        .filter(|s| *s > 0.0)
        .collect();
    let m = median(&mut sizes);
    let mut big: Vec<f32> = sizes.into_iter().filter(|s| *s > 0.35 * m).collect();
    median(&mut big).max(1e-3)
}

pub const SLANT_STEP: f32 = 0.05;

pub fn prepare(raw: &[Vec<Pt>]) -> Prepared {
    let boxes0: Vec<BBox> = raw.iter().map(|s| BBox::of(s)).collect();
    let unit0 = unit_of(&boxes0);
    let k = estimate_slant(raw, unit0);
    let slant_key = (k / SLANT_STEP).round() as i32;
    let k = slant_key as f32 * SLANT_STEP;
    let all = BBox::of_strokes(raw.iter().map(|s| s.as_slice()));
    let y_ref = if all.is_empty() { 0.0 } else { all.cy() };
    let strokes: Vec<Vec<Pt>> = if slant_key == 0 {
        raw.to_vec()
    } else {
        raw.iter()
            .map(|s| {
                s.iter()
                    .map(|p| Pt::new(p.x + k * (p.y - y_ref), p.y))
                    .collect()
            })
            .collect()
    };
    let boxes: Vec<BBox> = strokes.iter().map(|s| BBox::of(s)).collect();
    let unit = unit_of(&boxes);
    let mut prep = Prepared {
        strokes,
        boxes,
        unit,
        slant_key,
        seq: Vec::new(),
    };
    prep.seq = reorder(&prep);
    prep
}

/// Does stroke `s` belong with stroke `e` spatially (overlaps it, or is a
/// dot just above or below it)?
fn attaches(p: &Prepared, s: usize, e: usize) -> bool {
    let (bs, be) = (p.boxes[s], p.boxes[e]);
    let u = p.unit;
    if p.is_dot(s) {
        if p.is_dot(e) {
            return false;
        }
        let inx = bs.cx() >= be.x0 - 0.3 * u && bs.cx() <= be.x1 + 0.3 * u;
        let above = bs.cy() < be.y0 + 0.2 * be.h() && bs.cy() > be.y0 - 0.8 * u;
        let below = bs.cy() > be.y1 - 0.2 * be.h() && bs.cy() < be.y1 + 0.8 * u;
        return inx && (above || below);
    }
    if p.is_dot(e) {
        return false;
    }
    let r = bs.diag() / be.diag().max(1e-6);
    if !(0.4..=2.5).contains(&r) || bs.gap(be) > 0.0 {
        return false;
    }
    let inside = |a: BBox, b: BBox| {
        let m = (0.25 * b.size()).min(0.25 * u);
        a.cx() >= b.x0 - m && a.cx() <= b.x1 + m && a.cy() >= b.y0 - m && a.cy() <= b.y1 + m
    };
    inside(bs, be) || inside(be, bs)
}

/// Grouping order: time order, except that a stroke that attaches to an
/// earlier symbol but not to its predecessor moves right after that symbol.
fn reorder(p: &Prepared) -> Vec<usize> {
    let n = p.strokes.len();
    let mut seq: Vec<usize> = Vec::with_capacity(n);
    for s in 0..n {
        if p.strokes[s].is_empty() {
            continue;
        }
        let Some(&prev) = seq.last() else {
            seq.push(s);
            continue;
        };
        if attaches(p, s, prev) {
            seq.push(s);
            continue;
        }
        // Latest earlier stroke it attaches to.
        let target = seq.iter().rposition(|&e| attaches(p, s, e));
        match target {
            Some(pos) => {
                // After the target and anything already attached to it.
                let mut at = pos + 1;
                while at < seq.len() && attaches(p, seq[at], seq[pos]) {
                    at += 1;
                }
                seq.insert(at, s);
            }
            None => seq.push(s),
        }
    }
    seq
}

/// Could these strokes be one symbol? Cheap geometric checks only.
fn admissible(p: &Prepared, ids: &[usize]) -> Option<f32> {
    if ids.len() == 1 {
        return Some(0.0);
    }
    let u = p.unit;
    let dots = ids.iter().filter(|&&i| p.is_dot(i)).count();
    if dots == ids.len() {
        return None;
    }
    let all = ids
        .iter()
        .fold(BBox::EMPTY, |b, &i| b.union(p.boxes[i]));
    if ids.len() >= 3 && all.size() > 3.0 * u {
        return None;
    }
    let mut worst_gap = 0.0f32;
    for (k, &i) in ids.iter().enumerate() {
        let rest = ids
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != k)
            .fold(BBox::EMPTY, |b, (_, &j)| b.union(p.boxes[j]));
        let b = p.boxes[i];
        let gap = b.gap(rest);
        if gap > 0.6 * u {
            return None;
        }
        worst_gap = worst_gap.max(gap);
        if p.is_dot(i) {
            // A dot sits above or below the ink of the other strokes (i, j, ÷).
            let ink = ids
                .iter()
                .filter(|&&j| !p.is_dot(j))
                .fold(BBox::EMPTY, |a, &j| a.union(p.boxes[j]));
            let inx = b.cx() >= ink.x0 - 0.15 * u && b.cx() <= ink.x1 + 0.15 * u;
            let above = b.y1 < ink.y0 + 0.05 * u;
            let below = b.y0 > ink.y1 - 0.05 * u;
            if !(inx && (above || below)) {
                return None;
            }
            continue;
        }
        let ov = b.x_overlap(rest);
        let narrow = b.w().min(rest.w());
        let need = if narrow > 0.3 * u {
            0.2 * narrow
        } else {
            -0.1 * u
        };
        if ov < need {
            return None;
        }
    }
    Some(worst_gap / u)
}

/// One symbol of a segmentation.
#[derive(Clone, Debug)]
pub struct Group {
    /// Stroke ids, in grouping order.
    pub strokes: Vec<usize>,
    pub guesses: Vec<Guess>,
    pub cost: f32,
    pub dot: bool,
}

#[derive(Clone, Debug)]
pub struct Segmentation {
    pub groups: Vec<Group>,
    pub cost: f32,
}

/// Classification results by group, so live preview only classifies the
/// groups that contain a new stroke.
#[derive(Default)]
pub struct GroupCache {
    slant_key: i32,
    map: HashMap<Vec<u32>, Vec<Guess>>,
}

impl GroupCache {
    pub fn forget_stroke(&mut self, id: usize) {
        let id = id as u32;
        self.map.retain(|k, _| !k.contains(&id));
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
}

fn classify_group(
    p: &Prepared,
    lib: &Library,
    ids: &[usize],
    cache: &mut Option<&mut GroupCache>,
) -> Vec<Guess> {
    let mut key: Vec<u32> = ids.iter().map(|&i| i as u32).collect();
    key.sort_unstable();
    if let Some(c) = cache.as_deref_mut() {
        if c.slant_key != p.slant_key {
            c.map.clear();
            c.slant_key = p.slant_key;
        }
        if let Some(g) = c.map.get(&key) {
            return g.clone();
        }
    }
    let refs: Vec<&[Pt]> = ids.iter().map(|&i| p.strokes[i].as_slice()).collect();
    let g = lib.classify(&refs, TOP_K);
    if let Some(c) = cache.as_deref_mut() {
        c.map.insert(key, g.clone());
    }
    g
}

#[derive(Clone, Copy)]
struct Partial {
    cost: f32,
    /// (start position, rank in that position's beam, group index).
    back: Option<(usize, usize, usize)>,
}

/// The `beams` best segmentations, best first.
pub fn segment(
    p: &Prepared,
    lib: &Library,
    mut cache: Option<&mut GroupCache>,
    beams: usize,
) -> Vec<Segmentation> {
    let k = p.seq.len();
    if k == 0 {
        return Vec::new();
    }
    let beams = beams.max(1);
    let mut groups: Vec<Group> = Vec::new();
    let mut beam: Vec<Vec<Partial>> = vec![Vec::new(); k + 1];
    beam[0].push(Partial {
        cost: 0.0,
        back: None,
    });
    let dot_guesses = vec![
        Guess {
            sym: id("."),
            dist: 0.0,
        },
        Guess {
            sym: id("·"),
            dist: super::classify::TWIN_PENALTY,
        },
    ];
    for end in 1..=k {
        let mut cands: Vec<Partial> = Vec::new();
        for g in 1..=MAX_GROUP.min(end) {
            let start = end - g;
            if beam[start].is_empty() {
                continue;
            }
            let ids = &p.seq[start..end];
            let Some(gap) = admissible(p, ids) else {
                continue;
            };
            let (guesses, cost, dot) = if g == 1 && p.is_dot(ids[0]) {
                (dot_guesses.clone(), LAMBDA + DOT_COST, true)
            } else {
                let gs = classify_group(p, lib, ids, &mut cache);
                let Some(best) = gs.first() else {
                    continue;
                };
                let c = best.dist / TAU + LAMBDA + GAP_COST * gap;
                (gs, c, false)
            };
            let gi = groups.len();
            groups.push(Group {
                strokes: ids.to_vec(),
                guesses,
                cost,
                dot,
            });
            for (r, prev) in beam[start].iter().enumerate() {
                cands.push(Partial {
                    cost: prev.cost + cost,
                    back: Some((start, r, gi)),
                });
            }
        }
        cands.sort_by(|a, b| a.cost.total_cmp(&b.cost));
        cands.truncate(beams);
        beam[end] = cands;
    }
    let mut out = Vec::new();
    for fin in &beam[k] {
        let mut gs: Vec<Group> = Vec::new();
        let mut cur = *fin;
        while let Some((start, r, gi)) = cur.back {
            gs.push(groups[gi].clone());
            cur = beam[start][r];
        }
        gs.reverse();
        out.push(Segmentation {
            groups: gs,
            cost: fin.cost,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Pt> {
        (0..=10)
            .map(|i| {
                let t = i as f32 / 10.0;
                Pt::new(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
            })
            .collect()
    }

    #[test]
    fn delayed_cross_moves_next_to_its_stem() {
        // "t" stem, then "+" (two strokes) far right, then the t's cross.
        let strokes = vec![
            line(0.0, 0.0, 0.0, 30.0),
            line(40.0, 15.0, 60.0, 15.0),
            line(50.0, 5.0, 50.0, 25.0),
            line(-6.0, 10.0, 6.0, 10.0),
        ];
        let p = prepare(&strokes);
        assert_eq!(p.seq, vec![0, 3, 1, 2]);
    }

    #[test]
    fn unit_ignores_dots() {
        let b = |s: f32| BBox {
            x0: 0.0,
            y0: 0.0,
            x1: s,
            y1: s,
        };
        let u = unit_of(&[b(1.0), b(1.0), b(20.0), b(22.0), b(1.0)]);
        assert!(u >= 20.0);
    }
}
