//! Lay out a parsed line ([`super::parse::Node`]) as pen strokes in writing
//! units (baseline y = 0, x-height 14), in the order a person writes them:
//! left to right; a fraction's numerator, bar, then denominator; a root's
//! sign before its radicand; a base before its scripts.
//!
//! Natural variation is deterministic: it comes from a seeded generator, so
//! the same lines with the same seed always give the same ink.

use super::glyphs::{Glyph, GlyphSet, XH, smooth};
use super::parse::Node;
use crate::math::geom::{BBox, Pt};
use crate::math::synth::Rng;

/// What comes before a stroke, for the pen's pauses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pause {
    /// Another stroke of the same glyph.
    Stroke,
    /// The next glyph of a word or formula.
    Glyph,
    /// After a space.
    Word,
    /// The first stroke of a line.
    Line,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawStroke {
    pub pts: Vec<Pt>,
    pub pause: Pause,
}

/// A laid-out piece: its strokes, advance width and vertical extent
/// (`top` is negative above the baseline, `bottom` positive below it).
#[derive(Clone, Debug, Default)]
pub struct LBox {
    pub strokes: Vec<RawStroke>,
    pub w: f32,
    pub top: f32,
    pub bottom: f32,
}

impl LBox {
    fn empty(w: f32) -> Self {
        LBox {
            strokes: Vec::new(),
            w,
            top: 0.0,
            bottom: 0.0,
        }
    }

    fn shift(&mut self, dx: f32, dy: f32) {
        for s in &mut self.strokes {
            for p in &mut s.pts {
                p.x += dx;
                p.y += dy;
            }
        }
        self.top += dy;
        self.bottom += dy;
    }

    /// Append `other` (already positioned).
    fn absorb(&mut self, other: LBox) {
        if other.strokes.is_empty() {
            return;
        }
        self.top = self.top.min(other.top);
        self.bottom = self.bottom.max(other.bottom);
        self.strokes.extend(other.strokes);
    }

    pub fn ink(&self) -> BBox {
        BBox::of_strokes(self.strokes.iter().map(|s| s.pts.as_slice()))
    }
}

/// Variation knobs (all deterministic through the generator).
#[derive(Clone, Copy, Debug)]
pub struct Variation {
    /// 0 = machine-perfect, 1 = clearly hand-made.
    pub jitter: f32,
    /// Prefer the teacher's own forms when they exist.
    pub prefer_user: bool,
}

pub struct Layout<'a> {
    pub glyphs: &'a GlyphSet,
    pub var: Variation,
    pub rng: Rng,
    /// Characters with no glyph (written as `?`).
    pub missing: Vec<char>,
}

