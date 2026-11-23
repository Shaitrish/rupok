//! Context: choose each symbol's label from its candidates using a small
//! expression grammar and the symbol's size and position on its row.
//!
//! Every row of the layout tree (the main line, each script, numerator,
//! denominator and radical body) is decoded separately with Viterbi:
//!
//! - **Emission**: classifier distance, the symbol's rarity prior, and how
//!   well the candidate's profile fits the row's baseline and x-height
//!   (so a small round symbol is `o`, a tall one `O` or `0`; `p` hangs
//!   below the line, `P` does not; `·` floats, `.` sits).
//! - **Transitions** between grammar categories: two binary operators in a
//!   row, an operator at the end, or a relation at the start are unlikely;
//!   a minus after `=` or `(` is fine (unary). This is what turns the same
//!   cross into `×` in `3×4` but `x` in `2x+1`, and the same circle into
//!   `0` in `10` but `O` in `CO_2`.
//!
//! After decoding, small fixed rules: a tiny circle raised after a symbol is
//! a degree sign, a tiny raised tick a prime, and letter runs spelling
//! `sin cos tan log ln` become function names.

use super::geom::BBox;
use super::layout::{LBase, LRow, SymNode};
use super::segment::TAU;
use super::symbols::{Cat, Profile, SymId, id, sym};
use super::tree::{Atom, Base, FUNCS, Row};

/// Weight of the baseline-position mismatch (per x-height, squared).
const W_POS: f32 = 3.0;
/// Weight of the size mismatch (log ratio, squared).
const W_SIZE: f32 = 4.0;
/// Reward for reading a letter run as a function name.
const FUNC_BONUS: f32 = 3.0;
/// Candidates per symbol considered by the decoder.
const MAX_STATES: usize = 8;

#[derive(Clone, Copy, PartialEq)]
enum K {
    Start,
    End,
    /// A fraction or radical: behaves like an operand.
    Comp,
    C(Cat, SymId),
}

fn is_minus(s: SymId) -> bool {
    s == id("−") || s == id("±")
}

/// Cost of category `b` following `a`.
fn trans(a: K, b: K) -> f32 {
    use Cat::*;
    let norm = |k: K| match k {
        K::Comp => K::C(Letter, id("x")),
        k => k,
    };
    match (norm(a), norm(b)) {
        (K::Start, K::End) => 0.0,
        (K::Start, K::C(c, s)) => match c {
            Op if is_minus(s) => 0.3,
            Op => 2.0,
            Rel => 2.5,
            Close => 2.0,
            Dot => 1.5,
            Comma => 2.5,
            Mark => 0.3,
            _ => 0.0,
        },
        (K::C(c, s), K::End) => match c {
            Op | Rel | Open => 2.5,
            Comma => 1.5,
            // An integral needs an integrand.
            Big if s == id("∫") => 2.0,
            _ => 0.0,
        },
        (K::C(a, _), K::C(b, sb)) => match (a, b) {
            (Op, Op) => {
                if is_minus(sb) {
                    1.0
                } else {
                    2.5
                }
            }
            (Op, Rel) | (Rel, Rel) | (Open, Rel) | (Rel, Close) | (Op, Close) => 2.5,
            (Rel, Op) | (Open, Op) => {
                if is_minus(sb) {
                    0.3
                } else {
                    1.5
                }
            }
            (Open, Close) => 1.5,
            (Letter | Upper | Greek, Digit) => 1.2,
            (Close, Digit) => 0.6,
            (Dot | Comma, Dot | Comma) => 2.0,
            (Letter | Upper | Greek | Close, Dot) => 1.0,
            (Dot, Letter | Upper | Greek | Open) => 1.0,
            (Dot | Comma, Op | Rel) => 1.0,
            (Op | Rel, Dot | Comma) => 1.5,
            (_, Mark) | (Mark, _) => 1.0,
            _ => 0.0,
        },
        _ => 0.0,
    }
}

/// Baseline and x-height of a row, from its symbols' current labels.
struct RowGeom {
    base: f32,
    xh: f32,
    /// Enough symbols to judge size and position.
    ok: bool,
}

fn percentile(v: &mut [f32], num: usize, den: usize) -> f32 {
    v.sort_by(f32::total_cmp);
    v[(v.len() * num) / den]
}

