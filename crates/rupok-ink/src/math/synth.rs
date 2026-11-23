//! Synthetic handwriting for tests and benchmarks: render a LaTeX string as
//! perturbed ink with ground truth, and score a recognizer on a set.
//!
//! Glyphs come from the built-in templates (see [`super::templates`]), so
//! the set measures robustness to the perturbations, not to unseen writing
//! styles: per-glyph size, rotation and smooth elastic distortion, writing
//! slant, point noise, pen sampling, spacing and baseline wobble, stroke
//! order and direction shuffles, strokes joined without lifting the pen,
//! and out-of-order strokes (a late `i` dot or `t` cross, an exponent added
//! at the end). No handwriting dataset is used.

use std::collections::HashMap;

use super::geom::{BBox, Pt, resample_into};
use super::symbols::{Cat, SymId, id, sym};
use super::templates::{Glyph, builtin_glyphs};
use super::tree::{Atom, Base, Row, display_latex, parse_latex};
use super::{Candidate, MathRecognizer};
use crate::StrokePoint;

/// Small deterministic PRNG (xorshift64*), so tests are reproducible.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng((seed ^ 0x9E37_79B9_7F4A_7C15).wrapping_mul(0xD1B5_4A32_D192_ED03) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    /// Standard normal (Box–Muller).
    pub fn normal(&mut self) -> f32 {
        let u = self.f32().max(1e-7);
        let v = self.f32();
        (-2.0 * u.ln()).sqrt() * (std::f32::consts::TAU * v).cos()
    }
}

/// How much to perturb the ink.
#[derive(Clone, Debug, PartialEq)]
pub struct SynthStyle {
    /// Largest writing slant, degrees (either way).
    pub slant_deg: f32,
    /// Per-glyph relative size jitter.
    pub scale_jitter: f32,
    /// Per-glyph rotation, degrees (either way).
    pub rotate_deg: f32,
    /// Smooth elastic distortion, as a fraction of the glyph size.
    pub elastic: f32,
    /// Point noise σ as a fraction of the x-height.
    pub noise: f32,
    /// Gap between symbols, in x-heights (min, max).
    pub spacing: (f32, f32),
    /// Per-glyph vertical wobble, in x-heights.
    pub baseline_jitter: f32,
    /// Chance that consecutive strokes of a glyph are drawn without lifting.
    pub join: f32,
    /// Chance of one out-of-order stroke or script per expression.
    pub delayed: f32,
    /// On-screen x-height range, px.
    pub xh_px: (f32, f32),
}

impl SynthStyle {
    /// No perturbation at all.
    pub fn clean() -> Self {
        SynthStyle {
            slant_deg: 0.0,
            scale_jitter: 0.0,
            rotate_deg: 0.0,
            elastic: 0.0,
            noise: 0.0,
            spacing: (0.35, 0.35),
            baseline_jitter: 0.0,
            join: 0.0,
            delayed: 0.0,
            xh_px: (40.0, 40.0),
        }
    }

    /// The perturbation of the reported test set.
    pub fn standard() -> Self {
        SynthStyle {
            slant_deg: 15.0,
            scale_jitter: 0.1,
            rotate_deg: 3.0,
            elastic: 0.04,
            noise: 0.015,
            spacing: (0.15, 0.6),
            baseline_jitter: 0.06,
            join: 0.25,
            delayed: 0.15,
            xh_px: (18.0, 70.0),
        }
    }

    /// Messier writing.
    pub fn hard() -> Self {
        SynthStyle {
            slant_deg: 15.0,
            scale_jitter: 0.15,
            rotate_deg: 6.0,
            elastic: 0.08,
            noise: 0.025,
            spacing: (0.06, 0.7),
            baseline_jitter: 0.12,
            join: 0.4,
            delayed: 0.3,
            xh_px: (14.0, 70.0),
        }
    }
}

