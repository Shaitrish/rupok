//! Expression tree, its LaTeX form, and a parser for the LaTeX subset the
//! recognizer produces (used by the synthesiser and the tests).

use super::symbols::{SymId, find, id, sym};

/// Function names written as letters and typeset upright (`\sin`).
pub const FUNCS: &[&str] = &["sin", "cos", "tan", "log", "ln"];

#[derive(Clone, Debug, PartialEq)]
pub enum Base {
    Sym(SymId),
    /// One of [`FUNCS`].
    Func(&'static str),
    Frac(Row, Row),
    Sqrt(Row),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Atom {
    pub base: Base,
    pub sub: Row,
    pub sup: Row,
}

impl Atom {
    pub fn new(base: Base) -> Atom {
        Atom {
            base,
            sub: Vec::new(),
            sup: Vec::new(),
        }
    }
}

pub type Row = Vec<Atom>;

/// LaTeX for a row, in the canonical form used throughout: scripts without
/// braces when they are one token (`x^2`, `30^\circ`), primes as `'`, a
/// space after a command only when a letter or digit follows.
pub fn to_latex(row: &[Atom]) -> String {
    let mut toks: Vec<String> = Vec::new();
    for a in row {
        let mut t = match &a.base {
            Base::Sym(s) => {
                let l = sym(*s).latex;
                if l == "\\sqrt" {
                    "\\sqrt{}".to_string()
                } else {
                    l.to_string()
                }
            }
            Base::Func(f) => format!("\\{f}"),
            Base::Frac(n, d) => format!("\\frac{{{}}}{{{}}}", to_latex(n), to_latex(d)),
            Base::Sqrt(b) => format!("\\sqrt{{{}}}", to_latex(b)),
        };
        if !a.sub.is_empty() {
            t.push('_');
            t.push_str(&group(&a.sub));
        }
        if !a.sup.is_empty() {
            let primes = a
                .sup
                .iter()
                .all(|x| x.base == Base::Sym(id("′")) && x.sub.is_empty() && x.sup.is_empty());
            if primes {
                t.push_str(&"'".repeat(a.sup.len()));
            } else {
                t.push('^');
                t.push_str(&group(&a.sup));
            }
        }
        toks.push(t);
    }
    join(&toks)
}

fn group(row: &[Atom]) -> String {
    let s = to_latex(row);
    let single = row.len() == 1
        && row[0].sub.is_empty()
        && row[0].sup.is_empty()
        && matches!(row[0].base, Base::Sym(_))
        && (s.chars().count() == 1 || is_command_word(&s));
    if single { s } else { format!("{{{s}}}") }
}

fn is_command_word(s: &str) -> bool {
    s.len() > 1 && s.starts_with('\\') && s[1..].chars().all(|c| c.is_ascii_alphabetic())
}

/// Whether `s` ends in a control word (so a following letter needs a space).
fn ends_with_command(s: &str) -> bool {
    match s.rfind('\\') {
        Some(i) => {
            let tail = &s[i + 1..];
            !tail.is_empty() && tail.chars().all(|c| c.is_ascii_alphabetic())
        }
        None => false,
    }
}

fn join(toks: &[String]) -> String {
    let mut out = String::new();
    for t in toks {
        if ends_with_command(&out) && t.starts_with(|c: char| c.is_ascii_alphanumeric()) {
            out.push(' ');
        }
        out.push_str(t);
    }
    out
}

/// Every symbol in the row, depth first, in reading order.
pub fn symbols(row: &[Atom], out: &mut Vec<SymId>) {
    for a in row {
        match &a.base {
            Base::Sym(s) => out.push(*s),
            Base::Func(f) => {
                for c in f.chars() {
                    let mut b = [0u8; 4];
                    out.push(id(c.encode_utf8(&mut b)));
                }
            }
            Base::Frac(n, d) => {
                symbols(n, out);
                out.push(id("−"));
                symbols(d, out);
            }
            Base::Sqrt(b) => {
                out.push(id("√"));
                symbols(b, out);
            }
        }
        symbols(&a.sub, out);
        symbols(&a.sup, out);
    }
}

/// Parse the LaTeX subset: symbols, `^`, `_`, `'`, `{…}`, `\frac`,
/// `\sqrt`, the [`FUNCS`] and the symbol commands (`\times`, `\theta`…).
pub fn parse_latex(src: &str) -> Result<Row, String> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, at: 0 };
    let row = p.row()?;
    if p.at < p.toks.len() {
        return Err(format!("unexpected {:?}", p.toks[p.at]));
    }
    Ok(row)
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Open,
    Close,
    Sup,
    Sub,
    Prime,
    Cmd(String),
    Char(String),
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let mut it = src.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            ' ' | '\t' | '\n' => {}
            '{' => out.push(Tok::Open),
            '}' => out.push(Tok::Close),
            '^' => out.push(Tok::Sup),
            '_' => out.push(Tok::Sub),
            '\'' => out.push(Tok::Prime),
            '\\' => {
                let mut name = String::new();
                while let Some(&n) = it.peek() {
                    if n.is_ascii_alphabetic() {
                        name.push(n);
                        it.next();
                    } else {
                        break;
                    }
                }
                if name.is_empty() {
                    // `\,` `\;` `\ ` spacing: ignore.
                    it.next();
                } else {
                    out.push(Tok::Cmd(name));
                }
            }
            c => out.push(Tok::Char(c.to_string())),
        }
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Tok>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }

    fn row(&mut self) -> Result<Row, String> {
        let mut row: Row = Vec::new();
        while let Some(t) = self.peek().cloned() {
            match t {
                Tok::Close => break,
                Tok::Sup | Tok::Sub => {
                    self.at += 1;
                    let g = self.group()?;
                    let last = row.last_mut().ok_or("script without a base")?;
                    let slot = if t == Tok::Sup {
                        &mut last.sup
                    } else {
                        &mut last.sub
                    };
                    if !slot.is_empty() {
                        return Err("double script".into());
                    }
                    *slot = g;
                }
                Tok::Prime => {
                    self.at += 1;
                    let last = row.last_mut().ok_or("prime without a base")?;
                    last.sup.push(Atom::new(Base::Sym(id("′"))));
                }
                Tok::Open => {
                    self.at += 1;
                    let inner = self.row()?;
                    self.expect_close()?;
                    row.extend(inner);
                }
                _ => {
                    let a = self.atom()?;
                    row.push(a);
                }
            }
        }
        Ok(row)
    }

    fn expect_close(&mut self) -> Result<(), String> {
        if self.peek() == Some(&Tok::Close) {
            self.at += 1;
            Ok(())
        } else {
            Err("missing }".into())
        }
    }

    fn group(&mut self) -> Result<Row, String> {
        match self.peek() {
            Some(Tok::Open) => {
                self.at += 1;
                let r = self.row()?;
                self.expect_close()?;
                Ok(r)
            }
            Some(_) => Ok(vec![self.atom()?]),
            None => Err("missing group".into()),
        }
    }

    fn atom(&mut self) -> Result<Atom, String> {
        let t = self.peek().cloned().ok_or("unexpected end")?;
        self.at += 1;
        match t {
            Tok::Cmd(c) => match c.as_str() {
                "frac" => {
                    let n = self.group()?;
                    let d = self.group()?;
                    Ok(Atom::new(Base::Frac(n, d)))
                }
                "sqrt" => Ok(Atom::new(Base::Sqrt(self.group()?))),
                "left" | "right" => self.atom(),
                name => {
                    if let Some(f) = FUNCS.iter().find(|f| **f == name) {
                        return Ok(Atom::new(Base::Func(f)));
                    }
                    let s = if name == "circ" || name == "degree" {
                        Some(id("°"))
                    } else {
                        find(&format!("\\{name}"))
                    };
                    s.map(|s| Atom::new(Base::Sym(s)))
                        .ok_or_else(|| format!("unknown command \\{name}"))
                }
            },
            Tok::Char(c) => find(&c)
                .map(|s| Atom::new(Base::Sym(s)))
                .ok_or_else(|| format!("unknown symbol {c:?}")),
            other => Err(format!("unexpected {other:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(s: &str) -> String {
        to_latex(&parse_latex(s).unwrap())
    }

    #[test]
    fn canonical_round_trips() {
        for (src, want) in [
            ("x^2", "x^2"),
            ("x^{2}", "x^2"),
            ("a^2+b^2=c^2", "a^2+b^2=c^2"),
            ("\\frac{1}{2}", "\\frac{1}{2}"),
            ("\\frac{a+b}{c}", "\\frac{a+b}{c}"),
            ("\\sqrt{x+1}", "\\sqrt{x+1}"),
            ("s=ut+\\frac{1}{2}at^2", "s=ut+\\frac{1}{2}at^2"),
            ("H_2O", "H_2O"),
            ("30^\\circ", "30^\\circ"),
            ("\\sin\\theta", "\\sin\\theta"),
            ("\\sin x", "\\sin x"),
            ("x_{n+1}", "x_{n+1}"),
            ("10^{-3}", "10^{-3}"),
            ("2H_2+O_2\\to 2H_2O", "2H_2+O_2\\rightarrow 2H_2O"),
            ("f'(x)", "f'(x)"),
            ("\\pi r^2", "\\pi r^2"),
            ("3\\times 4", "3\\times 4"),
        ] {
            assert_eq!(round(src), want, "{src}");
        }
    }

    #[test]
    fn errors_do_not_panic() {
        for bad in ["^2", "{x", "\\frac{1}", "x^2^3", "\\nope", "#"] {
            assert!(parse_latex(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn symbol_listing() {
        let mut out = Vec::new();
        symbols(&parse_latex("\\frac{a}{\\sin x}").unwrap(), &mut out);
        let names: Vec<&str> = out.iter().map(|s| sym(*s).name).collect();
        assert_eq!(names, ["a", "−", "s", "i", "n", "x"]);
    }
}