fn row_geom(row: &LRow, syms: &[SymNode], labels: &[SymId]) -> RowGeom {
    let (mut bases, mut xhs) = (Vec::new(), Vec::new());
    for a in row {
        if let LBase::Sym(i) = a.base {
            let b = syms[i].bbox;
            match sym(labels[i]).profile {
                Profile::XHeight => {
                    bases.push(b.y1);
                    xhs.push(b.h());
                }
                Profile::Ascender => {
                    bases.push(b.y1);
                    xhs.push(0.6 * b.h());
                }
                Profile::Descender => {
                    bases.push(b.y0 + 0.6 * b.h());
                    xhs.push(0.6 * b.h());
                }
                Profile::Both => {
                    bases.push(b.y0 + 0.75 * b.h());
                    xhs.push(0.5 * b.h());
                }
                _ => {}
            }
        }
    }
    if bases.len() < 2 {
        return RowGeom {
            base: 0.0,
            xh: 1.0,
            ok: false,
        };
    }
    RowGeom {
        base: percentile(&mut bases, 1, 2),
        xh: percentile(&mut xhs, 2, 3).max(1e-3),
        ok: true,
    }
}

/// How badly symbol `s` fits a box at this place on the row.
fn geom_prior(s: SymId, b: BBox, g: &RowGeom) -> f32 {
    if !g.ok {
        return 0.0;
    }
    let xh = g.xh;
    let h = b.h().max(0.05 * xh);
    let axis = g.base - 0.5 * xh;
    let (pos, size) = match sym(s).profile {
        Profile::XHeight => ((b.y1 - g.base) / xh, (h / xh).ln()),
        Profile::Ascender => ((b.y1 - g.base) / xh, (h / (1.5 * xh)).ln()),
        Profile::Descender => ((b.y0 - (g.base - xh)) / xh, (h / (1.5 * xh)).ln()),
        Profile::Both => ((b.y0 + 0.75 * h - g.base) / xh, (h / (2.0 * xh)).ln()),
        Profile::Center => ((b.cy() - axis) / xh, 0.0),
        Profile::Tall => (0.5 * (b.cy() - axis) / xh, 0.5 * (h / (2.0 * xh)).ln()),
        Profile::Dot => {
            if s == id("·") {
                ((b.cy() - axis) / xh, 0.0)
            } else if s == id(",") {
                ((b.y0 - (g.base - 0.2 * xh)) / xh, 0.0)
            } else {
                ((b.cy() - (g.base - 0.1 * xh)) / xh, 0.0)
            }
        }
        Profile::High => (((b.y1 - (g.base - 0.9 * xh)) / xh).max(0.0), 0.0),
    };
    W_POS * pos * pos + W_SIZE * size * size
}

/// Labels for every symbol, and the decoding cost.
pub struct Decoded {
    pub labels: Vec<SymId>,
    pub cost: f32,
}

/// Decode the whole tree. `force` pins one symbol to one label (n-best).
pub fn decode(lrow: &LRow, syms: &[SymNode], force: Option<(usize, SymId)>) -> Decoded {
    let mut labels: Vec<SymId> = syms.iter().map(|s| s.label).collect();
    let cost = decode_row(lrow, syms, force, &mut labels);
    Decoded { labels, cost }
}

/// Labels a symbol could take besides its decoded one (for n-best).
pub fn alternatives(
    lrow: &LRow,
    syms: &[SymNode],
    labels: &[SymId],
    out: &mut Vec<(usize, SymId)>,
) {
    for a in lrow {
        match &a.base {
            LBase::Sym(i) => {
                let mut n = 0;
                for g in &syms[*i].group.guesses {
                    if g.sym != labels[*i] && g.sym != id("√") && n < 2 {
                        out.push((*i, g.sym));
                        n += 1;
                    }
                }
            }
            LBase::Frac(_, num, den) => {
                alternatives(num, syms, labels, out);
                alternatives(den, syms, labels, out);
            }
            LBase::Sqrt(_, body) => alternatives(body, syms, labels, out),
        }
        alternatives(&a.sub, syms, labels, out);
        alternatives(&a.sup, syms, labels, out);
    }
}

