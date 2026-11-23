//! Glyph lookup: the teacher's own calibrated forms first, then the Hershey
//! single-stroke fonts ([`super::font`]), then the recogniser's hand-authored
//! templates for the few symbols the fonts lack.
//!
//! Coordinates are *writing units*: y grows downwards, the baseline is at
//! y = 0, the x-height line at −14, capitals reach −21 and descenders +7
//! (Hershey units shifted so the baseline is 0). A glyph spans x = 0 to its
//! advance, side bearings included.

use std::collections::HashMap;

use super::font::{FUTURAL, GREEK, MATHLOW};
use crate::math::geom::{BBox, Pt};
use crate::math::hershey;

/// Writing units per x-height.
pub const XH: f32 = 14.0;
/// Hershey's baseline, subtracted to put the baseline at y = 0.
const HERSHEY_BASELINE: f32 = 9.0;
/// Side bearing given to glyphs that come without bounds (teacher forms).
const BEARING: f32 = 2.5;

/// One glyph form: pen strokes in writing units, x from 0 to `adv`.
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub strokes: Vec<Vec<Pt>>,
    pub adv: f32,
}

impl Glyph {
    pub fn bbox(&self) -> BBox {
        BBox::of_strokes(self.strokes.iter().map(Vec::as_slice))
    }
}

