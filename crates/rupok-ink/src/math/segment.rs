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
/// Cost of a small, non-flat stroke sitting outside the rest of its group
/// (usually a script or a neighbour, not part of the symbol).
const SATELLITE_COST: f32 = 2.0;
/// Guesses kept per group.
pub const TOP_K: usize = 3;

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
    let sizes: Vec<f32> = boxes
        .iter()
        .map(|b| b.size())
        .filter(|s| *s > 0.0)
        .collect();
    let max = sizes.iter().copied().fold(0.0f32, f32::max);
    let mut big: Vec<f32> = sizes.iter().copied().filter(|s| *s >= 0.15 * max).collect();
    let m = median(&mut big);
    let mut typical: Vec<f32> = sizes.into_iter().filter(|s| *s >= 0.35 * m).collect();
    median(&mut typical).max(1e-3)
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
    // Not into a container (a radical or a big bracket around it).
    let contains = be.x0 <= bs.x0 && be.x1 >= bs.x1 && be.y0 <= bs.y0 && be.y1 >= bs.y1;
    if contains && (be.w() > 2.0 * bs.w() || be.h() > 2.0 * bs.h()) {
        return false;
    }
    let r = bs.diag() / be.diag().max(1e-6);
    if !(0.2..=5.0).contains(&r) {
        return false;
    }
    // The second line of "=" (flat, stacked, overlapping in x).
    let flat = |b: BBox| b.h() <= 0.3 * b.w();
    if flat(bs) && flat(be) && bs.x_overlap(be) > 0.5 * bs.w().min(be.w()) && bs.gap(be) < 0.5 * u {
        return true;
    }
    if bs.gap(be) > 0.0 {
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
pub(super) fn admissible(p: &Prepared, ids: &[usize]) -> Option<f32> {
    if ids.len() == 1 {
        return Some(0.0);
    }
    let u = p.unit;
    let dots = ids.iter().filter(|&&i| p.is_dot(i)).count();
    if dots == ids.len() {
        return None;
    }
    let all = ids.iter().fold(BBox::EMPTY, |b, &i| b.union(p.boxes[i]));
    if ids.len() >= 3 && all.size() > 3.0 * u {
        return None;
    }
    let mut worst_gap = 0.0f32;
    let mut satellites = 0;
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
        if !p.is_dot(i)
            && b.h() > 0.25 * b.w()
            && rest.h() > 0.3 * rest.w()
            && b.size() < 1.2 * rest.size()
        {
            let m = 0.1 * u;
            let inside = b.cx() >= rest.x0 - m
                && b.cx() <= rest.x1 + m
                && b.cy() >= rest.y0 - m
                && b.cy() <= rest.y1 + m;
            // Small marks (a degree circle, a prime) stay separate even when
            // they touch; bigger strokes only when they don't.
            if !inside && (b.size() < 0.45 * rest.size() || !touches(p, i, ids, 0.1 * u)) {
                satellites += 1;
            } else if inside && b.size() < 0.4 * rest.size() && is_loop(&p.strokes[i]) {
                // A small closed loop near the edge of a bigger symbol is a
                // degree sign or a little o, unless it sits well inside.
                let (mx, my) = (0.2 * rest.w(), 0.2 * rest.h());
                let deep = b.cx() > rest.x0 + mx
                    && b.cx() < rest.x1 - mx
                    && b.cy() > rest.y0 + my
                    && b.cy() < rest.y1 - my;
                if !deep {
                    satellites += 1;
                }
            }
        }
        // A flat line floating above the rest (a fraction bar over its
        // numerator) is not part of that symbol; "=" has both lines flat.
        if !p.is_dot(i)
            && b.h() <= 0.25 * b.w()
            && rest.h() > 0.3 * rest.w()
            && b.y1 < rest.y0
            && !touches(p, i, ids, 0.1 * u)
        {
            satellites += 1;
        }
        if p.is_dot(i) {
            // A dot sits above or below the ink of the other strokes (i, j, ÷).
            let ink = ids
                .iter()
                .filter(|&&j| !p.is_dot(j))
                .fold(BBox::EMPTY, |a, &j| a.union(p.boxes[j]));
            let inx = b.cx() >= ink.x0 - 0.15 * u && b.cx() <= ink.x1 + 0.15 * u;
            let above = b.y1 < ink.y0 + 0.05 * u || b.cy() < ink.y0 + 0.1 * ink.h();
            let below = b.y0 > ink.y1 - 0.05 * u;
            if !(inx && (above || below)) {
                return None;
            }
            continue;
        }
        // Overlap is judged against the other ink, not against dots (the
        // dot of an i is placed by the rule above).
        let rest = ids
            .iter()
            .enumerate()
            .filter(|(j, s)| *j != k && !p.is_dot(**s))
            .fold(BBox::EMPTY, |b, (_, &j)| b.union(p.boxes[j]));
        if rest.is_empty() {
            continue;
        }
        let ov = b.x_overlap(rest);
        let narrow = b.w().min(rest.w());
        let need = if narrow > 0.3 * u {
            0.1 * narrow
        } else {
            -0.1 * u
        };
        // Side by side is fine when the ink actually touches (the arches
        // of an m, the arcs of a ")(" x).
        if ov < need && !(ov > -0.15 * u && touches(p, i, ids, 0.1 * u)) {
            return None;
        }
    }
    Some(GAP_COST * worst_gap / u + SATELLITE_COST * satellites as f32)
}