fn decode_row(
    row: &LRow,
    syms: &[SymNode],
    force: Option<(usize, SymId)>,
    labels: &mut [SymId],
) -> f32 {
    let mut total = 0.0;
    // Children first: they don't depend on this row's labels.
    for a in row {
        match &a.base {
            LBase::Frac(_, n, d) => {
                total += decode_row(n, syms, force, labels);
                total += decode_row(d, syms, force, labels);
            }
            LBase::Sqrt(_, b) => total += decode_row(b, syms, force, labels),
            LBase::Sym(_) => {}
        }
        total += decode_row(&a.sub, syms, force, labels);
        total += decode_row(&a.sup, syms, force, labels);
    }
    if row.is_empty() {
        return total;
    }
    let g = row_geom(row, syms, labels);
    // Chemistry: a capital other than the round twins makes O likelier than 0.
    let chem = row.iter().any(|a| match a.base {
        LBase::Sym(i) => {
            let s = sym(syms[i].label);
            s.cat == Cat::Upper && !matches!(s.name, "O" | "C" | "S" | "V" | "P")
        }
        _ => false,
    });
    let (o_up, zero) = (id("O"), id("0"));
    let mut states: Vec<Vec<(K, f32, Option<SymId>)>> = Vec::with_capacity(row.len());
    for a in row {
        match a.base {
            LBase::Sym(i) => {
                let node = &syms[i];
                let mut st = Vec::new();
                for gs in node.group.guesses.iter().take(MAX_STATES) {
                    if gs.sym == id("√") {
                        continue;
                    }
                    if let Some((fi, fs)) = force
                        && fi == i
                        && fs != gs.sym
                    {
                        continue;
                    }
                    let s = sym(gs.sym);
                    let mut e = gs.dist / TAU + s.prior + geom_prior(gs.sym, node.bbox, &g);
                    if chem && gs.sym == o_up {
                        e -= 0.6;
                    } else if chem && gs.sym == zero {
                        e += 0.3;
                    }
                    st.push((K::C(s.cat, gs.sym), e, Some(gs.sym)));
                }
                if st.is_empty() {
                    let l = labels[i];
                    st.push((K::C(sym(l).cat, l), 0.0, Some(l)));
                }
                states.push(st);
            }
            _ => states.push(vec![(K::Comp, 0.0, None)]),
        }
    }
    // Viterbi.
    let n = states.len();
    let mut cost: Vec<Vec<f32>> = Vec::with_capacity(n);
    let mut back: Vec<Vec<usize>> = Vec::with_capacity(n);
    cost.push(
        states[0]
            .iter()
            .map(|(k, e, _)| trans(K::Start, *k) + e)
            .collect(),
    );
    back.push(vec![0; states[0].len()]);
    for p in 1..n {
        let mut cp = Vec::with_capacity(states[p].len());
        let mut bp = Vec::with_capacity(states[p].len());
        for (k, e, _) in &states[p] {
            let (mut best, mut arg) = (f32::INFINITY, 0);
            for (j, (kj, _, _)) in states[p - 1].iter().enumerate() {
                let c = cost[p - 1][j] + trans(*kj, *k);
                if c < best {
                    best = c;
                    arg = j;
                }
            }
            cp.push(best + e);
            bp.push(arg);
        }
        cost.push(cp);
        back.push(bp);
    }
    let (mut best, mut arg) = (f32::INFINITY, 0);
    for (j, (k, _, _)) in states[n - 1].iter().enumerate() {
        let c = cost[n - 1][j] + trans(*k, K::End);
        if c < best {
            best = c;
            arg = j;
        }
    }
    for p in (0..n).rev() {
        if let (LBase::Sym(i), Some(l)) = (&row[p].base, states[p][arg].2) {
            labels[*i] = l;
        }
        arg = back[p][arg];
    }
    total + best
}

/// What a symbol of the output stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Sym,
    Bar,
    Radical,
}

/// The output tree, symbols in reading order, and a cost adjustment from
/// the fixed rules (function names).
pub struct Built {
    pub row: Row,
    pub order: Vec<(usize, Role)>,
    pub cost_adj: f32,
}

pub fn build(lrow: &LRow, syms: &[SymNode], labels: &mut [SymId]) -> Built {
    let mut b = Built {
        row: Vec::new(),
        order: Vec::new(),
        cost_adj: 0.0,
    };
    b.row = build_row(lrow, syms, labels, &mut b.order, &mut b.cost_adj);
    b
}