/// Rendered ink and its ground truth.
#[derive(Clone, Debug)]
pub struct SynthSample {
    /// Canonical LaTeX of what was rendered.
    pub latex: String,
    /// Strokes in writing order.
    pub strokes: Vec<Vec<StrokePoint>>,
    /// Every symbol with its stroke ids (ascending). Fraction bars are `─`.
    pub symbols: Vec<(&'static str, Vec<usize>)>,
}

struct Ink {
    name: &'static str,
    strokes: Vec<Vec<Pt>>,
    /// Superscript group, for "exponent written last".
    script: Option<usize>,
}

/// Renders LaTeX as synthetic handwriting.
pub struct Synth {
    by_sym: HashMap<SymId, Vec<Glyph>>,
}

impl Default for Synth {
    fn default() -> Self {
        Self::new()
    }
}

/// Font units per x-height (Hershey: −5 to 9).
const XH: f32 = 14.0;

impl Synth {
    pub fn new() -> Self {
        let mut by_sym: HashMap<SymId, Vec<Glyph>> = HashMap::new();
        for g in builtin_glyphs() {
            by_sym.entry(g.sym).or_default().push(g);
        }
        Synth { by_sym }
    }

    /// Render `latex` with perturbation `style`; `seed` picks the variation.
    pub fn render(
        &self,
        latex: &str,
        seed: u64,
        style: &SynthStyle,
    ) -> Result<SynthSample, String> {
        let row = parse_latex(latex)?;
        let mut r = Renderer {
            synth: self,
            rng: Rng::new(seed),
            st: style,
            inks: Vec::new(),
            scripts: 0,
        };
        r.row(&row, 0.0, 0.0, 1.0, None);
        r.finish(display_latex(&row))
    }
}

struct Renderer<'a> {
    synth: &'a Synth,
    rng: Rng,
    st: &'a SynthStyle,
    inks: Vec<Ink>,
    scripts: usize,
}

fn ink_box(inks: &[Ink]) -> BBox {
    inks.iter()
        .flat_map(|i| i.strokes.iter())
        .fold(BBox::EMPTY, |b, s| b.union(BBox::of(s)))
}

fn shift(inks: &mut [Ink], dx: f32, dy: f32) {
    for i in inks {
        for s in &mut i.strokes {
            for p in s {
                p.x += dx;
                p.y += dy;
            }
        }
    }
}

fn is_spaced(b: &Base) -> bool {
    matches!(b, Base::Sym(s) if matches!(sym(*s).cat, Cat::Op | Cat::Rel))
}

