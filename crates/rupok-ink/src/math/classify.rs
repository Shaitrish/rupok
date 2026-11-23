//! Symbol classification with the $Q point-cloud recognizer.
//!
//! $Q (Vatavu, Anthony and Wobbrock, "$Q: A Super-Quick, Articulation-
//! Invariant Stroke-Gesture Recognizer for Low-Resource Devices", MobileHCI
//! 2018) is the fast successor of $P ("Gestures as Point Clouds", ICMI 2012).
//! A gesture is resampled to [`N`] points regardless of how many strokes it
//! has or in which order and direction they were drawn, scaled uniformly and
//! centred on its centroid. Two clouds are compared by a greedy weighted
//! matching of their points; $Q adds a lookup-table lower bound and early
//! abandoning so most templates are rejected after a few operations.
//!
//! Differences from the paper, all deliberate:
//!
//! - **No rotation invariance** ($Q has none either): `6`/`9`, `u`/`n`,
//!   `<`/`>` differ only by rotation.
//! - **Short strokes keep points.** $P spreads points over the total path
//!   length, so the dot of an `i` gets none. Here each stroke gets points in
//!   proportion to `max(length, 8 % of the diagonal)`.
//! - The lookup table is 32 × 32 instead of 64 × 64 (1 KiB per template).
//! - Distances are normalised by the sum of weights, so a score is a
//!   weighted mean squared distance in units of the gesture's size.

use super::geom::{BBox, Pt, path_len, resample_into};
use super::symbols::{Cross, SYMBOLS, SymId, sym, twins};

/// Points per cloud (the $P/$Q default).
pub const N: usize = 32;
/// Lookup-table resolution per axis.
const M: usize = 32;
/// Weight sum of a full match: N + (N − 1) + … + 1.
const WEIGHTS: f32 = (N * (N + 1) / 2) as f32;
/// Smallest share of the diagonal a stroke is treated as long, so dots keep
/// points.
const MIN_STROKE_SHARE: f32 = 0.08;

/// A normalised point cloud with its $Q lookup table.
#[derive(Clone)]
pub struct Cloud {
    pts: [Pt; N],
    lut: Box<[u8; M * M]>,
}

impl Cloud {
    /// Normalise a set of strokes. `None` when there are no points.
    pub fn new(strokes: &[&[Pt]]) -> Option<Cloud> {
        let strokes: Vec<&[Pt]> = strokes.iter().copied().filter(|s| !s.is_empty()).collect();
        if strokes.is_empty() {
            return None;
        }
        let bbox = BBox::of_strokes(strokes.iter().copied());
        let floor = (MIN_STROKE_SHARE * bbox.diag()).max(1e-6);
        let lens: Vec<f32> = strokes.iter().map(|s| path_len(s).max(floor)).collect();
        let counts = allocate(&lens, N);
        let mut raw = Vec::with_capacity(N);
        for (s, &n) in strokes.iter().zip(&counts) {
            resample_into(s, n, &mut raw);
        }
        debug_assert_eq!(raw.len(), N);
        let b = BBox::of(&raw);
        let scale = b.size().max(1e-6);
        let (sx, sy) = raw.iter().fold((0.0, 0.0), |(x, y), p| (x + p.x, y + p.y));
        let c = Pt::new(sx / N as f32, sy / N as f32);
        let mut pts = [Pt::default(); N];
        for (o, p) in pts.iter_mut().zip(&raw) {
            *o = Pt::new((p.x - c.x) / scale, (p.y - c.y) / scale);
        }
        let mut lut = Box::new([0u8; M * M]);
        for gy in 0..M {
            for gx in 0..M {
                let q = Pt::new(from_grid(gx), from_grid(gy));
                let mut best = (f32::INFINITY, 0u8);
                for (k, p) in pts.iter().enumerate() {
                    let d = d2(q, *p);
                    if d < best.0 {
                        best = (d, k as u8);
                    }
                }
                lut[gy * M + gx] = best.1;
            }
        }
        Some(Cloud { pts, lut })
    }

    fn cell(p: Pt) -> usize {
        to_grid(p.y) * M + to_grid(p.x)
    }
}

fn to_grid(v: f32) -> usize {
    (((v + 1.0) * 0.5 * (M - 1) as f32).round()).clamp(0.0, (M - 1) as f32) as usize
}

