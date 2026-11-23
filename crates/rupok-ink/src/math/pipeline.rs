//! The recognition pipeline: segmentation → layout → context → n-best.

use super::classify::Library;
use super::context::{self, Role};
use super::geom::{BBox, Pt};
use super::layout::{LBase, LRow, Layout, Rel, SymNode};
use super::segment::{self, GroupCache, TAU};
use super::symbols::sym;
use super::tree::display_latex;
use super::{Candidate, RecognizedSymbol};

/// Segmentations kept by the dynamic programme.
const SEG_BEAMS: usize = 4;
/// Segmentations turned into parses.
const SEGS_USED: usize = 3;
/// Cost of reading a close script decision the other way: base + per unit
/// of margin past the threshold.
const DOUBT_BASE: f32 = 0.4;
const DOUBT_COST: f32 = 10.0;
/// Temperature of the softmax over candidate costs.
const CONF_TEMP: f32 = 0.6;
/// Classifier distance at which a symbol's shape quality is 0.5, and the
/// width of the transition.
const Q_MID: f32 = 0.03;
const Q_WIDTH: f32 = 0.006;

fn quality(d: f32) -> f32 {
    1.0 / (1.0 + ((d - Q_MID) / Q_WIDTH).exp())
}

struct Parse {
    latex: String,
    cost: f32,
    seg: usize,
    /// (symbol index, role, label).
    symbols: Vec<(usize, Role, super::SymId)>,
}

/// A subscript's top sits at least this far (× base height) below its
/// base's top, a superscript's bottom this far above the base's bottom;
/// each unit short of it costs [`SCRIPT_SHIFT_COST`].
const SCRIPT_SHIFT: f32 = 0.3;
const SCRIPT_SHIFT_COST: f32 = 8.0;

fn row_box(row: &LRow, syms: &[SymNode]) -> BBox {
    let mut b = BBox::EMPTY;
    for a in row {
        let i = match &a.base {
            LBase::Sym(i) | LBase::Frac(i, ..) | LBase::Sqrt(i, _) => *i,
        };
        b = b.union(syms[i].bbox);
        b = b.union(row_box(&a.sub, syms)).union(row_box(&a.sup, syms));
        if let LBase::Frac(_, n, d) = &a.base {
            b = b.union(row_box(n, syms)).union(row_box(d, syms));
        }
        if let LBase::Sqrt(_, body) = &a.base {
            b = b.union(row_box(body, syms));
        }
    }
    b
}

/// Layout cost: a subscript that is not lowered or a superscript that is
/// not raised is unlikely, whatever the labels say.
fn script_cost(row: &LRow, syms: &[SymNode]) -> f32 {
    let mut c = 0.0;
    for a in row {
        if let LBase::Sym(i) = a.base {
            let b = syms[i].bbox;
            let h = b.h().max(1e-3);
            if !a.sub.is_empty() {
                let lowered = (row_box(&a.sub, syms).y0 - b.y0) / h;
                c += SCRIPT_SHIFT_COST * (SCRIPT_SHIFT - lowered).max(0.0);
            }
            if !a.sup.is_empty() {
                let raised = (b.y1 - row_box(&a.sup, syms).y1) / h;
                c += SCRIPT_SHIFT_COST * (SCRIPT_SHIFT - raised).max(0.0);
            }
        }
        match &a.base {
            LBase::Frac(_, n, d) => c += script_cost(n, syms) + script_cost(d, syms),
            LBase::Sqrt(_, b) => c += script_cost(b, syms),
            LBase::Sym(_) => {}
        }
        c += script_cost(&a.sub, syms) + script_cost(&a.sup, syms);
    }
    c
}

/// Cost of each unmatched bracket in a row (`[x` is likelier `∫x`, and
/// `)(` likelier an `x`).
const BRACKET_COST: f32 = 1.5;

fn bracket_cost(row: &LRow, labels: &[super::SymId]) -> f32 {
    let mut depth: i32 = 0;
    let mut unmatched = 0;
    let mut c = 0.0;
    for a in row {
        match &a.base {
            LBase::Sym(i) => match sym(labels[*i]).cat {
                super::symbols::Cat::Open => depth += 1,
                super::symbols::Cat::Close => {
                    if depth > 0 {
                        depth -= 1;
                    } else {
                        unmatched += 1;
                    }
                }
                _ => {}
            },
            LBase::Frac(_, n, d) => c += bracket_cost(n, labels) + bracket_cost(d, labels),
            LBase::Sqrt(_, b) => c += bracket_cost(b, labels),
        }
        c += bracket_cost(&a.sub, labels) + bracket_cost(&a.sup, labels);
    }
    c + BRACKET_COST * (unmatched + depth.max(0)) as f32
}

fn parse_of(lrow: &LRow, syms: &[SymNode], dec: &context::Decoded, base: f32, seg: usize) -> Parse {
    let mut labels = dec.labels.clone();
    let built = context::build(lrow, syms, &mut labels);
    Parse {
        latex: display_latex(&built.row),
        cost: base
            + dec.cost
            + built.cost_adj
            + script_cost(lrow, syms)
            + bracket_cost(lrow, &labels),
        seg,
        symbols: built
            .order
            .iter()
            .map(|&(i, r)| (i, r, labels[i]))
            .collect(),
    }
}

