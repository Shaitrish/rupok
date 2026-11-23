//! Built-in glyphs: Hershey font glyphs plus hand-authored variants for the
//! stroke orders and letter forms people actually use on a board.
//!
//! All glyphs are in Hershey font units (see [`super::hershey`]): y grows
//! downwards, the baseline is at y = 9, the x-height line at −5, capitals
//! reach −12 and descenders 16. Keeping one coordinate frame lets the
//! synthesiser ([`super::synth`]) place glyphs on a baseline directly.

use super::geom::{Pt, path_len};
use super::hershey::{self, GLYPHS};
use super::symbols::{SymId, id};

/// Where a glyph came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Hershey,
    /// Written by hand in this file (alternative forms and stroke orders).
    Authored,
    /// Generated (radicals and arrows of several lengths).
    Procedural,
}

#[derive(Clone, Debug)]
pub struct Glyph {
    pub sym: SymId,
    pub strokes: Vec<Vec<Pt>>,
    pub source: Source,
}

/// Alternative forms, in a tiny path language over font units:
/// `M x y` starts a stroke, `L x y` extends it, `A cx cy rx ry a0 a1` appends
/// an elliptical arc (degrees, 0° = +x, 90° = down) and `;` ends the stroke.
const AUTHORED: &[(&str, &str)] = &[
    // x as two back-to-back arcs ")(", and in one stroke ("\", up, "/").
    ("x", "A -6 2 6 7 -90 90 ; A 6 2 6 7 270 90"),
    ("x", "M -5 -5 L 5 9 L 5 -5 L -5 9"),
    // Open 4 (an L and a crossing stem); 7 with a bar; plain and based 1.
    ("4", "M -4 -12 L -6 3 L 8 3 ; M 4 -7 L 4 9"),
    ("7", "M -7 -12 L 7 -12 L -3 9 ; M -1 -2 L 6 -2"),
    ("1", "M 0 -12 L 0 9"),
    ("1", "M -4 -8 L 0 -12 L 0 9 ; M -4 9 L 4 9"),
    // 5 as a body and a separate top bar; 9 with a straight stem.
    (
        "5",
        "M -5 -12 L -6 -3 L -3 -4 L 1 -4 L 4 -3 L 6 0 L 6 4 L 4 7 L 1 9 L -3 9 L -6 7 ; M -5 -12 L 6 -12",
    ),
    ("9", "A 0 -6 6 6 0 -360 L 6 9"),
    // Round-top 3 (two bowls); the Hershey 3 has a flat top.
    ("3", "A 0 -7.5 5.5 4.5 -160 90 A 0 3 6 6 -90 160"),
    // Straight t; curly y (a u with a tail) and a two-stroke straight y.
    ("t", "M 0 -12 L 0 9 ; M -5 -5 L 5 -5"),
    (
        "y",
        "M -6 -5 L -6 4 L -4 8 L -1 9 L 2 8 L 5 5 L 6 -5 L 6 12 L 4 15 L 1 16 L -2 16 L -5 14",
    ),
    ("y", "M -6 -5 L 0.9 7 ; M 6 -5 L -3 16"),
    // z with a crossbar (tells it from 2).
    ("z", "M -5 -5 L 6 -5 L -5 9 L 6 9 ; M -3 2 L 4 2"),
    // Two-storey a, and the one-stroke single-storey a.
    (
        "a",
        "M -5 -2 L -3 -4 L 0 -5 L 3 -4 L 5 -2 L 6 1 L 6 9 ; M 6 1 L 1 1 L -3 2 L -5 5 L -4 8 L -1 9 L 3 8 L 6 5",
    ),
    (
        "a",
        "M 6 -3 L 3 -5 L -1 -5 L -4 -3 L -6 1 L -6 5 L -4 8 L -1 9 L 2 8 L 5 5 L 6 -5 L 6 9",
    ),
    // Greek: a narrow θ, a handwritten λ, a three-stroke π, a fish α.
    ("θ", "A 0 -1.5 5 10.5 -90 270 ; M -5 -1.5 L 5 -1.5"),
    ("λ", "M -5 -12 L 6 9 ; M 0 -2 L -6 9"),
    ("π", "M -7 -4 L 7 -4 ; M -3 -4 L -4 9 ; M 3 -4 L 3 7 L 5 9"),
    (
        "α",
        "M 7 -5 L 5 0 L 3 5 L 1 8 L -2 9 L -5 8 L -7 4 L -6 -2 L -3 -5 L 0 -4 L 2 0 L 4 5 L 6 8 L 8 9",
    ),
    // Brackets, ≤ ≥, ∫ and a straight prime.
    ("[", "M 3 -16 L -3 -16 L -3 16 L 3 16"),
    ("]", "M -3 -16 L 3 -16 L 3 16 L -3 16"),
    ("≤", "M 6 -9 L -6 -3 L 6 3 ; M -6 8 L 6 8"),
    ("≥", "M -6 -9 L 6 -3 L -6 3 ; M -6 8 L 6 8"),
    (
        "∫",
        "M 6 -14 L 4 -16 L 2 -15 L 1 -12 L -1 12 L -2 15 L -4 16 L -6 14",
    ),
    ("′", "M 2 -13 L -1 -6"),
];