/// A stroke whose ends meet: a small circle.
fn is_loop(s: &[Pt]) -> bool {
    let b = BBox::of(s);
    s.len() >= 4
        && s[0].dist(s[s.len() - 1]) < 0.35 * b.size()
        && b.h() > 0.5 * b.w()
        && b.w() > 0.5 * b.h()
}

/// Whether stroke `i` comes within `tol` of another stroke of the group.
fn touches(p: &Prepared, i: usize, ids: &[usize], tol: f32) -> bool {
    let coarse = |s: &[Pt]| {
        let mut v = Vec::with_capacity(24);
        super::geom::resample_into(s, s.len().clamp(1, 24), &mut v);
        v
    };
    let a = coarse(&p.strokes[i]);
    ids.iter().filter(|&&j| j != i).any(|&j| {
        let b = coarse(&p.strokes[j]);
        a.iter().any(|pa| b.iter().any(|pb| pa.dist(*pb) <= tol))
    })
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
#[derive(Clone, Debug, Default)]
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

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
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

/// A stroke that is clearly a radical on its own never groups with a
/// non-flat stroke under its bar (its body); a separate flat bar is fine.
fn swallows_body(
    p: &Prepared,
    lib: &Library,
    ids: &[usize],
    cache: &mut Option<&mut GroupCache>,
) -> bool {
    let root = id("√");
    for &r in ids {
        let rb = p.boxes[r];
        if p.is_dot(r) || rb.w() < 0.8 * rb.h() {
            continue;
        }
        let g = classify_group(p, lib, &[r], cache);
        if g.first().is_none_or(|x| x.sym != root || x.dist > 0.02) {
            continue;
        }
        let left = rb.x0 + 0.25 * rb.h().min(rb.w());
        for &o in ids {
            let b = p.boxes[o];
            if o != r
                && !p.is_dot(o)
                && b.h() > 0.3 * b.w()
                && b.cx() > left
                && b.cx() < rb.x1
                && b.cy() > rb.y0
            {
                return true;
            }
        }
    }
    false
}

/// A straight stroke: (length, direction in degrees from horizontal, 0–180)
/// when it deviates from its chord by less than 12 % of the chord.
fn straight(s: &[Pt]) -> Option<(f32, f32)> {
    let (a, b) = (*s.first()?, *s.last()?);
    let len = a.dist(b);
    if len <= 0.0 {
        return None;
    }
    let (dx, dy) = ((b.x - a.x) / len, (b.y - a.y) / len);
    let dev = s
        .iter()
        .map(|p| ((p.x - a.x) * dy - (p.y - a.y) * dx).abs())
        .fold(0.0f32, f32::max);
    if dev > 0.12 * len {
        return None;
    }
    let mut ang = dy.atan2(dx).to_degrees();
    if ang < 0.0 {
        ang += 180.0;
    }
    Some((len, ang % 180.0))
}

/// The two commonest operators by construction, independent of how far
/// distortion pulls them from their templates: `+` (a horizontal and a
/// vertical line crossing near both middles) and `=` (two stacked
/// horizontal lines of similar length).
fn structural(p: &Prepared, ids: &[usize]) -> Option<Guess> {
    let [a, b] = ids else {
        return None;
    };
    let (sa, sb) = (&p.strokes[*a], &p.strokes[*b]);
    let ((la, aa), (lb, ab)) = (straight(sa)?, straight(sb)?);
    let horiz = |ang: f32| !(25.0..=155.0).contains(&ang);
    let vert = |ang: f32| (65.0..=115.0).contains(&ang);
    let ratio = la.max(lb) / la.min(lb).max(1e-6);
    let (ba, bb) = (p.boxes[*a], p.boxes[*b]);
    if ((horiz(aa) && vert(ab)) || (vert(aa) && horiz(ab))) && ratio < 2.0 {
        let (h, v) = if horiz(aa) { (ba, bb) } else { (bb, ba) };
        let in_mid = |x: f32, lo: f32, hi: f32| {
            let t = (x - lo) / (hi - lo).max(1e-6);
            (0.2..=0.8).contains(&t)
        };
        let crosses = in_mid(v.cx(), h.x0, h.x1) && in_mid(h.cy(), v.y0, v.y1);
        if crosses {
            return Some(Guess {
                sym: id("+"),
                dist: 0.003,
            });
        }
    }
    let flat = |ang: f32| !(15.0..=165.0).contains(&ang);
    if flat(aa) && flat(ab) && ratio < 1.7 {
        let shorter = ba.w().min(bb.w());
        let gap = (ba.cy() - bb.cy()).abs();
        let mean = 0.5 * (la + lb);
        if ba.x_overlap(bb) > 0.6 * shorter && gap > 0.12 * mean && gap < 0.8 * mean {
            return Some(Guess {
                sym: id("="),
                dist: 0.002,
            });
        }
    }
    None
}

/// A radical drawn as a tick plus a separate overbar: the tick alone reads
/// as a radical and a flat line starts at its top and runs right. Returns
/// the tick's distance.
fn radical_with_bar(
    p: &Prepared,
    lib: &Library,
    ids: &[usize],
    cache: &mut Option<&mut GroupCache>,
) -> Option<f32> {
    let [a, b] = ids else {
        return None;
    };
    let flat = |i: usize| {
        let bb = p.boxes[i];
        bb.h() <= 0.2 * bb.w()
    };
    let (tick, bar) = match (flat(*a), flat(*b)) {
        (false, true) => (*a, *b),
        (true, false) => (*b, *a),
        _ => return None,
    };
    let (tb, bb) = (p.boxes[tick], p.boxes[bar]);
    let at_top = (bb.cy() - tb.y0).abs() < 0.25 * tb.h().max(0.3 * p.unit);
    let starts = bb.x0 > tb.x0 + 0.3 * tb.w() && bb.x0 < tb.x1 + 0.3 * p.unit;
    if !(at_top && starts && bb.x1 > tb.x1) {
        return None;
    }
    let g = classify_group(p, lib, &[tick], cache);
    g.first()
        .filter(|x| x.sym == id("√") && x.dist < 0.015)
        .map(|x| x.dist)
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
    cache: Option<&mut GroupCache>,
    beams: usize,
) -> Vec<Segmentation> {
    // Without a caller's cache, a local one still saves re-classifying
    // single strokes.
    let mut local = GroupCache::default();
    let mut cache = Some(cache.unwrap_or(&mut local));
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
            if g > 1 && swallows_body(p, lib, ids, &mut cache) {
                continue;
            }
            let (guesses, cost, dot) = if g == 1 && p.is_dot(ids[0]) {
                (dot_guesses.clone(), LAMBDA + DOT_COST, true)
            } else if let Some(d) = radical_with_bar(p, lib, ids, &mut cache) {
                let gs = vec![Guess {
                    sym: id("√"),
                    dist: d,
                }];
                (gs, d / TAU + LAMBDA + gap, false)
            } else {
                let mut gs = classify_group(p, lib, ids, &mut cache);
                if let Some(s) = structural(p, ids) {
                    match gs.iter_mut().find(|x| x.sym == s.sym) {
                        Some(x) => x.dist = x.dist.min(s.dist),
                        None => gs.push(s),
                    }
                    gs.sort_by(|a, b| a.dist.total_cmp(&b.dist));
                }
                let Some(best) = gs.first() else {
                    continue;
                };
                let c = best.dist / TAU + LAMBDA + gap;
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