fn from_grid(g: usize) -> f32 {
    g as f32 / (M - 1) as f32 * 2.0 - 1.0
}

#[inline]
fn d2(a: Pt, b: Pt) -> f32 {
    let (dx, dy) = (a.x - b.x, a.y - b.y);
    dx * dx + dy * dy
}

/// Split `n` points over strokes in proportion to `lens`, at least one each
/// (largest remainder).
fn allocate(lens: &[f32], n: usize) -> Vec<usize> {
    let k = lens.len();
    if k >= n {
        // More strokes than points: keep the longest strokes.
        let mut idx: Vec<usize> = (0..k).collect();
        idx.sort_by(|&a, &b| lens[b].total_cmp(&lens[a]));
        let mut out = vec![0; k];
        for &i in idx.iter().take(n) {
            out[i] = 1;
        }
        return out;
    }
    let total: f32 = lens.iter().sum();
    let spare = (n - k) as f32;
    let mut out: Vec<usize> = Vec::with_capacity(k);
    let mut rem: Vec<(f32, usize)> = Vec::with_capacity(k);
    for (i, l) in lens.iter().enumerate() {
        let share = spare * l / total;
        out.push(1 + share.floor() as usize);
        rem.push((share - share.floor(), i));
    }
    let mut left = n - out.iter().sum::<usize>();
    rem.sort_by(|a, b| b.0.total_cmp(&a.0));
    for &(_, i) in rem.iter().cycle() {
        if left == 0 {
            break;
        }
        out[i] += 1;
        left -= 1;
    }
    out
}

/// $Q lower bounds of the distance from `a` to `b` for each start index
/// `0, step, 2·step, …` (equation 5 of the paper).
fn lower_bounds(a: &Cloud, b: &Cloud, step: usize, out: &mut [f32]) {
    let mut sat = [0.0f32; N];
    let mut lb0 = 0.0;
    for i in 0..N {
        let j = b.lut[Cloud::cell(a.pts[i])] as usize;
        let d = d2(a.pts[i], b.pts[j]);
        sat[i] = if i == 0 { d } else { sat[i - 1] + d };
        lb0 += (N - i) as f32 * d;
    }
    out[0] = lb0;
    for (k, i) in (step..N).step_by(step).enumerate() {
        out[k + 1] = lb0 + i as f32 * sat[N - 1] - N as f32 * sat[i - 1];
    }
}

/// Greedy weighted matching of `a` onto `b` starting at `start`, abandoned
/// as soon as it exceeds `limit`.
fn cloud_distance(a: &Cloud, b: &Cloud, start: usize, limit: f32) -> f32 {
    let mut unmatched: [u8; N] = std::array::from_fn(|i| i as u8);
    let mut left = N;
    let mut sum = 0.0;
    let mut i = start;
    let mut weight = N as f32;
    loop {
        let p = a.pts[i];
        let (mut best, mut at) = (f32::INFINITY, 0);
        for (k, &j) in unmatched[..left].iter().enumerate() {
            let d = d2(p, b.pts[j as usize]);
            if d < best {
                best = d;
                at = k;
            }
        }
        left -= 1;
        unmatched[at] = unmatched[left];
        sum += weight * best;
        if sum >= limit {
            return sum;
        }
        weight -= 1.0;
        i = (i + 1) % N;
        if i == start {
            return sum;
        }
    }
}

const STEP: usize = 5; // floor(N^(1 − 0.5)) for N = 32
const STARTS: usize = N.div_ceil(STEP);

/// $Q distance between two clouds, or something ≥ `limit` when it is
/// certainly worse. Normalised by the weight sum.
pub fn distance(a: &Cloud, b: &Cloud, limit: f32) -> f32 {
    let mut best = limit * WEIGHTS;
    let mut lb1 = [0.0f32; STARTS];
    let mut lb2 = [0.0f32; STARTS];
    lower_bounds(a, b, STEP, &mut lb1);
    lower_bounds(b, a, STEP, &mut lb2);
    for (k, i) in (0..N).step_by(STEP).enumerate() {
        if lb1[k] < best {
            best = best.min(cloud_distance(a, b, i, best));
        }
        if lb2[k] < best {
            best = best.min(cloud_distance(b, a, i, best));
        }
    }
    best / WEIGHTS
}

