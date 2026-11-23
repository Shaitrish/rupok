//! Layout analysis: from positioned symbols to a tree of rows, fractions,
//! radicals and scripts.
//!
//! Per region (the whole expression, a numerator, a radical's body, a
//! script):
//!
//! 1. **Radicals** (widest first) take the symbols inside them.
//! 2. **Fraction bars** (widest first): a flat line with symbols above and
//!    below it, within its width. A flat line with nothing above or below
//!    stays a minus sign.
//! 3. **Baseline**: the rest, left to right. Each symbol that can carry
//!    scripts collects the following symbols that sit clearly higher
//!    (superscript) or lower (subscript) than its x-height band, judged by
//!    baselines, centres and relative size; the first symbol back on its
//!    band ends the scripts. Scripts are laid out recursively as regions.
//!
//! Each symbol's x-height band comes from its profile (an `x` fills it, a
//! `2` stands on it and is taller, a `y` hangs below it, a `+` is centred
//! on it), so `2x`, `x^2`, `x_1` and `H_2O` separate without a trained
//! model.

use super::geom::BBox;
use super::segment::Group;
use super::symbols::{Cat, Profile, SymId, id, sym};

/// A symbol as layout and context see it.
#[derive(Clone, Debug)]
pub struct SymNode {
    pub group: Group,
    /// Box in deslanted coordinates.
    pub bbox: BBox,
    pub label: SymId,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LBase {
    Sym(usize),
    Frac(usize, LRow, LRow),
    Sqrt(usize, LRow),
}

#[derive(Clone, Debug, PartialEq)]
pub struct LAtom {
    pub base: LBase,
    pub sub: LRow,
    pub sup: LRow,
}

pub type LRow = Vec<LAtom>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rel {
    Right,
    Sup,
    Sub,
}

/// A script decision that was close, so the n-best list can try the other
/// reading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Doubt {
    pub base: usize,
    pub target: usize,
    pub other: Rel,
    /// How far past the threshold the decision was (0 = on it).
    pub margin: f32,
}

#[derive(Clone, Debug)]
enum Item {
    Sym(usize),
    Frac(usize, Vec<Item>, Vec<Item>),
    Sqrt(usize, Vec<Item>),
}

/// Script decisions closer than this to a threshold are recorded as doubts.
const DOUBT_BAND: f32 = 0.15;

pub struct Layout<'a> {
    syms: &'a [SymNode],
    unit: f32,
    /// Forced relations (base, target, relation), for n-best alternatives.
    forced: &'a [(usize, usize, Rel)],
    pub doubts: Vec<Doubt>,
}

impl<'a> Layout<'a> {
    pub fn new(syms: &'a [SymNode], unit: f32, forced: &'a [(usize, usize, Rel)]) -> Self {
        Layout {
            syms,
            unit,
            forced,
            doubts: Vec::new(),
        }
    }

    pub fn run(&mut self) -> LRow {
        let items = (0..self.syms.len()).map(Item::Sym).collect();
        self.region(items)
    }

    fn bbox(&self, it: &Item) -> BBox {
        match it {
            Item::Sym(i) => self.syms[*i].bbox,
            Item::Frac(b, n, d) => n
                .iter()
                .chain(d)
                .fold(self.syms[*b].bbox, |a, x| a.union(self.bbox(x))),
            Item::Sqrt(r, body) => body
                .iter()
                .fold(self.syms[*r].bbox, |a, x| a.union(self.bbox(x))),
        }
    }

    fn label(&self, i: usize) -> SymId {
        self.syms[i].label
    }

    fn is_flat(&self, i: usize) -> bool {
        let s = &self.syms[i];
        let b = s.bbox;
        !s.group.dot
            && b.w() >= 0.4 * self.unit
            && b.h() <= 0.25 * b.w()
            && (s.label == id("−") || b.h() <= 0.15 * b.w())
    }

    fn region(&mut self, mut items: Vec<Item>) -> LRow {
        self.radicals(&mut items);
        self.fractions(&mut items);
        items.sort_by(|a, b| self.bbox(a).x0.total_cmp(&self.bbox(b).x0));
        self.baseline(items)
    }

    fn radicals(&mut self, items: &mut Vec<Item>) {
        let root = id("√");
        loop {
            let mut rads: Vec<usize> = items
                .iter()
                .filter_map(|it| match it {
                    Item::Sym(i) if self.label(*i) == root => Some(*i),
                    _ => None,
                })
                .collect();
            if rads.is_empty() {
                return;
            }
            rads.sort_by(|a, b| self.syms[*b].bbox.w().total_cmp(&self.syms[*a].bbox.w()));
            let r = rads[0];
            let rb = self.syms[r].bbox;
            let u = self.unit;
            let left = rb.x0 + 0.25 * rb.h().min(rb.w());
            let mut body = Vec::new();
            let mut rest = Vec::new();
            for it in items.drain(..) {
                if matches!(it, Item::Sym(i) if i == r) {
                    continue;
                }
                let b = self.bbox(&it);
                let inside = b.cx() > left
                    && b.cx() < rb.x1 + 0.15 * u
                    && b.cy() > rb.y0
                    && b.cy() < rb.y1 + 0.2 * u;
                if inside { body.push(it) } else { rest.push(it) }
            }
            rest.push(Item::Sqrt(r, body));
            *items = rest;
        }
    }

