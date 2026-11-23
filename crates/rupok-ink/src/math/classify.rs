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
//! - **24 points and 4 starts** (step 6, ε ≈ 0.4) instead of 32 points and
//!   ε = 0.5: half the work for the same accuracy on the synthetic set
//!   (measured: 91.4 % vs 92.4 % expression-level, within noise).
//! - **Best-first search.** Templates are visited in order of their
//!   candidate-to-template bound and the loop stops once no template can
//!   enter the top k. The template-to-candidate bound uses a lookup table
//!   that the candidate fills lazily, only for the cells it is asked about.
//! - The lookup table is 32 × 32 instead of 64 × 64 (1 KiB per template).
//! - Distances are normalised by the sum of weights, so a score is a
//!   weighted mean squared distance in units of the gesture's size.

use super::geom::{BBox, Pt, path_len, resample_into};
use super::symbols::{Cross, SYMBOLS, SymId, sym, twins};

/// Points per cloud ($P/$Q use 32; 24 halves the matching cost).
pub const N: usize = 24;
/// Lookup-table resolution per axis.
const M: usize = 32;
/// Weight sum of a full match: N + (N − 1) + … + 1.
const WEIGHTS: f32 = (N * (N + 1) / 2) as f32;
/// Smallest share of the diagonal a stroke is treated as long, so dots keep
/// points.
const MIN_STROKE_SHARE: f32 = 0.08;

/// A normalised point cloud. Templates also carry their $Q lookup table;
/// a candidate fills one lazily, only for the cells it is asked about.
#[derive(Clone)]
pub struct Cloud {
    pts: [Pt; N],
    /// Lookup-table cell of each point.
    cells: [u16; N],
    /// Nearest point per cell (templates only).
    lut: Option<Box<[u8; M * M]>>,
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
        let cells = std::array::from_fn(|i| cell(pts[i]) as u16);
        Some(Cloud {
            pts,
            cells,
            lut: None,
        })
    }

    /// Precompute the full lookup table (for templates).
    fn with_lut(mut self) -> Cloud {
        let mut lut = Box::new([0u8; M * M]);
        for gy in 0..M {
            for gx in 0..M {
                lut[gy * M + gx] = nearest(&self.pts, Pt::new(from_grid(gx), from_grid(gy)));
            }
        }
        self.lut = Some(lut);
        self
    }
}

fn cell(p: Pt) -> usize {
    to_grid(p.y) * M + to_grid(p.x)
}

fn to_grid(v: f32) -> usize {
    // No `round()`: it is a libm call on baseline x86 and wasm.
    let g = (v + 1.0) * 0.5 * (M - 1) as f32 + 0.5;
    if g > 0.0 { (g as usize).min(M - 1) } else { 0 }
}

fn from_grid(g: usize) -> f32 {
    g as f32 / (M - 1) as f32 * 2.0 - 1.0
}

fn nearest(pts: &[Pt; N], q: Pt) -> u8 {
    let mut best = (f32::INFINITY, 0u8);
    for (k, p) in pts.iter().enumerate() {
        let d = d2(q, *p);
        if d < best.0 {
            best = (d, k as u8);
        }
    }
    best.1
}

/// A candidate's lookup table, filled on demand.
struct LazyLut {
    cells: Box<[u8; M * M]>,
}

impl LazyLut {
    const UNKNOWN: u8 = u8::MAX;

    fn new() -> LazyLut {
        LazyLut {
            cells: Box::new([Self::UNKNOWN; M * M]),
        }
    }

