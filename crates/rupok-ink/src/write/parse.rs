//! A line of text with inline LaTeX → a small layout tree.
//!
//! `$…$` (or `\(…\)`) switches to maths mode, which only changes spacing:
//! spaces typed inside maths are ignored and operators get room on both
//! sides. Commands, scripts and braces work everywhere, so a line an AI
//! writes without dollars (`x^2 + 3x = 10`) still comes out right.
//!
//! Supported: `\frac{a}{b}` (and `\dfrac`, `\tfrac`), `^` and `_` with a
//! single character or a `{group}`, `\sqrt{…}` (`\sqrt[n]{…}` shows `n`
//! small), Greek letters, common operators and relations, `\text{…}`,
//! `\left`/`\right` (dropped), spacing commands, function names (`\sin`…)
//! and Unicode superscript digits (`x²`). Unknown commands are written out
//! by name, so nothing silently disappears.

/// A node of a line.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// One character.
    Char(char),
    /// A function name written upright and tight (`sin`).
    Func(String),
    /// Horizontal space, in x-heights.
    Space(f32),
    Frac(Vec<Node>, Vec<Node>),
    /// A root: index (may be empty) and radicand.
    Sqrt(Vec<Node>, Vec<Node>),
    /// A base with optional superscript and subscript.
    Script {
        base: Box<Node>,
        sup: Option<Vec<Node>>,
        sub: Option<Vec<Node>>,
    },
    Group(Vec<Node>),
    /// A binary operator or relation in maths mode (spaced).
    Op(char),
}

/// Inputs longer than this are cut (one solution line, not a book).
pub const MAX_LINE_CHARS: usize = 400;
/// Deepest nesting of groups, fractions and scripts.
const MAX_DEPTH: usize = 12;

fn symbol(name: &str) -> Option<char> {
    Some(match name {
        "alpha" => 'α',
        "beta" => 'β',
        "gamma" => 'γ',
        "delta" => 'δ',
        "epsilon" | "varepsilon" => 'ε',
        "zeta" => 'ζ',
        "eta" => 'η',
        "theta" | "vartheta" => 'θ',
        "iota" => 'ι',
        "kappa" => 'κ',
        "lambda" => 'λ',
        "mu" => 'μ',
        "nu" => 'ν',
        "xi" => 'ξ',
        "pi" => 'π',
        "rho" => 'ρ',
        "sigma" => 'σ',
        "tau" => 'τ',
        "upsilon" => 'υ',
        "phi" | "varphi" => 'φ',
        "chi" => 'χ',
        "psi" => 'ψ',
        "omega" => 'ω',
        "Gamma" => 'Γ',
        "Delta" => 'Δ',
        "Theta" => 'Θ',
        "Lambda" => 'Λ',
        "Xi" => 'Ξ',
        "Pi" => 'Π',
        "Sigma" => 'Σ',
        "Upsilon" => 'Υ',
        "Phi" => 'Φ',
        "Psi" => 'Ψ',
        "Omega" => 'Ω',
        "times" => '×',
        "div" => '÷',
        "cdot" => '·',
        "pm" => '±',
        "mp" => '∓',
        "le" | "leq" | "leqslant" => '≤',
        "ge" | "geq" | "geqslant" => '≥',
        "ne" | "neq" => '≠',
        "approx" => '≈',
        "equiv" => '≡',
        "sim" => '~',
        "to" | "rightarrow" | "Rightarrow" | "implies" | "longrightarrow" => '→',
        "leftarrow" | "gets" => '←',
        "uparrow" => '↑',
        "downarrow" => '↓',
        "infty" => '∞',
        "circ" | "degree" => '°',
        "partial" => '∂',
        "nabla" => '∇',
        "int" => '∫',
        "angle" => '∠',
        "therefore" => '∴',
        "perp" => '⊥',
        "ldots" | "dots" | "cdots" => '…',
        "lt" => '<',
        "gt" => '>',
        "prime" => '\'',
        "ast" => '*',
        _ => return None,
    })
}

const FUNCS: &[&str] = &[
    "sin", "cos", "tan", "cot", "sec", "csc", "log", "ln", "exp", "lim", "max", "min", "sinh",
    "cosh", "tanh", "arcsin", "arccos", "arctan", "det", "gcd", "lcm", "mod",
];

/// Operators and relations that get space around them in maths mode.
pub fn is_spaced_op(c: char) -> bool {
    matches!(
        c,
        '+' | '-'
            | '−'
            | '='
            | '×'
            | '÷'
            | '±'
            | '∓'
            | '<'
            | '>'
            | '≤'
            | '≥'
            | '≠'
            | '≈'
            | '≡'
            | '→'
            | '←'
            | '·'
    )
}

fn superscript_digit(c: char) -> Option<char> {
    Some(match c {
        '⁰' => '0',
        '¹' => '1',
        '²' => '2',
        '³' => '3',
        '⁴' => '4',
        '⁵' => '5',
        '⁶' => '6',
        '⁷' => '7',
        '⁸' => '8',
        '⁹' => '9',
        '⁺' => '+',
        '⁻' => '-',
        'ⁿ' => 'n',
        _ => return None,
    })
}