/// Parse the path language of [`AUTHORED`]. Panics on a malformed path
/// (they are constants, checked by the tests).
pub fn parse_path(src: &str) -> Vec<Vec<Pt>> {
    let mut strokes: Vec<Vec<Pt>> = Vec::new();
    let mut cur: Vec<Pt> = Vec::new();
    let toks: Vec<&str> = src.split_whitespace().collect();
    let num = |i: usize| -> f32 {
        toks.get(i)
            .and_then(|t| t.parse().ok())
            .unwrap_or_else(|| panic!("bad number in path {src:?}"))
    };
    let mut i = 0;
    while i < toks.len() {
        match toks[i] {
            "M" => {
                if !cur.is_empty() {
                    strokes.push(std::mem::take(&mut cur));
                }
                cur.push(Pt::new(num(i + 1), num(i + 2)));
                i += 3;
            }
            "L" => {
                cur.push(Pt::new(num(i + 1), num(i + 2)));
                i += 3;
            }
            "A" => {
                let (cx, cy, rx, ry, a0, a1) = (
                    num(i + 1),
                    num(i + 2),
                    num(i + 3),
                    num(i + 4),
                    num(i + 5),
                    num(i + 6),
                );
                let steps = ((a1 - a0).abs() / 15.0).ceil().max(1.0) as usize;
                for k in 0..=steps {
                    let a = (a0 + (a1 - a0) * k as f32 / steps as f32).to_radians();
                    cur.push(Pt::new(cx + rx * a.cos(), cy + ry * a.sin()));
                }
                i += 7;
            }
            ";" => {
                if !cur.is_empty() {
                    strokes.push(std::mem::take(&mut cur));
                }
                i += 1;
            }
            t => panic!("bad token {t:?} in path {src:?}"),
        }
    }
    if !cur.is_empty() {
        strokes.push(cur);
    }
    strokes
}

/// Join strokes whose ends meet (Hershey splits strokes at corners that a
/// pen draws without lifting: `v`, `z`, `Σ`, `M`…). Dots are left alone.
pub fn join_strokes(mut strokes: Vec<Vec<Pt>>) -> Vec<Vec<Pt>> {
    const EPS: f32 = 0.6;
    const DOT: f32 = 3.0;
    let near = |a: Pt, b: Pt| a.dist(b) <= EPS;
    loop {
        let mut joined = None;
        'search: for i in 0..strokes.len() {
            for j in i + 1..strokes.len() {
                let (a, b) = (&strokes[i], &strokes[j]);
                if path_len(a) < DOT || path_len(b) < DOT {
                    continue;
                }
                let (a0, a1, b0, b1) = (a[0], a[a.len() - 1], b[0], b[b.len() - 1]);
                let mut out: Vec<Pt>;
                if near(a1, b0) {
                    out = a.clone();
                    out.extend_from_slice(&b[1..]);
                } else if near(a0, b0) {
                    out = a.iter().rev().copied().collect();
                    out.extend_from_slice(&b[1..]);
                } else if near(a1, b1) {
                    out = a.clone();
                    out.extend(b.iter().rev().skip(1));
                } else if near(a0, b1) {
                    out = b.clone();
                    out.extend_from_slice(&a[1..]);
                } else {
                    continue;
                }
                joined = Some((i, j, out));
                break 'search;
            }
        }
        match joined {
            Some((i, j, out)) => {
                strokes[i] = out;
                strokes.remove(j);
            }
            None => return strokes,
        }
    }
}