    fn get(&mut self, pts: &[Pt; N], c: usize) -> usize {
        let v = self.cells[c];
        if v != Self::UNKNOWN {
            return v as usize;
        }
        let (gx, gy) = (c % M, c / M);
        let n = nearest(pts, Pt::new(from_grid(gx), from_grid(gy)));
        self.cells[c] = n;
        n as usize
    }
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
/// `0, step, 2·step, …` (equation 5 of the paper). `nn(cell)` is the point
/// of `b` nearest to a lookup-table cell.
fn lower_bounds(a: &Cloud, b: &Cloud, mut nn: impl FnMut(usize) -> usize) -> [f32; STARTS] {
    let mut sat = [0.0f32; N];
    let mut lb0 = 0.0;
    let mut acc = 0.0;
    for i in 0..N {
        let j = nn(a.cells[i] as usize);
        let d = d2(a.pts[i], b.pts[j]);
        acc += d;
        sat[i] = acc;
        lb0 += (N - i) as f32 * d;
    }
    let mut out = [0.0f32; STARTS];
    out[0] = lb0;
    for (k, i) in (STEP..N).step_by(STEP).enumerate() {
        out[k + 1] = lb0 + i as f32 * sat[N - 1] - N as f32 * sat[i - 1];
    }
    out
}

/// Bounds from the candidate onto a template (uses the template's table).
fn forward_bounds(cand: &Cloud, tmpl: &Cloud) -> [f32; STARTS] {
    match &tmpl.lut {
        Some(lut) => lower_bounds(cand, tmpl, |c| lut[c] as usize),
        None => [0.0; STARTS],
    }
}

fn min_of(v: &[f32; STARTS]) -> f32 {
    v.iter().fold(f32::INFINITY, |m, &x| m.min(x)) / WEIGHTS
}

/// Greedy weighted matching of `a` onto `b` starting at `start`, abandoned
/// as soon as it exceeds `limit`.
fn cloud_distance(a: &Cloud, b: &Cloud, start: usize, limit: f32) -> f32 {
    // Unmatched points of `b` as two dense arrays (swap-remove on match):
    // a tight, branch-light inner loop.
    let mut xs: [f32; N] = std::array::from_fn(|j| b.pts[j].x);
    let mut ys: [f32; N] = std::array::from_fn(|j| b.pts[j].y);
    let mut left = N;
    let mut sum = 0.0;
    let mut i = start;
    let mut weight = N as f32;
    loop {
        let p = a.pts[i];
        let (mut best, mut at) = (f32::INFINITY, 0);
        for (k, (x, y)) in xs[..left].iter().zip(&ys[..left]).enumerate() {
            let (dx, dy) = (x - p.x, y - p.y);
            let d = dx * dx + dy * dy;
            if d < best {
                best = d;
                at = k;
            }
        }
        left -= 1;
        xs[at] = xs[left];
        ys[at] = ys[left];
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

/// $Q distance between a candidate and a template, or something ≥ `limit`
/// when it is certainly worse. Normalised by the weight sum.
pub fn distance(cand: &Cloud, tmpl: &Cloud, limit: f32) -> f32 {
    let lb1 = forward_bounds(cand, tmpl);
    let mut lazy = LazyLut::new();
    distance_with(cand, tmpl, &lb1, &mut lazy, limit)
}

fn distance_with(
    cand: &Cloud,
    tmpl: &Cloud,
    lb1: &[f32; STARTS],
    lazy: &mut LazyLut,
    limit: f32,
) -> f32 {
    let lb2 = lower_bounds(tmpl, cand, |c| lazy.get(&cand.pts, c));
    let mut best = limit * WEIGHTS;
    for (k, i) in (0..N).step_by(STEP).enumerate() {
        if lb1[k] < best {
            best = best.min(cloud_distance(cand, tmpl, i, best));
        }
        if lb2[k] < best {
            best = best.min(cloud_distance(tmpl, cand, i, best));
        }
    }
    best / WEIGHTS
}

const STEP: usize = 6; // about N^(1 − ε) with ε = 0.4
const STARTS: usize = N.div_ceil(STEP);

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
/// Distance added per stroke beyond the symbol's usual count.
const STROKE_PENALTY: f32 = 0.009;
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
    /// Usual stroke count per symbol, raised by user templates.
    usual: Vec<u8>,
}

impl Library {
    pub fn new(glyphs: &[super::templates::Glyph]) -> Library {
        let mut lib = Library {
            templates: Vec::with_capacity(glyphs.len()),
            ranges: SYMBOLS.iter().map(|s| (s.strokes.0, s.strokes.2)).collect(),
            usual: SYMBOLS.iter().map(|s| s.strokes.1).collect(),
        };
        for g in glyphs {
            let refs: Vec<&[Pt]> = g.strokes.iter().map(|s| s.as_slice()).collect();
            if let Some(cloud) = Cloud::new(&refs).map(Cloud::with_lut) {
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
        let Some(cloud) = Cloud::new(strokes).map(Cloud::with_lut) else {
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
        self.usual[sym as usize] = self.usual[sym as usize].max(n);
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
        self.ranges = SYMBOLS.iter().map(|s| (s.strokes.0, s.strokes.2)).collect();
        self.usual = SYMBOLS.iter().map(|s| s.strokes.1).collect();
        for t in self.templates.iter().filter(|t| t.user) {
            let r = &mut self.ranges[t.sym as usize];
            r.0 = r.0.min(t.strokes);
            r.1 = r.1.max(t.strokes);
            self.usual[t.sym as usize] = self.usual[t.sym as usize].max(t.strokes);
        }
    }

    pub fn user_count(&self) -> usize {
        self.templates.iter().filter(|t| t.user).count()
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// Approximate heap size of the templates in bytes.
    pub fn heap_bytes(&self) -> usize {
        self.templates.len() * (std::mem::size_of::<Template>() + M * M) + self.ranges.len() * 2
    }

    /// Best `k` symbols for a group of strokes, best first. Twins of the
    /// winners are added just behind them.
    pub fn classify(&self, strokes: &[&[Pt]], k: usize) -> Vec<Guess> {
        let Some(cloud) = Cloud::new(strokes) else {
            return Vec::new();
        };
        let n = strokes.iter().filter(|s| !s.is_empty()).count();
        let crossing = cross_state(strokes);
        // A two-stroke θ is an oval and a bar: one stroke must be flat
        // (otherwise it is likely "0°" or "o" plus a mark).
        let has_flat = strokes.iter().any(|s| {
            let b = BBox::of(s);
            b.h() <= 0.35 * b.w()
        });
        let theta = super::symbols::id("θ");
        let mut best = vec![f32::INFINITY; SYMBOLS.len()];
        // Top-k distinct symbols, kept sorted.
        let mut top: Vec<Guess> = Vec::with_capacity(k + 1);
        // Best-first: templates in order of their $Q lower bound, so tight
        // limits come early and the loop stops once no template can enter
        // the top k.
        let mut order: Vec<(f32, usize, f32, f32, [f32; STARTS])> =
            Vec::with_capacity(self.templates.len());
        let mut lazy = LazyLut::new();
        for (ti, t) in self.templates.iter().enumerate() {
            let (lo, hi) = self.ranges[t.sym as usize];
            if n < lo as usize || n > hi as usize {
                continue;
            }
            let usual = self.usual[t.sym as usize] as usize;
            let mut extra = STROKE_PENALTY * n.saturating_sub(usual) as f32;
            if t.sym == theta && n == 2 && !has_flat {
                extra += CROSS_PENALTY;
            }
            if n >= 2 {
                match (sym(t.sym).cross, crossing) {
                    (Cross::Needs, false) | (Cross::Forbids, true) => extra += CROSS_PENALTY,
                    _ => {}
                }
            }
            let scale = if t.user { USER_BONUS } else { 1.0 };
            let lb1 = forward_bounds(&cloud, &t.cloud);
            order.push((min_of(&lb1) * scale + extra, ti, extra, scale, lb1));
        }
        order.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (lb, ti, extra, scale, lb1) in &order {
            let (lb, extra, scale) = (*lb, *extra, *scale);
            let t = &self.templates[*ti];
            if top.len() >= k && lb >= top[k - 1].dist {
                break;
            }
            let mut limit = best[t.sym as usize];
            if top.len() >= k {
                limit = limit.min(top[k - 1].dist);
            }
            if lb >= limit {
                continue;
            }
            if !limit.is_finite() {
                limit = 1e9;
            }
            // d * scale + extra < limit  ⇔  d < (limit − extra) / scale
            let raw_limit = (limit - extra) / scale;
            if raw_limit <= 0.0 {
                continue;
            }
            let d = distance_with(&cloud, &t.cloud, lb1, &mut lazy, raw_limit) * scale + extra;
            if d < best[t.sym as usize] {
                best[t.sym as usize] = d;
                top.retain(|g| g.sym != t.sym);
                let at = top.partition_point(|g| g.dist <= d);
                top.insert(
                    at,
                    Guess {
                        sym: t.sym,
                        dist: d,
                    },
                );
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
                let excess = n
                    .saturating_sub(self.usual[t as usize] as usize)
                    .saturating_sub(n.saturating_sub(self.usual[g.sym as usize] as usize));
                let d = g.dist + TWIN_PENALTY + STROKE_PENALTY * excess as f32;
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
/// Strokes that miss each other by less than this share of the group's size
/// still count as crossing (a hurried `+` whose bar stops short).
const CROSS_SLACK: f32 = 0.06;

/// Whether any two strokes of the group cross (or nearly meet).
fn cross_state(strokes: &[&[Pt]]) -> bool {
    let tol = CROSS_SLACK * BBox::of_strokes(strokes.iter().copied()).size();
    for (i, a) in strokes.iter().enumerate() {
        for b in &strokes[i + 1..] {
            if super::geom::strokes_cross(a, b) || super::geom::strokes_near(a, b, tol) {
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