impl Renderer<'_> {
    /// Lay out a row starting at `x` on baseline `y`; returns the end x.
    fn row(&mut self, row: &Row, x: f32, y: f32, s: f32, script: Option<usize>) -> f32 {
        let mut cx = x;
        for (k, a) in row.iter().enumerate() {
            if k > 0 {
                let (lo, hi) = self.st.spacing;
                let mut gap = self.rng.range(lo, hi) * XH * s;
                if is_spaced(&a.base) || is_spaced(&row[k - 1].base) {
                    gap += self.rng.range(0.1, 0.4) * XH * s;
                }
                cx += gap;
            }
            cx = self.atom(a, cx, y, s, script);
        }
        cx
    }

    fn atom(&mut self, a: &Atom, x: f32, y: f32, s: f32, script: Option<usize>) -> f32 {
        let bb = self.base(&a.base, x, y, s, script);
        let mut end = bb.x1;
        let sx = bb.x1 + self.rng.range(0.5, 2.5) * s;
        if !a.sub.is_empty() {
            let ss = s * self.rng.range(0.55, 0.75);
            let i0 = self.inks.len();
            self.row(&a.sub, 0.0, 0.0, ss, script);
            let b = ink_box(&self.inks[i0..]);
            if !b.is_empty() {
                let top = bb.y0 + self.rng.range(0.45, 0.65) * bb.h();
                shift(&mut self.inks[i0..], sx - b.x0, top - b.y0);
                end = end.max(sx + b.w());
            }
        }
        if !a.sup.is_empty() {
            let ss = s * self.rng.range(0.55, 0.75);
            let group = self.scripts;
            self.scripts += 1;
            let i0 = self.inks.len();
            self.row(&a.sup, 0.0, 0.0, ss, Some(script.unwrap_or(group)));
            let b = ink_box(&self.inks[i0..]);
            if !b.is_empty() {
                let bottom = bb.y0 + self.rng.range(0.3, 0.5) * bb.h();
                shift(&mut self.inks[i0..], sx - b.x0, bottom - b.y1);
                end = end.max(sx + b.w());
            }
        }
        end
    }

    fn base(&mut self, b: &Base, x: f32, y: f32, s: f32, script: Option<usize>) -> BBox {
        match b {
            Base::Sym(sid) => self.glyph(*sid, x, y, s, script),
            Base::Func(f) => {
                let mut cx = x;
                let mut bb = BBox::EMPTY;
                for (k, ch) in f.chars().enumerate() {
                    if k > 0 {
                        cx += self.rng.range(0.08, 0.25) * XH * s;
                    }
                    let mut buf = [0u8; 4];
                    let g = self.glyph(id(ch.encode_utf8(&mut buf)), cx, y, s, script);
                    cx = g.x1;
                    bb = bb.union(g);
                }
                bb
            }
            Base::Frac(n, d) => {
                let sc = s * self.rng.range(0.8, 0.95);
                let i0 = self.inks.len();
                self.row(n, 0.0, 0.0, sc, script);
                let nb = ink_box(&self.inks[i0..]);
                let i1 = self.inks.len();
                self.row(d, 0.0, 0.0, sc, script);
                let db = ink_box(&self.inks[i1..]);
                let pad = self.rng.range(1.5, 4.0) * s;
                let w = nb.w().max(db.w()) + 2.0 * pad;
                let bar_y = y - 7.0 * s + self.rng.range(-1.0, 1.0) * s;
                let gap = self.rng.range(2.5, 5.0) * s;
                if !nb.is_empty() {
                    shift(
                        &mut self.inks[i0..i1],
                        x + 0.5 * (w - nb.w()) - nb.x0,
                        bar_y - gap - nb.y1,
                    );
                }
                if !db.is_empty() {
                    shift(
                        &mut self.inks[i1..],
                        x + 0.5 * (w - db.w()) - db.x0,
                        bar_y + gap - db.y0,
                    );
                }
                let tilt = self.rng.range(-0.04, 0.04) * w;
                let bar = vec![Pt::new(x, bar_y), Pt::new(x + w, bar_y + tilt)];
                let bar = self.wobble(bar, s);
                self.inks.push(Ink {
                    name: "─",
                    strokes: vec![bar],
                    script,
                });
                ink_box(&self.inks[i0..])
            }
            Base::Sqrt(body) => {
                let i0 = self.inks.len();
                let lead = self.rng.range(9.0, 12.0) * s;
                self.row(body, x + lead, y, s, script);
                let mut bb = ink_box(&self.inks[i0..]);
                if bb.is_empty() {
                    bb = BBox {
                        x0: x + lead,
                        y0: y - XH * s,
                        x1: x + lead + XH * s,
                        y1: y,
                    };
                }
                let m = self.rng.range(2.0, 3.5) * s;
                let (top, bottom) = (bb.y0 - m, bb.y1 + 0.5 * m);
                let mid = 0.5 * (top + bottom);
                let mut check = Vec::new();
                if self.rng.chance(0.4) {
                    check.push(Pt::new(x - 1.5 * s, mid + 3.5 * s));
                }
                check.push(Pt::new(x, mid + 2.0 * s));
                check.push(Pt::new(x + 0.35 * lead, bottom));
                check.push(Pt::new(x + 0.75 * lead, top));
                let end = Pt::new(bb.x1 + m, top + self.rng.range(-0.8, 0.8) * s);
                let strokes = if self.rng.chance(0.25) {
                    vec![
                        self.wobble(check, s),
                        self.wobble(vec![Pt::new(x + 0.72 * lead, top), end], s),
                    ]
                } else {
                    check.push(end);
                    vec![self.wobble(check, s)]
                };
                // The radical is written before its body.
                self.inks.insert(
                    i0,
                    Ink {
                        name: "√",
                        strokes,
                        script,
                    },
                );
                ink_box(&self.inks[i0..])
            }
        }
    }

    /// Small smooth wobble on a constructed stroke.
    fn wobble(&mut self, pts: Vec<Pt>, s: f32) -> Vec<Pt> {
        let mut dense = Vec::new();
        let n = (crate::math::geom::path_len(&pts) / (2.0 * s))
            .ceil()
            .clamp(2.0, 200.0) as usize;
        resample_into(&pts, n, &mut dense);
        let amp = self.st.elastic * 6.0 * s;
        let (ph, f) = (self.rng.range(0.0, 6.3), self.rng.range(0.5, 1.5));
        let k = dense.len().max(2) as f32;
        for (i, p) in dense.iter_mut().enumerate() {
            p.y += amp * (ph + f * std::f32::consts::PI * i as f32 / k).sin();
        }
        dense
    }

    fn glyph(&mut self, sid: SymId, x: f32, y: f32, s: f32, script: Option<usize>) -> BBox {
        let name = sym(sid).name;
        let src = if name == "×" { id("x") } else { sid };
        let Some(list) = self.synth.by_sym.get(&src) else {
            return BBox {
                x0: x,
                y0: y,
                x1: x,
                y1: y,
            };
        };
        let g = &list[self.rng.below(list.len())];
        let mut strokes: Vec<Vec<Pt>> = g
            .strokes
            .iter()
            .map(|st| st.iter().map(|p| Pt::new(p.x, p.y - 9.0)).collect())
            .collect();
        let gb = BBox::of_strokes(strokes.iter().map(|v| v.as_slice()));
        let size = gb.size().max(1.0);
        // Shape tweaks for symbols that differ from their template.
        let (mut sxk, mut syk) = (1.0, 1.0);
        if name == "×" {
            sxk = 0.8;
            syk = 0.8;
        } else if name == "−" {
            sxk = self.rng.range(0.55, 0.9);
        }
        let jit = 1.0 + self.rng.range(-self.st.scale_jitter, self.st.scale_jitter);
        let rot = self
            .rng
            .range(-self.st.rotate_deg, self.st.rotate_deg)
            .to_radians();
        let (c, sn) = (rot.cos(), rot.sin());
        let (cx, cy) = (gb.cx(), gb.cy());
        let amp = self.st.elastic * size;
        let (p1, p2) = (self.rng.range(0.0, 6.3), self.rng.range(0.0, 6.3));
        let (f1, f2) = (self.rng.range(0.5, 1.2), self.rng.range(0.5, 1.2));
        let w = std::f32::consts::TAU / size;
        for st in &mut strokes {
            for p in st.iter_mut() {
                let (dx0, dy0) = (p.x - cx, p.y - cy);
                let ex = amp * (w * f1 * dy0 + p1).sin();
                let ey = amp * (w * f2 * dx0 + p2).sin();
                let (dx, dy) = ((dx0 + ex) * sxk * jit, (dy0 + ey) * syk * jit);
                p.x = cx + dx * c - dy * sn;
                p.y = cy + dx * sn + dy * c;
            }
        }
        // Stroke order, direction and pen lifts.
        let n = strokes.len();
        for i in (1..n).rev() {
            let j = self.rng.below(i + 1);
            strokes.swap(i, j);
        }
        for st in &mut strokes {
            if self.rng.chance(0.3) {
                st.reverse();
            }
        }
        let dot = |v: &[Pt]| crate::math::geom::path_len(v) < 0.15 * size;
        // Join only while the symbol keeps its plausible stroke count.
        let min_strokes = sym(src).strokes.0 as usize;
        let mut joined: Vec<Vec<Pt>> = Vec::with_capacity(n);
        let mut left = n;
        for st in strokes {
            if left > min_strokes
                && let Some(last) = joined.last_mut()
                && !dot(last)
                && !dot(&st)
                && last[last.len() - 1].dist(st[0]) < 0.6 * size
                && self.rng.chance(self.st.join)
            {
                last.extend(st);
                left -= 1;
                continue;
            }
            joined.push(st);
        }
        let b = BBox::of_strokes(joined.iter().map(|v| v.as_slice()));
        let dy = self.rng.range(-1.0, 1.0) * self.st.baseline_jitter * XH;
        let ox = x - b.x0 * s;
        for st in &mut joined {
            for p in st.iter_mut() {
                p.x = ox + p.x * s;
                p.y = y + (p.y + dy) * s;
            }
        }
        let out = BBox::of_strokes(joined.iter().map(|v| v.as_slice()));
        self.inks.push(Ink {
            name,
            strokes: joined,
            script,
        });
        out
    }

    fn finish(mut self, latex: String) -> Result<SynthSample, String> {
        if self.inks.is_empty() {
            return Err("nothing to render".into());
        }
        // Writing order: ink order, with one optional out-of-order stroke.
        let mut order: Vec<(usize, usize)> = Vec::new();
        for (i, ink) in self.inks.iter().enumerate() {
            for k in 0..ink.strokes.len() {
                order.push((i, k));
            }
        }
        if self.rng.chance(self.st.delayed) {
            let scripted: Vec<usize> = (0..self.inks.len())
                .filter(|&i| self.inks[i].script.is_some())
                .collect();
            // Strokes people really add later: dots, crosses, the second
            // stroke of x or +.
            let two: Vec<usize> = (0..self.inks.len().saturating_sub(1))
                .filter(|&i| {
                    self.inks[i].strokes.len() == 2
                        && matches!(
                            self.inks[i].name,
                            "i" | "j" | "t" | "f" | "x" | "×" | "+" | "≠"
                        )
                })
                .collect();
            if !scripted.is_empty() && self.rng.chance(0.5) {
                let g = self.inks[scripted[self.rng.below(scripted.len())]].script;
                let (moved, mut keep): (Vec<_>, Vec<_>) = order
                    .into_iter()
                    .partition(|(i, _)| self.inks[*i].script == g);
                keep.extend(moved);
                order = keep;
            } else if !two.is_empty() {
                let i = two[self.rng.below(two.len())];
                // The shorter stroke comes after the next symbol.
                let short = if crate::math::geom::path_len(&self.inks[i].strokes[0])
                    < crate::math::geom::path_len(&self.inks[i].strokes[1])
                {
                    0
                } else {
                    1
                };
                if let Some(pos) = order.iter().position(|&o| o == (i, short)) {
                    let o = order.remove(pos);
                    let after = order
                        .iter()
                        .rposition(|&(j, _)| j == i + 1)
                        .map_or(order.len(), |p| p + 1);
                    order.insert(after, o);
                }
            }
        }
        // Global slant, screen scale, sampling and noise.
        let all = ink_box(&self.inks);
        let slant = self
            .rng
            .range(-self.st.slant_deg, self.st.slant_deg)
            .to_radians()
            .tan();
        let xh_px = self.rng.range(self.st.xh_px.0, self.st.xh_px.1);
        let k = xh_px / XH;
        let (ox, oy) = (self.rng.range(50.0, 400.0), self.rng.range(50.0, 400.0));
        let ymid = all.cy();
        let sigma = self.st.noise * xh_px;
        let mut strokes = Vec::with_capacity(order.len());
        let mut owner: Vec<usize> = Vec::with_capacity(order.len());
        for &(i, s) in &order {
            let src: Vec<Pt> = self.inks[i].strokes[s]
                .iter()
                .map(|p| Pt::new(ox + k * (p.x + slant * (ymid - p.y)), oy + k * (p.y - ymid)))
                .collect();
            let len = crate::math::geom::path_len(&src);
            let spacing = self.rng.range(1.2, 3.0);
            let n = ((len / spacing).ceil() as usize).clamp(2, 400);
            let mut pts = Vec::with_capacity(n);
            resample_into(&src, n, &mut pts);
            for p in &mut pts {
                p.x += sigma * self.rng.normal();
                p.y += sigma * self.rng.normal();
            }
            let smooth: Vec<Pt> = (0..pts.len())
                .map(|j| {
                    let a = pts[j.saturating_sub(1)];
                    let b = pts[j];
                    let c = pts[(j + 1).min(pts.len() - 1)];
                    Pt::new((a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0)
                })
                .collect();
            strokes.push(
                smooth
                    .iter()
                    .enumerate()
                    .map(|(j, p)| StrokePoint {
                        x: p.x,
                        y: p.y,
                        p: 0.5,
                        dt: j as u32 * 8,
                    })
                    .collect(),
            );
            owner.push(i);
        }
        let symbols = self
            .inks
            .iter()
            .enumerate()
            .map(|(i, ink)| {
                let ids: Vec<usize> = (0..owner.len()).filter(|&k| owner[k] == i).collect();
                (ink.name, ids)
            })
            .collect();
        Ok(SynthSample {
            latex,
            strokes,
            symbols,
        })
    }
}