/// Recognise `raw` strokes (input order = time order = stroke ids).
pub fn run(
    raw: &[Vec<Pt>],
    lib: &Library,
    cache: Option<&mut GroupCache>,
    n_best: usize,
) -> Vec<Candidate> {
    let prep = segment::prepare(raw);
    let segs = segment::segment(&prep, lib, cache, SEG_BEAMS);
    if segs.is_empty() {
        return Vec::new();
    }
    let raw_boxes: Vec<BBox> = raw.iter().map(|s| BBox::of(s)).collect();
    let mut all_syms: Vec<Vec<SymNode>> = Vec::new();
    let mut parses: Vec<Parse> = Vec::new();
    for (si, seg) in segs.iter().take(SEGS_USED).enumerate() {
        let mut syms: Vec<SymNode> = seg
            .groups
            .iter()
            .map(|g| SymNode {
                bbox: g
                    .strokes
                    .iter()
                    .fold(BBox::EMPTY, |b, &s| b.union(prep.boxes[s])),
                label: g.guesses.first().map(|x| x.sym).unwrap_or(0),
                group: g.clone(),
            })
            .collect();
        let base = seg.cost
            - seg
                .groups
                .iter()
                .map(|g| g.guesses.first().map_or(0.0, |x| x.dist) / TAU)
                .sum::<f32>();
        let mut lay = Layout::new(&syms, prep.unit, &[]);
        let mut lrow = lay.run();
        let mut doubts = lay.doubts;
        let mut dec = context::decode(&lrow, &syms, None);
        let changed = syms
            .iter()
            .zip(&dec.labels)
            .any(|(s, &l)| sym(s.label).profile != sym(l).profile);
        if changed {
            for (s, &l) in syms.iter_mut().zip(&dec.labels) {
                s.label = l;
            }
            let mut lay = Layout::new(&syms, prep.unit, &[]);
            lrow = lay.run();
            doubts = lay.doubts;
            dec = context::decode(&lrow, &syms, None);
        }
        parses.push(parse_of(&lrow, &syms, &dec, base, si));
        if si == 0 {
            let mut alts = Vec::new();
            context::alternatives(&lrow, &syms, &dec.labels, &mut alts);
            for (i, s) in alts {
                if sym(s).profile == sym(dec.labels[i]).profile {
                    let d = context::decode(&lrow, &syms, Some((i, s)));
                    parses.push(parse_of(&lrow, &syms, &d, base, si));
                } else {
                    // A label that sits differently on the line (× vs x,
                    // μ vs H) changes the layout: lay out again.
                    let mut alt = syms.clone();
                    for (n, &l) in alt.iter_mut().zip(&dec.labels) {
                        n.label = l;
                    }
                    alt[i].label = s;
                    let l2 = Layout::new(&alt, prep.unit, &[]).run();
                    let d = context::decode(&l2, &alt, Some((i, s)));
                    parses.push(parse_of(&l2, &alt, &d, base, si));
                }
            }
            doubts.sort_by(|a, b| a.margin.total_cmp(&b.margin));
            for dbt in doubts.iter().take(2) {
                let forced: [(usize, usize, Rel); 1] = [(dbt.base, dbt.target, dbt.other)];
                let mut lay2 = Layout::new(&syms, prep.unit, &forced);
                let l2 = lay2.run();
                let d2 = context::decode(&l2, &syms, None);
                let extra = DOUBT_BASE + DOUBT_COST * dbt.margin;
                parses.push(parse_of(&l2, &syms, &d2, base + extra, si));
            }
        }
        all_syms.push(syms);
    }
    parses.sort_by(|a, b| a.cost.total_cmp(&b.cost));
    let mut seen: Vec<String> = Vec::new();
    parses.retain(|p| {
        if seen.contains(&p.latex) {
            false
        } else {
            seen.push(p.latex.clone());
            true
        }
    });
    parses.truncate(n_best.max(1));
    let min = parses[0].cost;
    let weights: Vec<f32> = parses
        .iter()
        .map(|p| (-(p.cost - min) / CONF_TEMP).exp())
        .collect();
    let z: f32 = weights.iter().sum();
    parses
        .iter()
        .zip(&weights)
        .map(|(p, w)| {
            let syms = &all_syms[p.seg];
            let mut log_q = 0.0;
            let symbols: Vec<RecognizedSymbol> = p
                .symbols
                .iter()
                .map(|&(i, role, label)| {
                    let node = &syms[i];
                    let g = &node.group.guesses;
                    let d = g
                        .iter()
                        .find(|x| x.sym == label)
                        .or(g.first())
                        .map_or(1.0, |x| x.dist);
                    let q = if node.group.dot { 1.0 } else { quality(d) };
                    log_q += q.max(1e-6).ln();
                    let e = |s: super::SymId, dist: f32| dist / TAU + sym(s).prior;
                    let mine = e(label, d);
                    let z: f32 = g.iter().map(|x| (mine - e(x.sym, x.dist)).exp()).sum();
                    let share = if z > 0.0 { (1.0 / z).min(1.0) } else { 1.0 };
                    let (name, latex) = match role {
                        Role::Bar => ("─", "\\frac"),
                        Role::Radical => ("√", "\\sqrt"),
                        Role::Sym => (sym(label).name, sym(label).latex),
                    };
                    let mut strokes = node.group.strokes.clone();
                    strokes.sort_unstable();
                    let bb = strokes
                        .iter()
                        .fold(BBox::EMPTY, |b, &s| b.union(raw_boxes[s]));
                    RecognizedSymbol {
                        symbol: name,
                        latex,
                        bbox: [bb.x0, bb.y0, bb.x1, bb.y1],
                        strokes,
                        confidence: if role == Role::Sym { q * share } else { q },
                    }
                })
                .collect();
            let n = symbols.len().max(1) as f32;
            let q = (log_q / n).exp();
            Candidate {
                latex: p.latex.clone(),
                confidence: (w / z) * q,
                symbols,
            }
        })
        .collect()
}