/// One stored template.
#[derive(Clone)]
pub struct Template {
    pub sym: SymId,
    pub strokes: u8,
    pub user: bool,
    pub cloud: Cloud,
}

/// Distance multiplier for the user's own templates (their handwriting).
const USER_BONUS: f32 = 0.8;
/// Distance added when a group's strokes break the symbol's crossing rule.
const CROSS_PENALTY: f32 = 0.02;
/// Distance added to a twin (same shape, different symbol).
pub const TWIN_PENALTY: f32 = 0.0015;
/// Most user templates kept per symbol; older ones are dropped first.
pub const MAX_USER_PER_SYMBOL: usize = 16;

/// A ranked guess for one group of strokes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guess {
    pub sym: SymId,
    /// Normalised $Q distance (lower is better).
    pub dist: f32,
}

/// The template library and classifier.
#[derive(Clone)]
pub struct Library {
    templates: Vec<Template>,
    /// Stroke-count range per symbol, widened by user templates.
    ranges: Vec<(u8, u8)>,
}

impl Library {
    pub fn new(glyphs: &[super::templates::Glyph]) -> Library {
        let mut lib = Library {
            templates: Vec::with_capacity(glyphs.len()),
            ranges: SYMBOLS.iter().map(|s| s.strokes).collect(),
        };
        for g in glyphs {
            let refs: Vec<&[Pt]> = g.strokes.iter().map(|s| s.as_slice()).collect();
            if let Some(cloud) = Cloud::new(&refs) {
                lib.templates.push(Template {
                    sym: g.sym,
                    strokes: g.strokes.len().min(255) as u8,
                    user: false,
                    cloud,
                });
            }
        }
        lib
    }

    pub fn add_user(&mut self, sym: SymId, strokes: &[&[Pt]]) -> bool {
        let Some(cloud) = Cloud::new(strokes) else {
            return false;
        };
        let n = strokes.iter().filter(|s| !s.is_empty()).count().min(255) as u8;
        let mine = self
            .templates
            .iter()
            .filter(|t| t.user && t.sym == sym)
            .count();
        if mine >= MAX_USER_PER_SYMBOL
            && let Some(i) = self.templates.iter().position(|t| t.user && t.sym == sym)
        {
            self.templates.remove(i);
        }
        let r = &mut self.ranges[sym as usize];
        r.0 = r.0.min(n);
        r.1 = r.1.max(n);
        self.templates.push(Template {
            sym,
            strokes: n,
            user: true,
            cloud,
        });
        true
    }

    pub fn clear_user(&mut self, sym: Option<SymId>) {
        self.templates
            .retain(|t| !t.user || sym.is_some_and(|s| s != t.sym));
        self.ranges = SYMBOLS.iter().map(|s| s.strokes).collect();
        for t in &self.templates {
            let r = &mut self.ranges[t.sym as usize];
            r.0 = r.0.min(t.strokes);
            r.1 = r.1.max(t.strokes);
        }
    }

    pub fn user_count(&self) -> usize {
        self.templates.iter().filter(|t| t.user).count()
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }

    /// Approximate heap size of the templates in bytes.
    pub fn heap_bytes(&self) -> usize {
        self.templates.len() * (std::mem::size_of::<Template>() + M * M)
            + self.ranges.len() * 2
    }