/// Simple expressions (no fractions or radicals) of the reported test set.
pub const SIMPLE: &[&str] = &[
    "x^2",
    "a^2+b^2=c^2",
    "2x+3=7",
    "y=mx+c",
    "E=mc^2",
    "v=u+at",
    "H_2O",
    "CO_2",
    "30^\\circ",
    "\\sin\\theta",
    "x_1",
    "x_{n+1}",
    "10^{-3}",
    "3x-5=10",
    "x^2+3x-4=0",
    "2H_2+O_2\\to 2H_2O",
    "\\cos\\theta",
    "F=ma",
    "y=x^2-1",
    "a\\neq b",
    "x\\leq 5",
    "y\\geq 0",
    "2\\pi r",
    "\\pi r^2",
    "\\Delta x",
    "\\alpha+\\beta=90^\\circ",
    "\\lambda=2",
    "3\\times 4=12",
    "12\\div 4=3",
    "x=\\pm 2",
    "f(x)=2x+1",
    "(a+b)^2",
    "e^{2x}",
    "k=7",
    "n_1=1.5",
    "3.14",
    "\\mu=0.2",
    "\\Sigma x",
    "x\\to\\infty",
    "p\\cdot q",
    "t=2s",
    "R=8.3",
    "y_2-y_1",
    "4x^3",
    "b^2-4ac",
    "\\int x",
    "\\tan x",
    "\\log x",
    "CH_4",
    "x<y",
    "m>0",
    "6\\times 7",
    "a_n=a_1+d",
    "f'(x)",
    "z=9",
    "w=hg",
];