impl Layout<'_> {
    fn normal(&mut self) -> f32 {
        self.rng.normal().clamp(-2.5, 2.5)
    }

    /// The form to write `c` with, and whether it is a built-in (font) form.
    fn form(&mut self, c: char) -> (Glyph, bool) {
        if self.var.prefer_user {
            let forms = self.glyphs.user_forms(c);
            if !forms.is_empty() {
                let k = self.rng.below(forms.len());
                return (forms[k].clone(), false);
            }
        }
        if let Some(g) = self.glyphs.builtin(c) {
            return (g.clone(), true);
        }
        if !self.missing.contains(&c) {
            self.missing.push(c);
        }
        let q = self.glyphs.builtin('?').cloned().unwrap_or(Glyph {
            strokes: Vec::new(),
            adv: XH,
        });
        (q, true)
    }

    /// One glyph at scale `s`, its left edge at x = 0.
    pub fn glyph(&mut self, c: char, s: f32) -> LBox {
        let (mut g, builtin) = self.form(c);
        if builtin && let Some((kx, ky)) = op_scale(c) {
            // Font operators are drawn wide; on a board they are about an
            // x-height across.
            let cx = g.adv * 0.5;
            let cy = -9.0;
            for st in &mut g.strokes {
                for p in st {
                    p.x = (p.x - cx) * kx + cx * kx;
                    p.y = (p.y - cy) * ky + cy;
                }
            }
            g.adv *= kx;
        }
        let j = self.var.jitter;
        let k = 1.0 + 0.04 * j * self.normal();
        let rot = (1.6 * j * self.normal()).to_radians();
        let dy = 0.5 * j * self.normal();
        // A slow elastic wobble, so no two glyphs are the same shape.
        let amp = 0.8 * j;
        let (fx, fy) = (self.rng.range(0.35, 0.6), self.rng.range(0.35, 0.6));
        let (px, py) = (self.rng.range(0.0, 6.3), self.rng.range(0.0, 6.3));
        let (sin, cos) = rot.sin_cos();
        let cx = g.adv * 0.5;
        let cy = -XH * 0.5;
        let mut out = LBox::empty(g.adv * s * k);
        for (i, st) in g.strokes.iter().enumerate() {
            let pts = if builtin { smooth(st) } else { st.clone() };
            let pts: Vec<Pt> = pts
                .into_iter()
                .map(|p| {
                    let (x, y) = (p.x - cx, p.y - cy);
                    let (x, y) = (
                        x + amp * (fy * p.y + py).sin(),
                        y + amp * (fx * p.x + px).sin(),
                    );
                    let (x, y) = (x * cos - y * sin, x * sin + y * cos);
                    Pt::new((x * k + cx) * s, (y * k + cy + dy) * s)
                })
                .collect();
            out.strokes.push(RawStroke {
                pts,
                pause: if i == 0 { Pause::Glyph } else { Pause::Stroke },
            });
        }
        let b = out.ink();
        if b.y0.is_finite() {
            out.top = b.y0.min(-XH * 0.5 * s);
            out.bottom = b.y1.max(0.0);
        } else {
            out.top = -XH * s;
        }
        out
    }

    /// Gap between neighbouring glyphs.
    fn kern(&mut self, s: f32) -> f32 {
        (-0.6 + 0.6 * self.var.jitter * (self.rng.f32() - 0.5)) * s
    }

    pub fn row(&mut self, nodes: &[Node], s: f32) -> LBox {
        let mut out = LBox::empty(0.0);
        out.top = -XH * s;
        let mut x = 0.0f32;
        let mut after_space = false;
        let mut first = true;
        for n in nodes {
            if let Node::Space(em) = n {
                x += em * XH * s * (1.0 + 0.15 * self.var.jitter * (self.rng.f32() - 0.5));
                after_space = true;
                continue;
            }
            let mut b = self.node(n, s);
            if b.strokes.is_empty() && b.w <= 0.0 {
                continue;
            }
            if !first {
                x += self.kern(s);
            }
            if after_space && let Some(st) = b.strokes.first_mut() {
                st.pause = Pause::Word;
            }
            after_space = false;
            first = false;
            b.shift(x, 0.0);
            x += b.w;
            out.absorb(b);
        }
        out.w = x.max(0.0);
        out
    }

    fn node(&mut self, n: &Node, s: f32) -> LBox {
        match n {
            Node::Char(c) => self.glyph(*c, s),
            Node::Space(em) => LBox::empty(em * XH * s),
            Node::Op(c) => {
                let pad = 4.5 * s;
                let mut b = self.glyph(*c, s);
                b.shift(pad, 0.0);
                b.w += 2.0 * pad;
                b
            }
            Node::Func(name) => {
                let mut out = LBox::empty(0.0);
                out.top = -XH * s;
                let mut x = 0.0;
                for (i, c) in name.chars().enumerate() {
                    let mut g = self.glyph(c, s);
                    if i > 0 {
                        x -= 1.2 * s;
                    }
                    g.shift(x, 0.0);
                    x += g.w;
                    out.absorb(g);
                }
                out.w = x + 1.5 * s;
                out
            }
            Node::Group(g) => self.row(g, s),
            Node::Frac(num, den) => self.frac(num, den, s),
            Node::Sqrt(index, body) => self.sqrt(index, body, s),
            Node::Script { base, sup, sub } => self.script(base, sup.as_deref(), sub.as_deref(), s),
        }
    }

    fn frac(&mut self, num: &[Node], den: &[Node], s: f32) -> LBox {
        let sc = s * 0.88;
        let mut n = self.row(num, sc);
        let mut d = self.row(den, sc);
        let pad = 2.5 * s;
        let w = n.w.max(d.w) + 2.0 * pad;
        let bar_y = -XH * 0.5 * s + 0.3 * self.var.jitter * self.normal() * s;
        let gap = 3.0 * s;
        n.shift((w - n.w) * 0.5, bar_y - gap - n.bottom);
        d.shift((w - d.w) * 0.5, bar_y + gap - d.top);
        let tilt = 0.6 * self.var.jitter * self.normal() * s;
        let bar = vec![
            Pt::new(0.0, bar_y),
            Pt::new(w * 0.5, bar_y + tilt * 0.4),
            Pt::new(w, bar_y + tilt),
        ];
        let mut out = LBox::empty(w);
        out.top = n.top.min(bar_y);
        out.bottom = d.bottom.max(bar_y);
        let first_pause = Pause::Glyph;
        if let Some(st) = n.strokes.first_mut() {
            st.pause = first_pause;
        }
        out.strokes.extend(n.strokes);
        out.strokes.push(RawStroke {
            pts: bar,
            pause: Pause::Glyph,
        });
        out.strokes.extend(d.strokes);
        out.w = w + 1.0 * s;
        out
    }

    fn sqrt(&mut self, index: &[Node], body: &[Node], s: f32) -> LBox {
        let mut b = self.row(body, s);
        let top = b.top.min(-XH * s) - 3.0 * s;
        let bottom = b.bottom.max(0.0) + 1.0 * s;
        let mut ix = if index.is_empty() {
            None
        } else {
            Some(self.row(index, s * 0.55))
        };
        let lead = ix.as_ref().map_or(0.0, |i| (i.w - 3.0 * s).max(0.0));
        let tick_y = (top + bottom) * 0.5 + 2.0 * s;
        let wob = 0.3 * self.var.jitter * self.normal() * s;
        let x_body = lead + 11.0 * s;
        let sign = vec![
            Pt::new(lead, tick_y),
            Pt::new(lead + 2.5 * s, tick_y - 1.8 * s),
            Pt::new(lead + 6.0 * s, bottom),
            Pt::new(lead + 9.5 * s, top + wob),
            Pt::new(x_body + b.w + 1.5 * s, top),
        ];
        let mut out = LBox::empty(x_body + b.w + 2.5 * s);
        out.top = top;
        out.bottom = bottom;
        if let Some(i) = ix.as_mut() {
            i.shift(0.0, tick_y - 2.5 * s - i.bottom);
            out.top = out.top.min(i.top);
            out.strokes.append(&mut i.strokes);
        }
        out.strokes.push(RawStroke {
            pts: sign,
            pause: Pause::Glyph,
        });
        b.shift(x_body, 0.0);
        out.strokes.extend(b.strokes);
        out
    }

    fn script(&mut self, base: &Node, sup: Option<&[Node]>, sub: Option<&[Node]>, s: f32) -> LBox {
        let b = self.node(base, s);
        let ss = s * 0.62;
        let x = b.w + 0.3 * s;
        let base_top = b.top.min(-XH * s);
        let mut out = LBox::empty(b.w);
        out.top = b.top;
        out.bottom = b.bottom;
        out.strokes = b.strokes;
        let mut end = x;
        if let Some(sp) = sup {
            let mut r = self.row(sp, ss);
            let baseline = base_top * 0.6 + 0.3 * self.var.jitter * self.normal() * s;
            r.shift(x, baseline);
            end = end.max(x + r.w);
            out.absorb(r);
        }
        if let Some(sb) = sub {
            let mut r = self.row(sb, ss);
            r.shift(x, 4.5 * s);
            end = end.max(x + r.w);
            out.absorb(r);
        }
        out.w = end + 0.5 * s;
        out
    }
}