    fn fractions(&mut self, items: &mut Vec<Item>) {
        let u = self.unit;
        let mut tried: Vec<usize> = Vec::new();
        loop {
            let mut bars: Vec<usize> = items
                .iter()
                .filter_map(|it| match it {
                    Item::Sym(i) if self.is_flat(*i) && !tried.contains(i) => Some(*i),
                    _ => None,
                })
                .collect();
            if bars.is_empty() {
                return;
            }
            bars.sort_by(|a, b| self.syms[*b].bbox.w().total_cmp(&self.syms[*a].bbox.w()));
            let bar = bars[0];
            tried.push(bar);
            let bb = self.syms[bar].bbox;
            let by = bb.cy();
            let over = |b: BBox| {
                if b.w() < 0.3 * u {
                    b.cx() >= bb.x0 - 0.1 * bb.w() && b.cx() <= bb.x1 + 0.1 * bb.w()
                } else {
                    b.x_overlap(bb) >= 0.5 * b.w()
                }
            };
            let (mut above, mut below, mut rest) = (Vec::new(), Vec::new(), Vec::new());
            for it in items.drain(..) {
                if matches!(it, Item::Sym(i) if i == bar) {
                    continue;
                }
                let b = self.bbox(&it);
                if over(b) && b.y1 <= by + 0.25 * u && b.cy() < by && b.y1 > by - 3.0 * u {
                    above.push(it);
                } else if over(b) && b.y0 >= by - 0.25 * u && b.cy() > by && b.y0 < by + 3.0 * u {
                    below.push(it);
                } else {
                    rest.push(it);
                }
            }
            let all_dots = |v: &[Item]| {
                v.iter()
                    .all(|it| matches!(it, Item::Sym(i) if self.syms[*i].group.dot))
            };
            if above.is_empty() || below.is_empty() || (all_dots(&above) && all_dots(&below)) {
                rest.push(Item::Sym(bar));
                rest.extend(above);
                rest.extend(below);
            } else {
                rest.push(Item::Frac(bar, above, below));
            }
            *items = rest;
        }
    }

    /// x-height of a region: a high percentile of per-symbol estimates, so
    /// smaller scripts don't drag it down.
    fn xh(&self, items: &[Item]) -> f32 {
        let mut v: Vec<f32> = Vec::new();
        let mut hs: Vec<f32> = Vec::new();
        for it in items {
            if let Item::Sym(i) = it {
                let s = &self.syms[*i];
                let h = s.bbox.h();
                if !s.group.dot {
                    hs.push(h);
                }
                match sym(s.label).profile {
                    Profile::XHeight => v.push(h),
                    Profile::Ascender | Profile::Descender => v.push(0.6 * h),
                    Profile::Both => v.push(0.5 * h),
                    _ => {}
                }
            }
        }
        let pick = |v: &mut Vec<f32>| {
            v.sort_by(f32::total_cmp);
            v[(v.len() * 2) / 3]
        };
        if !v.is_empty() {
            pick(&mut v).max(0.05 * self.unit)
        } else if !hs.is_empty() {
            (0.6 * pick(&mut hs)).max(0.3 * self.unit)
        } else {
            0.6 * self.unit
        }
    }

    /// The x-height band (top, bottom) an item stands in.
    fn band(&self, it: &Item, xh: f32) -> (f32, f32) {
        let b = self.bbox(it);
        match it {
            Item::Sym(i) => {
                let h = b.h();
                match sym(self.label(*i)).profile {
                    Profile::XHeight => (b.y0, b.y1),
                    Profile::Ascender => (b.y1 - 0.6 * h, b.y1),
                    Profile::Descender => (b.y0, b.y0 + 0.6 * h),
                    Profile::Both => (b.y0 + 0.25 * h, b.y0 + 0.75 * h),
                    Profile::Center | Profile::Tall => (b.cy() - 0.5 * xh, b.cy() + 0.5 * xh),
                    Profile::Dot => (b.y1 - xh, b.y1),
                    Profile::High => (b.y1, b.y1 + xh),
                }
            }
            Item::Frac(bar, _, _) => {
                let y = self.syms[*bar].bbox.cy();
                (y - 0.5 * xh, y + 0.5 * xh)
            }
            Item::Sqrt(..) => (b.y1 - 1.2 * xh, b.y1 - 0.2 * xh),
        }
    }

