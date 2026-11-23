//! The symbol set: what each symbol is called, how it is typeset, where it
//! sits relative to the baseline and how many strokes people use for it.

/// Index into [`SYMBOLS`].
pub type SymId = u8;

/// Vertical profile: where a symbol sits relative to the baseline and the
/// x-height. Layout uses it to estimate baselines; context uses it to tell
/// twins such as `c`/`C`, `p`/`P` and `.`/`·` apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// Sits on the baseline, x-height tall: `a c e m n o r s u v w x z`.
    XHeight,
    /// Sits on the baseline, cap-height tall: digits, capitals, `b d h k l t`.
    Ascender,
    /// Hangs below the baseline: `g j p q y μ`.
    Descender,
    /// Centred on the maths axis: `+ − = < >` and friends.
    Center,
    /// Spans the line: `( ) [ ] | / ∫ √`.
    Tall,
    /// A dot: `.` on the baseline, `·` on the axis.
    Dot,
    /// A small raised mark: `′ °`.
    High,
}

/// Grammar category, used by the context model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cat {
    Digit,
    Letter,
    Upper,
    Greek,
    Op,
    Rel,
    Open,
    Close,
    Bar,
    Dot,
    Comma,
    Mark,
    Big,
    Radical,
}

impl Cat {
    /// Something an operator can apply to (a number, a variable, `∞`).
    pub fn is_operand(self) -> bool {
        matches!(
            self,
            Cat::Digit | Cat::Letter | Cat::Upper | Cat::Greek | Cat::Big
        )
    }
}

/// Whether the strokes of a multi-stroke symbol cross each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cross {
    Any,
    /// `+ t f ± ≠`: two strokes cross.
    Needs,
    /// `= ÷ i j ≤ ≥`: no stroke crosses another.
    Forbids,
}

#[derive(Clone, Copy, Debug)]
pub struct Sym {
    /// Unicode text of the symbol, also its identifier (`"x"`, `"×"`, `"√"`).
    pub name: &'static str,
    /// LaTeX for the symbol on its own (`"\\times"`); `\sqrt` takes a body.
    pub latex: &'static str,
    pub profile: Profile,
    pub cat: Cat,
    /// Fewest and most strokes a writer plausibly uses.
    pub strokes: (u8, u8),
    pub cross: Cross,
    /// Rarity cost added when choosing this symbol (0 = common).
    pub prior: f32,
}

const fn s(
    name: &'static str,
    latex: &'static str,
    profile: Profile,
    cat: Cat,
    strokes: (u8, u8),
    cross: Cross,
    prior: f32,
) -> Sym {
    Sym {
        name,
        latex,
        profile,
        cat,
        strokes,
        cross,
        prior,
    }
}

use Cat as C;
use Cross::{Any, Forbids, Needs};
use Profile as P;