    /// Best `k` symbols for a group of strokes, best first. Twins of the
    /// winners are added just behind them.
    pub fn classify(&self, strokes: &[&[Pt]], k: usize) -> Vec<Guess> {
        let Some(cloud) = Cloud::new(strokes) else {
            return Vec::new();
        };
        let n = strokes.iter().filter(|s| !s.is_empty()).count();
        let crossing = cross_state(strokes);
        let mut best = vec![f32::INFINITY; SYMBOLS.len()];
        // Top-k distinct symbols, kept sorted.
        let mut top: Vec<Guess> = Vec::with_capacity(k + 1);
        for t in &self.templates {
            let (lo, hi) = self.ranges[t.sym as usize];
            if n < lo as usize || n > hi as usize {
                continue;
            }
            let mut extra = 0.0;
            if n >= 2 {
                match (sym(t.sym).cross, crossing) {
                    (Cross::Needs, false) | (Cross::Forbids, true) => extra = CROSS_PENALTY,
                    _ => {}
                }
            }
            let scale = if t.user { USER_BONUS } else { 1.0 };
            let mut limit = best[t.sym as usize];
            if top.len() >= k {
                limit = limit.min(top[k - 1].dist);
            }
            if !limit.is_finite() {
                limit = 1e9;
            }
            // d * scale + extra < limit  ⇔  d < (limit − extra) / scale
            let raw_limit = (limit - extra) / scale;
            if raw_limit <= 0.0 {
                continue;
            }
            let d = distance(&cloud, &t.cloud, raw_limit) * scale + extra;
            if d < best[t.sym as usize] {
                best[t.sym as usize] = d;
                top.retain(|g| g.sym != t.sym);
                let at = top.partition_point(|g| g.dist <= d);
                top.insert(at, Guess { sym: t.sym, dist: d });
                top.truncate(k);
            }
        }
        // Twins share the winner's shape.
        let mut out: Vec<Guess> = Vec::with_capacity(top.len() * 2);
        for g in &top {
            out.push(*g);
        }
        for g in &top {
            for t in twins(g.sym) {
                let (lo, hi) = self.ranges[t as usize];
                if n < lo as usize || n > hi as usize {
                    continue;
                }
                let d = g.dist + TWIN_PENALTY;
                match out.iter_mut().find(|o| o.sym == t) {
                    Some(o) => o.dist = o.dist.min(d),
                    None => out.push(Guess { sym: t, dist: d }),
                }
            }
        }
        out.sort_by(|a, b| a.dist.total_cmp(&b.dist));
        out
    }
}

/// Whether any two strokes of the group cross.
fn cross_state(strokes: &[&[Pt]]) -> bool {
    for (i, a) in strokes.iter().enumerate() {
        for b in &strokes[i + 1..] {
            if super::geom::strokes_cross(a, b) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::super::symbols::id;
    use super::super::templates::builtin_glyphs;
    use super::*;

    #[test]
    fn allocation_sums_to_n() {
        assert_eq!(allocate(&[1.0], N), vec![N]);
        let a = allocate(&[10.0, 1.0, 1.0], N);
        assert_eq!(a.iter().sum::<usize>(), N);
        assert!(a.iter().all(|&c| c >= 1));
        let many = vec![1.0; 40];
        assert_eq!(allocate(&many, N).iter().sum::<usize>(), N);
    }

    #[test]
    fn identical_clouds_have_zero_distance() {
        let s = [Pt::new(0.0, 0.0), Pt::new(5.0, 9.0), Pt::new(9.0, 0.0)];
        let a = Cloud::new(&[&s]).unwrap();
        let b = Cloud::new(&[&s]).unwrap();
        assert!(distance(&a, &b, f32::INFINITY) < 1e-6);
        // Stroke direction does not matter.
        let rev: Vec<Pt> = s.iter().rev().copied().collect();
        let c = Cloud::new(&[&rev]).unwrap();
        assert!(distance(&a, &c, f32::INFINITY) < 1e-4);
    }

    #[test]
    fn early_abandon_never_undercuts() {
        let lib = Library::new(&builtin_glyphs());
        let a = &lib.templates[3].cloud;
        for t in &lib.templates {
            let full = distance(a, &t.cloud, f32::INFINITY);
            let cut = distance(a, &t.cloud, full * 0.5);
            assert!(cut >= full * 0.5 - 1e-6);
        }
    }

    #[test]
    fn classifies_its_own_glyphs() {
        let glyphs = builtin_glyphs();
        let lib = Library::new(&glyphs);
        let mut wrong = Vec::new();
        for g in &glyphs {
            let refs: Vec<&[Pt]> = g.strokes.iter().map(|s| s.as_slice()).collect();
            let top = lib.classify(&refs, 5);
            if top.first().map(|t| t.sym) != Some(g.sym)
                && !top.first().is_some_and(|t| t.dist < 1e-6)
            {
                wrong.push(sym(g.sym).name);
            }
        }
        assert!(wrong.is_empty(), "{wrong:?}");
        let plus = glyphs.iter().find(|g| g.sym == id("+")).unwrap();
        let refs: Vec<&[Pt]> = plus.strokes.iter().map(|s| s.as_slice()).collect();
        assert_eq!(lib.classify(&refs, 3)[0].sym, id("+"));
    }
}