    fn can_take_scripts(&self, it: &Item) -> bool {
        match it {
            Item::Sym(i) => matches!(
                sym(self.label(*i)).cat,
                Cat::Digit
                    | Cat::Letter
                    | Cat::Upper
                    | Cat::Greek
                    | Cat::Close
                    | Cat::Big
                    | Cat::Bar
            ),
            _ => false,
        }
    }

    fn sym_index(it: &Item) -> usize {
        match it {
            Item::Sym(i) | Item::Frac(i, _, _) | Item::Sqrt(i, _) => *i,
        }
    }

    /// How `t` relates to the base `s`, plus the other reading and the
    /// margin when the decision was close.
    fn relation(&self, s: &Item, t: &Item, xh: f32) -> (Rel, Option<(Rel, f32)>) {
        let (st, sb) = self.band(s, xh);
        if let Item::Sym(ti) = t {
            let ts = sym(self.label(*ti));
            if ts.cat == Cat::Mark && self.bbox(t).cy() < self.bbox(s).cy() {
                return (Rel::Sup, None);
            }
            // A dot is on the line (".", "·") unless clearly above or below it.
            if self.syms[*ti].group.dot {
                let (cy, h) = (self.bbox(t).cy(), (sb - st).max(0.3 * xh));
                return if cy < st - 0.05 * h {
                    (Rel::Sup, None)
                } else if cy > sb + 0.5 * h {
                    (Rel::Sub, None)
                } else {
                    (Rel::Right, None)
                };
            }
        }
        let (tt, tb) = self.band(t, xh);
        let hs = (sb - st).max(0.3 * xh);
        let c = (0.5 * (tt + tb) - 0.5 * (st + sb)) / hs;
        let dbot = (tb - sb) / hs;
        let dtop = (tt - st) / hs;
        let size = (tb - tt) / hs;
        // Scores > 0 mean "yes"; the larger, the surer.
        let sup = (-0.45 - dbot).min(-0.35 - c);
        let sup_small = (-0.3 - dbot).min(0.85 - size).min(-0.2 - c);
        let sub = (c - 0.45).min(dtop - 0.25);
        let sub_small = (c - 0.25).min(0.85 - size).min(dtop - 0.1);
        let sup = sup.max(sup_small);
        let sub = sub.max(sub_small);
        if sup > 0.0 {
            (Rel::Sup, (sup < DOUBT_BAND).then_some((Rel::Right, sup)))
        } else if sub > 0.0 {
            (Rel::Sub, (sub < DOUBT_BAND).then_some((Rel::Right, sub)))
        } else if -sup < DOUBT_BAND && sup >= sub {
            (Rel::Right, Some((Rel::Sup, -sup)))
        } else if -sub < DOUBT_BAND {
            (Rel::Right, Some((Rel::Sub, -sub)))
        } else {
            (Rel::Right, None)
        }
    }

    fn baseline(&mut self, items: Vec<Item>) -> LRow {
        let xh = self.xh(&items);
        let mut out = Vec::new();
        let mut it = items.into_iter().peekable();
        while let Some(base) = it.next() {
            let (mut sup, mut sub) = (Vec::new(), Vec::new());
            if self.can_take_scripts(&base) {
                let bi = Self::sym_index(&base);
                while let Some(t) = it.peek() {
                    let ti = Self::sym_index(t);
                    let forced = self
                        .forced
                        .iter()
                        .find(|f| f.0 == bi && f.1 == ti)
                        .map(|f| f.2);
                    let (rel, doubt) = self.relation(&base, t, xh);
                    if forced.is_none()
                        && let Some((other, margin)) = doubt
                    {
                        self.doubts.push(Doubt {
                            base: bi,
                            target: ti,
                            other,
                            margin,
                        });
                    }
                    let Some(next) = (match forced.unwrap_or(rel) {
                        Rel::Right => None,
                        r => it.next().map(|n| (r, n)),
                    }) else {
                        break;
                    };
                    match next.0 {
                        Rel::Sup => sup.push(next.1),
                        _ => sub.push(next.1),
                    }
                }
            }
            let base = match base {
                Item::Sym(i) => LBase::Sym(i),
                Item::Frac(b, n, d) => {
                    let n = self.region(n);
                    let d = self.region(d);
                    LBase::Frac(b, n, d)
                }
                Item::Sqrt(r, body) => LBase::Sqrt(r, self.region(body)),
            };
            let sub = if sub.is_empty() {
                Vec::new()
            } else {
                self.region(sub)
            };
            let sup = if sup.is_empty() {
                Vec::new()
            } else {
                self.region(sup)
            };
            out.push(LAtom { base, sub, sup });
        }
        out
    }
}