/// Parse a Hershey record into a [`Glyph`] (bounds from the record).
fn from_hershey(rec: &str) -> Option<Glyph> {
    let b = rec.as_bytes();
    let left = f32::from(*b.get(3)?) - f32::from(b'R');
    let right = f32::from(*b.get(4)?) - f32::from(b'R');
    let strokes = hershey::parse(rec)?
        .into_iter()
        .map(|s| {
            s.into_iter()
                .map(|p| Pt::new(p.x - left, p.y - HERSHEY_BASELINE))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    Some(Glyph {
        strokes: join_touching(strokes),
        adv: right - left,
    })
}

/// Join strokes that meet end to start, as one pen movement would draw them.
fn join_touching(strokes: Vec<Vec<Pt>>) -> Vec<Vec<Pt>> {
    let mut out: Vec<Vec<Pt>> = Vec::with_capacity(strokes.len());
    for s in strokes {
        if let (Some(last), Some(first)) = (out.last_mut(), s.first())
            && last.last().is_some_and(|p| p.dist(*first) < 0.01)
        {
            last.extend_from_slice(&s[1..]);
            continue;
        }
        out.push(s);
    }
    out
}

/// A glyph from the teacher's own sample (already in writing units, see
/// [`GlyphSet::add_user`]): shifted so its ink starts at the left bearing.
fn from_sample(strokes: &[Vec<Pt>]) -> Option<Glyph> {
    let strokes: Vec<Vec<Pt>> = strokes
        .iter()
        .map(|s| {
            s.iter()
                .copied()
                .filter(|p| p.x.is_finite() && p.y.is_finite())
                .collect::<Vec<_>>()
        })
        .filter(|s| !s.is_empty())
        .collect();
    let b = BBox::of_strokes(strokes.iter().map(Vec::as_slice));
    if strokes.is_empty() || !b.x0.is_finite() {
        return None;
    }
    let dx = BEARING - b.x0;
    let strokes = strokes
        .into_iter()
        .map(|s| s.into_iter().map(|p| Pt::new(p.x + dx, p.y)).collect())
        .collect();
    Some(Glyph {
        strokes,
        adv: (b.x1 - b.x0) + 2.0 * BEARING,
    })
}

/// Characters drawn with another character's glyph.
fn alias(c: char) -> char {
    match c {
        '−' | '–' | '—' => '-',
        '∗' => '*',
        '‘' | '’' | '′' => '\'',
        '“' | '”' => '"',
        '⇒' | '⟹' => '→',
        '∼' => '~',
        '⋅' => '·',
        'ϕ' => 'φ',
        'ϵ' => 'ε',
        _ => c,
    }
}

/// Most samples kept per character (as the recogniser keeps).
pub const MAX_USER_SAMPLES: usize = 16;
/// Characters a [`GlyphSet`] holds user samples for.
pub const MAX_USER_CHARS: usize = 512;

/// Every glyph the writer can use. Cheap to build (the fonts parse in well
/// under a millisecond); build once and reuse.
#[derive(Clone, Debug, Default)]
pub struct GlyphSet {
    builtin: HashMap<char, Glyph>,
    user: HashMap<char, Vec<Glyph>>,
}

impl GlyphSet {
    /// The Hershey glyphs (neat hand) and no teacher forms.
    pub fn new() -> Self {
        let mut builtin = HashMap::new();
        for (i, rec) in FUTURAL.iter().enumerate() {
            let c = char::from(32 + i as u8);
            if let Some(g) = from_hershey(rec) {
                builtin.insert(c, g);
            }
        }
        for (c, rec) in GREEK.iter().chain(MATHLOW.iter()) {
            if let Some(g) = from_hershey(rec) {
                builtin.insert(*c, g);
            }
        }
        // ≈: two stacked tildes, made from the tilde.
        if let Some(t) = builtin.get(&'~').cloned() {
            let mut strokes = Vec::new();
            for dy in [-3.0, 3.0] {
                for s in &t.strokes {
                    strokes.push(s.iter().map(|p| Pt::new(p.x, p.y + dy)).collect());
                }
            }
            builtin.insert(
                '≈',
                Glyph {
                    strokes,
                    adv: t.adv,
                },
            );
        }
        // … as three dots.
        if let Some(d) = builtin.get(&'.').cloned() {
            let mut strokes = Vec::new();
            for k in 0..3 {
                for s in &d.strokes {
                    strokes.push(
                        s.iter()
                            .map(|p| Pt::new(p.x + k as f32 * 5.0, p.y))
                            .collect(),
                    );
                }
            }
            builtin.insert(
                '…',
                Glyph {
                    strokes,
                    adv: d.adv + 10.0,
                },
            );
        }
        GlyphSet {
            builtin,
            user: HashMap::new(),
        }
    }

    /// Add the teacher's own form of `c`, in writing units (baseline y = 0,
    /// x-height line y = −14; x anywhere). Returns false when the sample has
    /// no usable points or the set is full.
    pub fn add_user(&mut self, c: char, strokes: &[Vec<Pt>]) -> bool {
        if strokes.len() > 8 || strokes.iter().map(Vec::len).sum::<usize>() > 4_000 {
            return false;
        }
        if !self.user.contains_key(&c) && self.user.len() >= MAX_USER_CHARS {
            return false;
        }
        let Some(g) = from_sample(strokes) else {
            return false;
        };
        let list = self.user.entry(c).or_default();
        if list.len() >= MAX_USER_SAMPLES {
            list.remove(0);
        }
        list.push(g);
        true
    }

    pub fn clear_user(&mut self) {
        self.user.clear();
    }

    /// Characters the teacher has taught.
    pub fn user_chars(&self) -> usize {
        self.user.len()
    }

    /// The teacher's forms of `c` (empty when not taught).
    pub fn user_forms(&self, c: char) -> &[Glyph] {
        self.user.get(&c).map_or(&[], Vec::as_slice)
    }

    /// The built-in (neat) form of `c`, after aliasing.
    pub fn builtin(&self, c: char) -> Option<&Glyph> {
        self.builtin.get(&c).or_else(|| self.builtin.get(&alias(c)))
    }

    /// True when `c` can be written (by the teacher's form or a built-in one).
    pub fn has(&self, c: char) -> bool {
        self.user.contains_key(&c) || self.builtin(c).is_some()
    }
}

/// Round a polyline's curves (Chaikin corner cutting, two passes) while
/// keeping its sharp corners and its end points: Hershey curves are coarse
/// polygons, a pen draws them smooth.
pub fn smooth(pts: &[Pt]) -> Vec<Pt> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    // Split at corners sharper than ~65°.
    let mut pieces: Vec<Vec<Pt>> = vec![vec![pts[0]]];
    for i in 1..pts.len() {
        let p = pts[i];
        if let Some(v) = pieces.last_mut() {
            v.push(p);
        }
        if i + 1 < pts.len() {
            let a = pts[i - 1];
            let n = pts[i + 1];
            let (ux, uy) = (p.x - a.x, p.y - a.y);
            let (vx, vy) = (n.x - p.x, n.y - p.y);
            let lu = ux.hypot(uy);
            let lv = vx.hypot(vy);
            if lu > 0.0 && lv > 0.0 && (ux * vx + uy * vy) / (lu * lv) < 0.42 {
                pieces.push(vec![p]);
            }
        }
    }
    let mut out: Vec<Pt> = Vec::with_capacity(pts.len() * 4);
    for piece in pieces {
        let mut cur = piece;
        for _ in 0..2 {
            if cur.len() < 3 {
                break;
            }
            let mut next = Vec::with_capacity(cur.len() * 2);
            next.push(cur[0]);
            for w in cur.windows(2) {
                next.push(w[0].lerp(w[1], 0.25));
                next.push(w[0].lerp(w[1], 0.75));
            }
            next.push(cur[cur.len() - 1]);
            cur = next;
        }
        if out
            .last()
            .is_some_and(|l: &Pt| cur.first().is_some_and(|f| l.dist(*f) < 1e-4))
        {
            out.extend_from_slice(&cur[1..]);
        } else {
            out.extend_from_slice(&cur);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts_parse_and_sit_on_the_baseline() {
        let g = GlyphSet::new();
        for c in (33u8..127).map(char::from) {
            let gl = g.builtin(c).unwrap_or_else(|| panic!("{c}"));
            assert!(!gl.strokes.is_empty(), "{c}");
            assert!(gl.adv > 0.0, "{c}");
        }
        // x sits on the baseline and reaches the x-height line.
        let b = g.builtin('x').unwrap().bbox();
        assert!((b.y1 - 0.0).abs() < 0.5 && (b.y0 + XH).abs() < 0.5, "{b:?}");
        // a capital reaches −21, g descends.
        assert!((g.builtin('H').unwrap().bbox().y0 + 21.0).abs() < 0.5);
        assert!(g.builtin('g').unwrap().bbox().y1 > 5.0);
        // the space has no ink but an advance
        assert!(g.builtin(' ').is_some_and(|s| s.adv > 0.0));
    }

    #[test]
    fn teacher_forms_take_precedence_and_are_bounded() {
        let mut g = GlyphSet::new();
        assert!(!g.add_user('x', &[]));
        assert!(g.add_user('x', &[vec![Pt::new(10.0, -14.0), Pt::new(20.0, 0.0)]]));
        let f = &g.user_forms('x')[0];
        assert!((f.bbox().x0 - BEARING).abs() < 1e-4);
        for _ in 0..40 {
            g.add_user('x', &[vec![Pt::new(0.0, 0.0), Pt::new(1.0, 1.0)]]);
        }
        assert_eq!(g.user_forms('x').len(), MAX_USER_SAMPLES);
        assert!(!g.add_user('y', &[vec![Pt::new(f32::NAN, 0.0)]]));
        g.clear_user();
        assert_eq!(g.user_chars(), 0);
    }

    #[test]
    fn smoothing_keeps_ends_and_corners() {
        let v = [Pt::new(0.0, 0.0), Pt::new(5.0, 10.0), Pt::new(10.0, 0.0)];
        let s = smooth(&v);
        assert_eq!(s.first(), v.first());
        assert_eq!(s.last(), v.last());
        assert!(s.contains(&Pt::new(5.0, 10.0)), "the V keeps its point");
        let arc: Vec<Pt> = (0..8)
            .map(|i| {
                let a = i as f32 * 0.3;
                Pt::new(a.cos() * 10.0, a.sin() * 10.0)
            })
            .collect();
        assert!(smooth(&arc).len() > arc.len() * 2);
    }
}