/// Every symbol the recognizer knows, in a fixed order (the order is the
/// [`SymId`]).
pub const SYMBOLS: &[Sym] = &[
    // Digits.
    s("0", "0", P::Ascender, C::Digit, (1, 2), Any, 0.0),
    s("1", "1", P::Ascender, C::Digit, (1, 3), Any, 0.0),
    s("2", "2", P::Ascender, C::Digit, (1, 2), Any, 0.0),
    s("3", "3", P::Ascender, C::Digit, (1, 2), Any, 0.0),
    s("4", "4", P::Ascender, C::Digit, (1, 3), Any, 0.0),
    s("5", "5", P::Ascender, C::Digit, (1, 2), Any, 0.0),
    s("6", "6", P::Ascender, C::Digit, (1, 2), Any, 0.0),
    s("7", "7", P::Ascender, C::Digit, (1, 3), Any, 0.0),
    s("8", "8", P::Ascender, C::Digit, (1, 2), Any, 0.0),
    s("9", "9", P::Ascender, C::Digit, (1, 2), Any, 0.0),
    // Lowercase Latin.
    s("a", "a", P::XHeight, C::Letter, (1, 2), Any, 0.0),
    s("b", "b", P::Ascender, C::Letter, (1, 2), Any, 0.0),
    s("c", "c", P::XHeight, C::Letter, (1, 1), Any, 0.0),
    s("d", "d", P::Ascender, C::Letter, (1, 2), Any, 0.0),
    s("e", "e", P::XHeight, C::Letter, (1, 1), Any, 0.0),
    s("f", "f", P::Ascender, C::Letter, (2, 3), Needs, 0.2),
    s("g", "g", P::Descender, C::Letter, (1, 2), Any, 0.2),
    s("h", "h", P::Ascender, C::Letter, (1, 2), Any, 0.2),
    s("i", "i", P::Ascender, C::Letter, (2, 2), Forbids, 0.0),
    s("j", "j", P::Descender, C::Letter, (2, 2), Forbids, 0.2),
    s("k", "k", P::Ascender, C::Letter, (1, 3), Any, 0.0),
    s("l", "l", P::Ascender, C::Letter, (1, 1), Any, 1.2),
    s("m", "m", P::XHeight, C::Letter, (1, 2), Any, 0.0),
    s("n", "n", P::XHeight, C::Letter, (1, 2), Any, 0.0),
    s("o", "o", P::XHeight, C::Letter, (1, 1), Any, 0.6),
    s("p", "p", P::Descender, C::Letter, (1, 2), Any, 0.0),
    s("q", "q", P::Descender, C::Letter, (1, 2), Any, 0.3),
    s("r", "r", P::XHeight, C::Letter, (1, 2), Any, 0.0),
    s("s", "s", P::XHeight, C::Letter, (1, 1), Any, 0.0),
    s("t", "t", P::Ascender, C::Letter, (2, 2), Needs, 0.0),
    s("u", "u", P::XHeight, C::Letter, (1, 2), Any, 0.0),
    s("v", "v", P::XHeight, C::Letter, (1, 2), Any, 0.0),
    s("w", "w", P::XHeight, C::Letter, (1, 4), Any, 0.2),
    s("x", "x", P::XHeight, C::Letter, (1, 2), Any, 0.0),
    s("y", "y", P::Descender, C::Letter, (1, 2), Any, 0.0),
    s("z", "z", P::XHeight, C::Letter, (1, 2), Any, 0.3),
    // Uppercase Latin.
    s("A", "A", P::Ascender, C::Upper, (1, 3), Any, 0.2),
    s("B", "B", P::Ascender, C::Upper, (1, 3), Any, 0.2),
    s("C", "C", P::Ascender, C::Upper, (1, 1), Any, 0.2),
    s("E", "E", P::Ascender, C::Upper, (1, 4), Any, 0.0),
    s("F", "F", P::Ascender, C::Upper, (1, 3), Any, 0.2),
    s("H", "H", P::Ascender, C::Upper, (2, 3), Any, 0.0),
    s("M", "M", P::Ascender, C::Upper, (1, 4), Any, 0.2),
    s("N", "N", P::Ascender, C::Upper, (1, 3), Any, 0.2),
    s("O", "O", P::Ascender, C::Upper, (1, 1), Any, 0.3),
    s("P", "P", P::Ascender, C::Upper, (1, 2), Any, 0.2),
    s("R", "R", P::Ascender, C::Upper, (1, 2), Any, 0.2),
    s("S", "S", P::Ascender, C::Upper, (1, 1), Any, 0.4),
    s("T", "T", P::Ascender, C::Upper, (2, 2), Any, 0.2),
    s("V", "V", P::Ascender, C::Upper, (1, 2), Any, 0.3),
    // Greek.
    s("α", "\\alpha", P::XHeight, C::Greek, (1, 2), Any, 0.3),
    s("β", "\\beta", P::Ascender, C::Greek, (1, 3), Any, 0.3),
    s("θ", "\\theta", P::Ascender, C::Greek, (1, 2), Any, 0.2),
    s("λ", "\\lambda", P::Ascender, C::Greek, (1, 2), Any, 0.3),
    s("μ", "\\mu", P::Descender, C::Greek, (1, 3), Any, 0.3),
    s("π", "\\pi", P::XHeight, C::Greek, (2, 3), Any, 0.2),
    s("Δ", "\\Delta", P::Ascender, C::Greek, (1, 3), Any, 0.3),
    s("Σ", "\\Sigma", P::Ascender, C::Greek, (1, 3), Any, 0.3),
    // Operators.
    s("+", "+", P::Center, C::Op, (2, 2), Needs, 0.0),
    s("−", "-", P::Center, C::Op, (1, 1), Any, 0.0),
    s("×", "\\times", P::Center, C::Op, (1, 2), Any, 0.6),
    s("÷", "\\div", P::Center, C::Op, (3, 3), Forbids, 0.0),
    s("·", "\\cdot", P::Dot, C::Op, (1, 1), Any, 0.3),
    s("±", "\\pm", P::Center, C::Op, (2, 3), Needs, 0.0),
    s("/", "/", P::Tall, C::Op, (1, 1), Any, 0.3),
    // Relations and arrows.
    s("=", "=", P::Center, C::Rel, (2, 2), Forbids, 0.0),
    s("≠", "\\neq", P::Center, C::Rel, (2, 3), Needs, 0.0),
    s("<", "<", P::Center, C::Rel, (1, 1), Any, 0.0),
    s(">", ">", P::Center, C::Rel, (1, 1), Any, 0.0),
    s("≤", "\\leq", P::Center, C::Rel, (2, 2), Forbids, 0.0),
    s("≥", "\\geq", P::Center, C::Rel, (2, 2), Forbids, 0.0),
    s("→", "\\rightarrow", P::Center, C::Rel, (1, 2), Any, 0.0),
    // Delimiters.
    s("(", "(", P::Tall, C::Open, (1, 1), Any, 0.0),
    s(")", ")", P::Tall, C::Close, (1, 1), Any, 0.0),
    s("[", "[", P::Tall, C::Open, (1, 3), Any, 0.2),
    s("]", "]", P::Tall, C::Close, (1, 3), Any, 0.2),
    s("|", "|", P::Tall, C::Bar, (1, 1), Any, 1.0),
    // Punctuation and marks.
    s(".", ".", P::Dot, C::Dot, (1, 1), Any, 0.0),
    s(",", ",", P::Dot, C::Comma, (1, 1), Any, 0.5),
    s("′", "'", P::High, C::Mark, (1, 1), Any, 0.5),
    s("°", "\\circ", P::High, C::Mark, (1, 1), Any, 0.3),
    // Big symbols.
    s("√", "\\sqrt", P::Tall, C::Radical, (1, 2), Any, 0.0),
    s("∫", "\\int", P::Tall, C::Big, (1, 1), Any, 0.2),
    s("∞", "\\infty", P::XHeight, C::Big, (1, 1), Any, 0.2),
];