/// Expressions with fractions and radicals.
pub const STRUCTURED: &[&str] = &[
    "\\frac{1}{2}",
    "\\frac{a+b}{c}",
    "\\sqrt{x+1}",
    "s=ut+\\frac{1}{2}at^2",
    "\\frac{x}{2}=3",
    "\\sqrt{2}",
    "x=\\frac{-b}{2a}",
    "\\frac{dy}{dx}",
    "\\sqrt{a^2+b^2}",
    "\\frac{1}{x}+1",
    "y=\\sqrt{x}",
    "\\frac{3}{4}",
    "v=\\frac{d}{t}",
    "\\frac{\\pi}{2}",
];

/// The expressions the brief names explicitly.
pub const REQUIRED: &[&str] = &[
    "x^2",
    "a^2+b^2=c^2",
    "\\frac{1}{2}",
    "\\frac{a+b}{c}",
    "\\sqrt{x+1}",
    "2x+3=7",
    "y=mx+c",
    "E=mc^2",
    "v=u+at",
    "s=ut+\\frac{1}{2}at^2",
    "H_2O",
    "CO_2",
    "30^\\circ",
    "\\sin\\theta",
    "x_1",
    "x_{n+1}",
    "10^{-3}",
];

/// Accuracy of a recognizer on a synthetic set.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub samples: usize,
    /// Top-1 LaTeX equals the truth.
    pub correct: usize,
    /// The truth is somewhere in the n-best list.
    pub in_n_best: usize,
    /// (correct, total) on expressions without fractions or radicals.
    pub simple: (usize, usize),
    /// (correct, total) on expressions with them.
    pub structured: (usize, usize),
    pub symbols: usize,
    /// Symbols whose strokes were grouped exactly right.
    pub segmented: usize,
    /// Symbols grouped right and labelled right.
    pub symbols_correct: usize,
    /// (truth, read) → count, for correctly grouped but mislabelled symbols.
    pub confusions: Vec<((&'static str, &'static str), usize)>,
    /// Per expression: (truth, correct, total).
    pub per_expr: Vec<(String, usize, usize)>,
    /// Some failures: (truth, top-1, render seed).
    pub failures: Vec<(String, String, u64)>,
    /// Calibration: per confidence decile, (top-1 count, correct count).
    pub by_confidence: [(usize, usize); 10],
}