/// Horizontal and vertical scale for font operators (drawn ~26 units wide).
fn op_scale(c: char) -> Option<(f32, f32)> {
    match c {
        '+' | '-' | '−' | '=' | '±' | '∓' | '≠' | '≡' | '≈' | '~' => Some((0.72, 0.85)),
        '×' | '÷' | '<' | '>' | '≤' | '≥' => Some((0.7, 0.8)),
        '→' | '←' => Some((0.75, 0.85)),
        _ => None,
    }
}

/// Split a line's nodes into visual lines no wider than `max_w` units,
/// breaking only at spaces. Widths are measured with a copy of the
/// generator, so measuring does not change the ink.
pub fn wrap(lay: &mut Layout<'_>, nodes: &[Node], max_w: f32) -> Vec<Vec<Node>> {
    let mut words: Vec<Vec<Node>> = vec![Vec::new()];
    for n in nodes {
        if matches!(n, Node::Space(_)) {
            if words.last().is_some_and(|w| !w.is_empty()) {
                words.push(Vec::new());
            }
        } else if let Some(w) = words.last_mut() {
            w.push(n.clone());
        }
    }
    words.retain(|w| !w.is_empty());
    if words.len() <= 1 {
        return vec![nodes.to_vec()];
    }
    let saved = lay.rng.clone();
    let space = 0.6 * XH;
    let mut out: Vec<Vec<Node>> = Vec::new();
    let mut cur: Vec<Node> = Vec::new();
    let mut cur_w = 0.0;
    for w in words {
        let ww = lay.row(&w, 1.0).w;
        if !cur.is_empty() && cur_w + space + ww > max_w {
            out.push(std::mem::take(&mut cur));
            cur_w = 0.0;
        }
        if !cur.is_empty() {
            cur.push(Node::Space(0.6));
            cur_w += space;
        }
        cur.extend(w);
        cur_w += ww;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    lay.rng = saved;
    out
}