/// Look a symbol up by its Unicode name (`"×"`), its LaTeX (`"\\times"`) or
/// a few ASCII spellings (`"-"`, `"*"`, `"sqrt"`, `"theta"`).
pub fn find(name: &str) -> Option<SymId> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    if let Some(i) = SYMBOLS.iter().position(|s| s.name == name) {
        return Some(i as SymId);
    }
    let alias = match name {
        "-" => "−",
        "*" | "\\times" => "×",
        "\\cdot" => "·",
        "'" | "\\prime" => "′",
        "\\to" | "->" => "→",
        "<=" => "≤",
        ">=" => "≥",
        "!=" => "≠",
        "+-" => "±",
        other => {
            let bare = other.strip_prefix('\\').unwrap_or(other);
            return SYMBOLS
                .iter()
                .position(|s| s.latex.strip_prefix('\\') == Some(bare) || s.latex == other)
                .map(|i| i as SymId);
        }
    };
    SYMBOLS
        .iter()
        .position(|s| s.name == alias)
        .map(|i| i as SymId)
}

/// The symbol with this exact name. Panics on a typo, so only for names
/// written in this crate's source.
pub(crate) fn id(name: &str) -> SymId {
    match SYMBOLS.iter().position(|s| s.name == name) {
        Some(i) => i as SymId,
        None => panic!("unknown maths symbol {name:?}"),
    }
}

pub(crate) fn sym(id: SymId) -> &'static Sym {
    &SYMBOLS[id as usize]
}

/// Symbols the classifier cannot tell apart by shape alone, plus the cost of
/// choosing the twin instead. Size, position and the grammar pick between
/// them later.
pub(crate) const TWINS: &[(&str, &[&str])] = &[
    ("x", &["×"]),
    ("×", &["x"]),
    (".", &["·"]),
    ("·", &["."]),
    ("o", &["O", "0", "°"]),
    ("0", &["o", "O", "°"]),
    ("O", &["0", "o", "°"]),
    ("°", &["o", "0", "O"]),
    ("c", &["C"]),
    ("C", &["c"]),
    ("s", &["S"]),
    ("S", &["s"]),
    ("v", &["V"]),
    ("V", &["v"]),
    ("p", &["P"]),
    ("P", &["p"]),
    ("1", &["l", "|"]),
    ("l", &["1", "|"]),
    ("|", &["1", "l"]),
    (",", &["′"]),
    ("′", &[","]),
];

/// Twins of a symbol (see [`TWINS`]).
pub(crate) fn twins(id: SymId) -> impl Iterator<Item = SymId> {
    let name = SYMBOLS[id as usize].name;
    TWINS
        .iter()
        .filter(move |(n, _)| *n == name)
        .flat_map(|(_, t)| t.iter().copied().map(self::id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_findable() {
        for (i, a) in SYMBOLS.iter().enumerate() {
            assert_eq!(find(a.name), Some(i as SymId), "{}", a.name);
            assert!(a.strokes.0 >= 1 && a.strokes.0 <= a.strokes.1);
            for b in &SYMBOLS[i + 1..] {
                assert_ne!(a.name, b.name);
            }
        }
        assert!(SYMBOLS.len() < 255);
        assert_eq!(find("\\times"), Some(id("×")));
        assert_eq!(find("theta"), Some(id("θ")));
        assert_eq!(find("-"), Some(id("−")));
        assert_eq!(find("\\sqrt"), Some(id("√")));
        assert_eq!(find("nope"), None);
        assert_eq!(find(""), None);
    }

    #[test]
    fn twins_resolve() {
        for (a, ts) in TWINS {
            id(a);
            for t in *ts {
                id(t);
            }
        }
        assert_eq!(twins(id("x")).collect::<Vec<_>>(), vec![id("×")]);
    }
}