fn subscript_digit(c: char) -> Option<char> {
    Some(match c {
        '₀' => '0',
        '₁' => '1',
        '₂' => '2',
        '₃' => '3',
        '₄' => '4',
        '₅' => '5',
        '₆' => '6',
        '₇' => '7',
        '₈' => '8',
        '₉' => '9',
        _ => return None,
    })
}

struct Parser {
    s: Vec<char>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += 1;
        Some(c)
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += 1;
        }
    }

    /// A command name after `\`: letters, or one other character.
    fn command(&mut self) -> String {
        let mut name = String::new();
        // An over-escaped `\\frac` (JSON escaping applied twice) still means `\frac`.
        if self.peek() == Some('\\')
            && self
                .s
                .get(self.i + 1)
                .is_some_and(char::is_ascii_alphabetic)
        {
            self.i += 1;
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_alphabetic() {
                name.push(c);
                self.i += 1;
            } else {
                break;
            }
        }
        if name.is_empty()
            && let Some(c) = self.bump()
        {
            name.push(c);
        }
        name
    }

    /// One argument: `{group}` or a single atom.
    fn arg(&mut self, math: bool, depth: usize) -> Vec<Node> {
        self.skip_spaces();
        if self.eat('{') {
            return self.row(Some('}'), math, depth + 1);
        }
        let mut out = Vec::new();
        if depth < MAX_DEPTH {
            self.atom(&mut out, math, depth + 1, true);
        }
        out
    }

    /// Parse until `end` (consumed) or the end of input.
    fn row(&mut self, end: Option<char>, mut math: bool, depth: usize) -> Vec<Node> {
        let mut out: Vec<Node> = Vec::new();
        while let Some(c) = self.peek() {
            if Some(c) == end {
                self.i += 1;
                break;
            }
            if c == '$' {
                self.i += 1;
                if end.is_none() {
                    math = !math;
                }
                continue;
            }
            if depth >= MAX_DEPTH {
                self.i += 1;
                continue;
            }
            self.atom(&mut out, math, depth, false);
        }
        out
    }

    /// Parse one atom into `out` (scripts attach to the last node).
    fn atom(&mut self, out: &mut Vec<Node>, math: bool, depth: usize, single: bool) {
        let Some(c) = self.bump() else { return };
        match c {
            '^' | '_' => {
                let arg = self.arg(math, depth);
                attach(out, c == '^', arg);
            }
            '{' => {
                let g = self.row(Some('}'), math, depth + 1);
                out.push(Node::Group(g));
            }
            '}' => {}
            '\\' => {
                let name = self.command();
                self.command_node(out, &name, math, depth);
            }
            c if c.is_whitespace() => {
                if !math && !single && !matches!(out.last(), Some(Node::Space(_))) {
                    out.push(Node::Space(0.6));
                }
            }
            '√' => {
                let arg = self.arg(math, depth);
                out.push(Node::Sqrt(Vec::new(), arg));
            }
            c if superscript_digit(c).is_some() => {
                let mut sup = vec![Node::Char(superscript_digit(c).unwrap_or(c))];
                while let Some(n) = self.peek().and_then(superscript_digit) {
                    self.i += 1;
                    sup.push(Node::Char(n));
                }
                attach(out, true, sup);
            }
            c if subscript_digit(c).is_some() => {
                let mut sub = vec![Node::Char(subscript_digit(c).unwrap_or(c))];
                while let Some(n) = self.peek().and_then(subscript_digit) {
                    self.i += 1;
                    sub.push(Node::Char(n));
                }
                attach(out, false, sub);
            }
            c if math && is_spaced_op(c) => {
                // A sign at the start or after another operator is unary: no space.
                let unary = matches!(c, '-' | '−' | '+' | '±')
                    && matches!(out.last(), None | Some(Node::Op(_)) | Some(Node::Char('(')));
                out.push(if unary { Node::Char(c) } else { Node::Op(c) });
            }
            '*' if math => out.push(Node::Op('×')),
            c => out.push(Node::Char(c)),
        }
    }

    fn command_node(&mut self, out: &mut Vec<Node>, name: &str, math: bool, depth: usize) {
        match name {
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                let n = self.arg(math, depth);
                let d = self.arg(math, depth);
                out.push(Node::Frac(n, d));
            }
            "sqrt" => {
                self.skip_spaces();
                let mut index = Vec::new();
                if self.eat('[') {
                    index = self.row(Some(']'), math, depth + 1);
                }
                let r = self.arg(math, depth);
                out.push(Node::Sqrt(index, r));
            }
            "text" | "mathrm" | "textrm" | "textbf" | "mathbf" | "mathit" | "textit"
            | "operatorname" | "mbox" | "boldsymbol" | "mathsf" | "displaystyle" => {
                if name == "displaystyle" {
                    return;
                }
                self.skip_spaces();
                if self.eat('{') {
                    let g = self.row(Some('}'), false, depth + 1);
                    out.push(Node::Group(g));
                }
            }
            "left" | "right" | "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr" => {
                self.skip_spaces();
                match self.peek() {
                    Some('.') => self.i += 1,
                    Some('\\') => {
                        self.i += 1;
                        let n = self.command();
                        self.command_node(out, &n, math, depth);
                    }
                    Some(c) => {
                        self.i += 1;
                        out.push(Node::Char(c));
                    }
                    None => {}
                }
            }
            "," | ":" | ">" => out.push(Node::Space(0.25)),
            ";" => out.push(Node::Space(0.35)),
            " " => out.push(Node::Space(0.5)),
            "!" => {}
            "quad" => out.push(Node::Space(1.2)),
            "qquad" => out.push(Node::Space(2.4)),
            "\\" => out.push(Node::Space(1.0)),
            "{" | "}" | "%" | "$" | "#" | "&" | "_" => {
                out.push(Node::Char(name.chars().next().unwrap_or(' ')));
            }
            n if FUNCS.contains(&n) => out.push(Node::Func(n.to_string())),
            n => match symbol(n) {
                Some(c) if math && is_spaced_op(c) => out.push(Node::Op(c)),
                Some(c) => out.push(Node::Char(c)),
                None => out.push(Node::Func(n.to_string())),
            },
        }
    }
}