fn emission(syms: &[SymNode], i: usize, s: SymId) -> Option<f32> {
    syms[i]
        .group
        .guesses
        .iter()
        .find(|g| g.sym == s)
        .map(|g| g.dist / TAU + sym(s).prior)
}

fn build_row(
    lrow: &LRow,
    syms: &[SymNode],
    labels: &mut [SymId],
    order: &mut Vec<(usize, Role)>,
    adj: &mut f32,
) -> Row {
    let mut out: Row = Vec::new();
    let mut p = 0;
    while p < lrow.len() {
        if let Some((f, len, extra)) = function_at(lrow, p, syms, labels) {
            for (k, ch) in f.chars().enumerate() {
                if let LBase::Sym(i) = lrow[p + k].base {
                    let mut buf = [0u8; 4];
                    labels[i] = id(ch.encode_utf8(&mut buf));
                    order.push((i, Role::Sym));
                }
            }
            *adj += extra - FUNC_BONUS;
            let last = &lrow[p + len - 1];
            let mut atom = Atom::new(Base::Func(f));
            atom.sub = build_row(&last.sub, syms, labels, order, adj);
            atom.sup = build_row(&last.sup, syms, labels, order, adj);
            out.push(atom);
            p += len;
            continue;
        }
        let a = &lrow[p];
        let base = match &a.base {
            LBase::Sym(i) => {
                order.push((*i, Role::Sym));
                Base::Sym(labels[*i])
            }
            LBase::Frac(bar, n, d) => {
                let n = build_row(n, syms, labels, order, adj);
                order.push((*bar, Role::Bar));
                let d = build_row(d, syms, labels, order, adj);
                Base::Frac(n, d)
            }
            LBase::Sqrt(r, body) => {
                order.push((*r, Role::Radical));
                Base::Sqrt(build_row(body, syms, labels, order, adj))
            }
        };
        let mut atom = Atom::new(base);
        atom.sub = build_row(&a.sub, syms, labels, order, adj);
        // Degree signs and primes: a lone tiny raised mark.
        if let (LBase::Sym(bi), [only]) = (&a.base, a.sup.as_slice())
            && let LBase::Sym(si) = only.base
            && only.sub.is_empty()
            && only.sup.is_empty()
        {
            let (bh, sh) = (syms[*bi].bbox.h(), syms[si].bbox.h());
            let name = sym(labels[si]).name;
            let after_number = sym(labels[*bi]).cat == Cat::Digit;
            if matches!(name, "o" | "O" | "0" | "°") && sh < 0.6 * bh {
                labels[si] = id("°");
            } else if matches!(name, "1" | "l" | "|" | "′" | "," | "/") && sh < 0.7 * bh {
                labels[si] = id("′");
            } else if syms[si].group.dot {
                // A raised dot: a tiny degree sign after a number, else a prime.
                labels[si] = id(if after_number { "°" } else { "′" });
            }
        }
        atom.sup = build_row(&a.sup, syms, labels, order, adj);
        out.push(atom);
        p += 1;
    }
    out
}

/// A function name spelled by the symbols at `p`: (name, length, extra cost
/// of relabelling them).
fn function_at(
    lrow: &LRow,
    p: usize,
    syms: &[SymNode],
    labels: &[SymId],
) -> Option<(&'static str, usize, f32)> {
    let mut best: Option<(&'static str, usize, f32)> = None;
    'funcs: for f in FUNCS {
        let len = f.len();
        if p + len > lrow.len() {
            continue;
        }
        let mut extra = 0.0;
        for (k, ch) in f.chars().enumerate() {
            let a = &lrow[p + k];
            let LBase::Sym(i) = a.base else {
                continue 'funcs;
            };
            if k + 1 < len && (!a.sub.is_empty() || !a.sup.is_empty()) {
                continue 'funcs;
            }
            let mut buf = [0u8; 4];
            let want = id(ch.encode_utf8(&mut buf));
            let (Some(e_want), Some(e_have)) =
                (emission(syms, i, want), emission(syms, i, labels[i]))
            else {
                continue 'funcs;
            };
            extra += (e_want - e_have).max(0.0);
        }
        if extra < FUNC_BONUS && best.is_none_or(|b| extra < b.2) {
            best = Some((f, len, extra));
        }
    }
    best
}