/// A radical with an overbar `w` font units long (`w = 0`: just the tick).
fn radical(w: f32, lead_tick: bool, separate_bar: bool) -> Vec<Vec<Pt>> {
    let mut check = Vec::new();
    if lead_tick {
        check.push(Pt::new(-4.0, 1.0));
        check.push(Pt::new(-2.0, -1.0));
    } else {
        check.push(Pt::new(-3.0, 2.0));
    }
    check.push(Pt::new(2.0, 11.0));
    check.push(Pt::new(7.0, -14.0));
    if w <= 0.0 {
        return vec![check];
    }
    let end = Pt::new(7.0 + w, -14.0);
    if separate_bar {
        vec![check, vec![Pt::new(6.0, -14.0), end]]
    } else {
        check.push(end);
        vec![check]
    }
}

/// A right arrow with a shaft `len` font units long.
fn arrow(len: f32, one_stroke: bool) -> Vec<Vec<Pt>> {
    let tip = Pt::new(-9.0 + len, 0.0);
    let up = Pt::new(tip.x - 5.0, -5.0);
    let down = Pt::new(tip.x - 5.0, 5.0);
    if one_stroke {
        vec![vec![Pt::new(-9.0, 0.0), tip, up, tip, down]]
    } else {
        vec![vec![Pt::new(-9.0, 0.0), tip], vec![up, tip, down]]
    }
}

/// Every built-in glyph.
pub fn builtin_glyphs() -> Vec<Glyph> {
    let mut out = Vec::with_capacity(GLYPHS.len() + AUTHORED.len() + 16);
    for g in GLYPHS {
        // Validated by tests; a bad record is skipped rather than panicking.
        if let Some(strokes) = hershey::parse(g.data) {
            out.push(Glyph {
                sym: id(g.sym),
                strokes: join_strokes(strokes),
                source: Source::Hershey,
            });
        }
    }
    for (name, path) in AUTHORED {
        out.push(Glyph {
            sym: id(name),
            strokes: parse_path(path),
            source: Source::Authored,
        });
    }
    let root = id("√");
    for (w, tick, sep) in [
        (0.0, false, false),
        (8.0, true, false),
        (18.0, false, false),
        (18.0, true, true),
        (34.0, false, false),
        (34.0, false, true),
        (56.0, true, false),
        (80.0, false, false),
    ] {
        out.push(Glyph {
            sym: root,
            strokes: radical(w, tick, sep),
            source: Source::Procedural,
        });
    }
    let to = id("→");
    for (len, one) in [(18.0, false), (18.0, true), (30.0, false), (44.0, true)] {
        out.push(Glyph {
            sym: to,
            strokes: arrow(len, one),
            source: Source::Procedural,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::symbols::{SYMBOLS, sym};
    use super::*;

    #[test]
    fn every_symbol_has_a_glyph_or_a_twin() {
        let glyphs = builtin_glyphs();
        for (i, s) in SYMBOLS.iter().enumerate() {
            let own = glyphs.iter().any(|g| g.sym as usize == i);
            let twin = matches!(s.name, "×");
            assert!(own || twin, "no glyph for {}", s.name);
        }
    }

    #[test]
    fn stroke_counts_match_the_symbol_table() {
        for g in builtin_glyphs() {
            let s = sym(g.sym);
            let n = g.strokes.len() as u8;
            assert!(
                n >= s.strokes.0 && n <= s.strokes.2,
                "{} ({:?}) has {n} strokes, table says {:?}",
                s.name,
                g.source,
                s.strokes
            );
            for st in &g.strokes {
                assert!(!st.is_empty());
            }
        }
    }

    #[test]
    fn joins_corners_but_not_dots() {
        let v = GLYPHS.iter().find(|g| g.sym == "v").unwrap();
        assert_eq!(join_strokes(hershey::parse(v.data).unwrap()).len(), 1);
        let i = GLYPHS.iter().find(|g| g.sym == "i").unwrap();
        assert_eq!(join_strokes(hershey::parse(i.data).unwrap()).len(), 2);
        let sigma = GLYPHS.iter().find(|g| g.sym == "Σ").unwrap();
        assert_eq!(join_strokes(hershey::parse(sigma.data).unwrap()).len(), 1);
    }

    #[test]
    fn path_language() {
        let p = parse_path("M 0 0 L 1 2 ; A 0 0 1 1 0 90");
        assert_eq!(p.len(), 2);
        assert_eq!(p[0], vec![Pt::new(0.0, 0.0), Pt::new(1.0, 2.0)]);
        assert_eq!(p[1].len(), 7);
        assert!((p[1][6].y - 1.0).abs() < 1e-5);
    }
}