/// Attach a script to the last node (or to nothing, written alone).
fn attach(out: &mut Vec<Node>, sup: bool, arg: Vec<Node>) {
    let base = out.pop().unwrap_or(Node::Group(Vec::new()));
    let node = match base {
        Node::Script {
            base,
            sup: s,
            sub: b,
        } if (sup && s.is_none()) || (!sup && b.is_none()) => {
            if sup {
                Node::Script {
                    base,
                    sup: Some(arg),
                    sub: b,
                }
            } else {
                Node::Script {
                    base,
                    sup: s,
                    sub: Some(arg),
                }
            }
        }
        other => {
            let (s, b) = if sup {
                (Some(arg), None)
            } else {
                (None, Some(arg))
            };
            Node::Script {
                base: Box::new(other),
                sup: s,
                sub: b,
            }
        }
    };
    out.push(node);
}

/// Parse one line. Never fails: unknown input is written as it is.
pub fn parse_line(line: &str) -> Vec<Node> {
    // `\(`…`\)` and `\[`…`\]` mean the same as dollars here.
    let line = line
        .replace("\\(", "$")
        .replace("\\)", "$")
        .replace("\\[", "$")
        .replace("\\]", "$");
    let s: Vec<char> = line
        .chars()
        .filter(|c| !c.is_control() || *c == '\t')
        .take(MAX_LINE_CHARS)
        .collect();
    let mut p = Parser { s, i: 0 };
    let mut row = p.row(None, false, 0);
    while matches!(row.last(), Some(Node::Space(_))) {
        row.pop();
    }
    while matches!(row.first(), Some(Node::Space(_))) {
        row.remove(0);
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_with_maths() {
        let r = parse_line("Subtract 3: $2x = 4$");
        assert!(r.contains(&Node::Space(0.6)));
        assert!(r.contains(&Node::Op('=')));
        // spaces inside maths are dropped
        assert!(!r[r.len() - 3..].iter().any(|n| matches!(n, Node::Space(_))));
    }

    #[test]
    fn structures() {
        let r = parse_line("$\\frac{x+1}{2}$");
        assert!(matches!(&r[0], Node::Frac(n, d) if n.len() == 3 && d.len() == 1));
        let r = parse_line("$x^{2}_i$");
        assert!(
            matches!(&r[0], Node::Script { sup: Some(s), sub: Some(b), .. } if s.len() == 1 && b.len() == 1)
        );
        let r = parse_line("x² + y³");
        assert!(matches!(&r[0], Node::Script { sup: Some(_), .. }));
        let r = parse_line("$\\sqrt[3]{27} = 3$");
        assert!(matches!(&r[0], Node::Sqrt(i, b) if i.len() == 1 && b.len() == 2));
        let r = parse_line("$\\\\frac{1}{2}$");
        assert!(
            matches!(&r[0], Node::Frac(..)),
            "over-escaped commands still work"
        );
        let r = parse_line("$\\left( a \\right)$");
        assert_eq!(r, vec![Node::Char('('), Node::Char('a'), Node::Char(')')]);
        let r = parse_line("$\\sin\\theta \\le \\pi$");
        assert_eq!(r[0], Node::Func("sin".into()));
        assert_eq!(r[1], Node::Char('θ'));
        assert_eq!(r[2], Node::Op('≤'));
        // unary minus is not spaced
        let r = parse_line("$-3 + x$");
        assert_eq!(r[0], Node::Char('-'));
    }

    #[test]
    fn hostile_input_never_panics() {
        for s in [
            "",
            "$",
            "\\",
            "^^^",
            "{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{",
            "\\frac",
            "\\sqrt[",
            "}}}}",
            "a_",
            "\u{0}\u{7}",
            "\\left",
        ] {
            let _ = parse_line(s);
        }
        let long = "x^".repeat(5000);
        let _ = parse_line(&long);
        assert!(parse_line(&"a".repeat(10_000)).len() <= MAX_LINE_CHARS);
    }
}