impl Report {
    pub fn rate(&self) -> f32 {
        self.correct as f32 / self.samples.max(1) as f32
    }

    pub fn simple_rate(&self) -> f32 {
        self.simple.0 as f32 / self.simple.1.max(1) as f32
    }

    pub fn structured_rate(&self) -> f32 {
        self.structured.0 as f32 / self.structured.1.max(1) as f32
    }

    pub fn symbol_rate(&self) -> f32 {
        self.symbols_correct as f32 / self.symbols.max(1) as f32
    }
}

/// Score `rec` on `exprs`, each rendered once per seed.
pub fn evaluate(
    rec: &MathRecognizer,
    exprs: &[&str],
    seeds: std::ops::Range<u64>,
    style: &SynthStyle,
) -> Report {
    let synth = Synth::new();
    let mut rep = Report::default();
    let mut conf: HashMap<(&'static str, &'static str), usize> = HashMap::new();
    for (ei, e) in exprs.iter().enumerate() {
        let simple = !e.contains("\\frac") && !e.contains("\\sqrt");
        let (mut ok_n, mut n) = (0, 0);
        for seed in seeds.clone() {
            let seed = seed * 1009 + ei as u64;
            let Ok(sample) = synth.render(e, seed, style) else {
                continue;
            };
            let cands: Vec<Candidate> = rec.recognize(&sample.strokes);
            let top = cands.first();
            let ok = top.is_some_and(|c| c.latex == sample.latex);
            n += 1;
            rep.samples += 1;
            if ok {
                ok_n += 1;
                rep.correct += 1;
            } else if rep.failures.len() < 200 {
                rep.failures.push((
                    sample.latex.clone(),
                    top.map(|c| c.latex.clone()).unwrap_or_default(),
                    seed,
                ));
            }
            if cands.iter().any(|c| c.latex == sample.latex) {
                rep.in_n_best += 1;
            }
            if simple {
                rep.simple.1 += 1;
                rep.simple.0 += ok as usize;
            } else {
                rep.structured.1 += 1;
                rep.structured.0 += ok as usize;
            }
            if let Some(c) = top {
                let d = ((c.confidence * 10.0) as usize).min(9);
                rep.by_confidence[d].0 += 1;
                rep.by_confidence[d].1 += ok as usize;
            }
            for (name, ids) in &sample.symbols {
                rep.symbols += 1;
                if let Some(rs) = top.and_then(|c| c.symbols.iter().find(|s| s.strokes == *ids)) {
                    rep.segmented += 1;
                    if rs.symbol == *name {
                        rep.symbols_correct += 1;
                    } else {
                        *conf.entry((name, rs.symbol)).or_default() += 1;
                    }
                }
            }
        }
        rep.per_expr.push((e.to_string(), ok_n, n));
    }
    let mut c: Vec<_> = conf.into_iter().collect();
    c.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rep.confusions = c;
    rep
}
