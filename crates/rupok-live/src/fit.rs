//! Fit agent text to the page: every text an agent adds (`add_text`,
//! `replace_items_with_text`) is sized, wrapped and placed so it stays
//! **inside the slide frame**, readable from the back of the room, never
//! running off the page or the window.
//!
//! An agent asks for a spot and maybe a size; the page decides:
//!
//! 1. **Size.** The asked size, else the size of the page's other text, else
//!    [`crate::apply::DEFAULT_TEXT_SIZE`]; never bigger than an eighth of the
//!    page height, and long passages are capped (a paragraph at title size is
//!    what overflows).
//! 2. **Place.** The box is kept inside the frame's safe margin.
//! 3. **Wrap.** Lines wrap at the room left on the page (or the asked width,
//!    if narrower), estimated with per-character widths of the board font.
//! 4. **Too tall?** Shrink step by step down to the projector minimum, then
//!    move up into the free space above, then continue on a new page.
//!
//! The estimate errs wide (the board renders with Inter, `line-height`
//! 1.25), so the real text is never bigger than the box.

use rupok_core::Frame;

/// Space kept free from the slide edges, slide px.
pub const MARGIN: f64 = 48.0;
/// Line height of board text, in ems (`.item.text` in the board's CSS).
pub const LINE_HEIGHT: f64 = 1.25;
/// Largest text size as a fraction of the page height (a title).
const MAX_SIZE_OF_PAGE: f64 = 1.0 / 8.0;

/// Where and how big a text goes.
#[derive(Clone, Debug, PartialEq)]
pub struct TextFit {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub size: f64,
    /// The text that fits on this page; `rest` continues on a new page.
    pub text: String,
    pub rest: Option<String>,
    /// What was changed from the request, for the agent.
    pub notes: Vec<String>,
}

/// Width of one character in ems for the board font (Inter, medium):
/// a deliberately generous estimate.
pub fn char_em(c: char) -> f64 {
    match c {
        ' ' => 0.28,
        'i' | 'l' | 'j' | '.' | ',' | ':' | ';' | '!' | '|' | '\'' | '`' | 'I' => 0.3,
        'f' | 't' | 'r' | '(' | ')' | '[' | ']' | '{' | '}' | '"' | '1' | '-' => 0.42,
        'm' | 'w' | 'M' | 'W' | '@' | '%' | '&' => 0.9,
        'A'..='Z' => 0.7,
        '0'..='9' => 0.6,
        c if c.is_ascii() => 0.57,
        // Emoji and CJK are a full em; Bangla and other scripts are wide.
        c if (c as u32) >= 0x1F000 || ('\u{2E80}'..='\u{9FFF}').contains(&c) => 1.05,
        _ => 0.66,
    }
}

/// Width of `s` in px at `size`.
pub fn text_width(s: &str, size: f64) -> f64 {
    s.chars().map(char_em).sum::<f64>() * size
}

/// Word-wrap `text` at `width` px (explicit newlines kept). Words longer
/// than a line are broken by character.
pub fn wrap(text: &str, size: f64, width: f64) -> Vec<String> {
    let width = width.max(size);
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        let mut lw = 0.0;
        for word in para.split(' ') {
            let ww = text_width(word, size);
            let sp = if line.is_empty() {
                0.0
            } else {
                char_em(' ') * size
            };
            if !line.is_empty() && lw + sp + ww > width {
                out.push(std::mem::take(&mut line));
                lw = 0.0;
            }
            if ww > width {
                // A very long word: break it across lines.
                for c in word.chars() {
                    let cw = char_em(c) * size;
                    if !line.is_empty() && lw + cw > width {
                        out.push(std::mem::take(&mut line));
                        lw = 0.0;
                    }
                    line.push(c);
                    lw += cw;
                }
                continue;
            }
            if !line.is_empty() {
                line.push(' ');
                lw += sp;
            }
            line.push_str(word);
            lw += ww;
        }
        out.push(line);
    }
    out
}

/// Size of the wrapped block: `(width of the longest line, height)`.
fn block(lines: &[String], size: f64) -> (f64, f64) {
    let w = lines
        .iter()
        .map(|l| text_width(l, size))
        .fold(0.0, f64::max);
    (w, lines.len().max(1) as f64 * size * LINE_HEIGHT)
}

/// The largest size worth using for `text` on `frame`: a title may be big,
/// a paragraph may not.
pub fn max_size_for(text: &str, frame: Frame) -> f64 {
    let n = text.chars().count();
    let page_cap = frame.h * MAX_SIZE_OF_PAGE;
    let cap = match n {
        0..=40 => page_cap,
        41..=120 => 64.0,
        121..=400 => 48.0,
        _ => 40.0,
    };
    cap.min(page_cap).max(crate::apply::MIN_PROJECTOR_TEXT)
}

/// Fit `text` asked at (`x`, `y`) with an optional `size` and wrap width
/// `w` onto a page of `frame`. `min_size` is the smallest size it may be
/// shrunk to (the projector minimum, or the asked size when smaller).
/// `above_free` is the top-most y the block may move up to without covering
/// anything (`None`: don't move it).
pub fn fit_text(
    text: &str,
    size: f64,
    x: f64,
    y: f64,
    w: Option<f64>,
    frame: Frame,
    above_free: Option<f64>,
) -> TextFit {
    let mut notes = Vec::new();
    let asked = size;
    let mut size = size.min(max_size_for(text, frame));
    if size < asked {
        notes.push(format!(
            "text size {asked:.0}px is too big for {} characters on a {:.0}×{:.0} page; used {size:.0}px",
            text.chars().count(),
            frame.w,
            frame.h
        ));
    }
    let min_size = crate::apply::MIN_PROJECTOR_TEXT.min(size);
    let inner_w = (frame.w - 2.0 * MARGIN).max(size * 4.0);

    // Keep the start inside the safe area, leaving room for a few words.
    let min_room = (size * 8.0).min(inner_w);
    let nx = x.clamp(MARGIN, (frame.w - MARGIN - min_room).max(MARGIN));
    let ny = y.clamp(MARGIN, (frame.h - MARGIN - size * LINE_HEIGHT).max(MARGIN));
    if (nx - x).abs() > 0.5 || (ny - y).abs() > 0.5 {
        notes.push(format!(
            "moved from ({x:.0}, {y:.0}) to ({nx:.0}, {ny:.0}) to keep it on the page"
        ));
    }
    let (x, mut y) = (nx, ny);

    let room_w = frame.w - MARGIN - x;
    let wrap_w = match w {
        Some(w) if w < room_w => w.max(size * 2.0),
        _ => room_w,
    };
    let fits = |size: f64, y: f64| -> (f64, f64, bool) {
        let (bw, bh) = block(&wrap(text, size, wrap_w), size);
        (bw, bh, y + bh <= frame.h - MARGIN + 0.5)
    };

    let (mut bw, mut bh, mut ok) = fits(size, y);
    // Too tall: first move up into free space above (keeps the asked size).
    if !ok && let Some(top) = above_free {
        let ny = (frame.h - MARGIN - bh).max(top.max(MARGIN));
        if ny < y - 0.5 {
            notes.push(format!("moved up to y {ny:.0} to fit the page"));
            y = ny;
            ok = y + bh <= frame.h - MARGIN + 0.5;
        }
    }
    // Still too tall: shrink, a step at a time, down to the minimum.
    if !ok {
        let start = size;
        while !ok && size > min_size {
            size = (size * 0.9).max(min_size).floor();
            (bw, bh, ok) = fits(size, y);
        }
        if (start - size).abs() > 0.5 {
            notes.push(format!(
                "shrank the text from {start:.0}px to {size:.0}px to fit the page"
            ));
        }
    }
    // Still too tall: what fits stays here, the rest goes on a new page.
    let mut rest = None;
    let mut kept = text.to_string();
    if !ok {
        let per_page = (((frame.h - MARGIN - y) / (size * LINE_HEIGHT)).floor() as usize).max(1);
        let (here, more) = split_lines(text, size, wrap_w, per_page);
        kept = here;
        rest = (!more.trim().is_empty()).then_some(more);
        (bw, bh) = block(&wrap(&kept, size, wrap_w), size);
    }
    let w = (bw + size * 0.25).min(frame.w - MARGIN - x).max(size);
    TextFit {
        x,
        y,
        w,
        h: bh,
        size,
        text: kept,
        rest,
        notes,
    }
}

/// Split `text` at a word boundary so the first part wraps into at most
/// `n` lines: `(first part, the rest)`. Paragraph breaks are kept.
fn split_lines(text: &str, size: f64, width: f64, n: usize) -> (String, String) {
    // Tokens: words, and `None` for a paragraph break.
    let mut tokens: Vec<Option<&str>> = Vec::new();
    for (i, para) in text.split('\n').enumerate() {
        if i > 0 {
            tokens.push(None);
        }
        tokens.extend(para.split(' ').filter(|w| !w.is_empty()).map(Some));
    }
    let render = |toks: &[Option<&str>]| -> String {
        let mut out = String::new();
        for t in toks {
            match t {
                None => out.push('\n'),
                Some(w) => {
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push(' ');
                    }
                    out.push_str(w);
                }
            }
        }
        out
    };
    // The longest prefix of tokens that wraps into n lines (line counts
    // only grow with the prefix: binary search).
    let (mut lo, mut hi) = (1usize, tokens.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if wrap(&render(&tokens[..mid]), size, width).len() <= n {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let cut = lo.min(tokens.len());
    (
        render(&tokens[..cut]).trim_end().to_string(),
        render(&tokens[cut..]).trim_start().to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLIDE: Frame = Frame::SLIDE_16_9;

    fn inside(f: &TextFit) -> bool {
        f.x >= MARGIN - 0.5
            && f.y >= MARGIN - 0.5
            && f.x + f.w <= SLIDE.w - MARGIN + 0.5
            && f.y + f.h <= SLIDE.h - MARGIN + 0.5
    }

    #[test]
    fn short_text_keeps_its_place_and_size() {
        let f = fit_text("Ohm's law", 56.0, 200.0, 120.0, None, SLIDE, None);
        assert_eq!((f.x, f.y, f.size), (200.0, 120.0, 56.0));
        assert!(f.notes.is_empty(), "{:?}", f.notes);
        assert!(f.rest.is_none());
        assert!(inside(&f));
        assert!(f.w < 400.0, "{}", f.w);
    }

    #[test]
    fn long_text_wraps_inside_the_page_at_a_sensible_size() {
        // What an agent sent in class: a paragraph at 96px, near the right.
        let para = "Ohm's law says that the current through a conductor between two points is \
directly proportional to the voltage across the two points, provided the temperature stays the same. \
We write it as V = IR, where R is the resistance in ohms.";
        let f = fit_text(para, 96.0, 1300.0, 700.0, None, SLIDE, Some(MARGIN));
        assert!(inside(&f), "{f:?}");
        assert!(f.size <= 48.0, "paragraph capped: {}", f.size);
        assert!(f.size >= crate::apply::MIN_PROJECTOR_TEXT);
        assert!(f.rest.is_none(), "fits on the page: {f:?}");
        // Every wrapped line fits the box.
        for l in wrap(&f.text, f.size, f.w) {
            assert!(text_width(&l, f.size) <= f.w + 0.5, "{l}");
        }
        assert!(!f.notes.is_empty());
    }

    #[test]
    fn off_page_starts_are_pulled_in() {
        for (x, y) in [(-300.0, -50.0), (5000.0, 3000.0), (1900.0, 1070.0)] {
            let f = fit_text("Answer: 42", 40.0, x, y, None, SLIDE, None);
            assert!(inside(&f), "{x},{y}: {f:?}");
        }
        let f = fit_text("x", 40.0, f64::MAX / 4.0, 0.0, Some(1e9), SLIDE, None);
        assert!(inside(&f));
    }

    #[test]
    fn too_much_text_continues_on_a_new_page() {
        let long: String = (0..1200)
            .map(|i| format!("word{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let f = fit_text(&long, 32.0, 100.0, 100.0, None, SLIDE, Some(MARGIN));
        assert!(inside(&f), "{f:?}");
        let rest = f.rest.as_deref().expect("continues");
        // Nothing is lost: both parts together hold every word, in order.
        let joined = format!("{} {}", f.text, rest);
        let words: Vec<&str> = joined.split_whitespace().collect();
        assert_eq!(words.len(), 1200);
        assert_eq!(words[0], "word0");
        assert_eq!(words[1199], "word1199");
        assert_eq!(f.size, crate::apply::MIN_PROJECTOR_TEXT);
    }

    #[test]
    fn wrap_respects_newlines_and_breaks_long_words() {
        let l = wrap("ab\ncd", 20.0, 1000.0);
        assert_eq!(l, vec!["ab", "cd"]);
        let l = wrap(&"x".repeat(200), 20.0, 300.0);
        assert!(l.len() > 1);
        for s in &l {
            assert!(text_width(s, 20.0) <= 300.0 + 0.01);
        }
        // Bangla and emoji count as wide.
        assert!(text_width("পদার্থবিজ্ঞান", 32.0) > text_width("abcdefghijklm", 32.0));
    }

    #[test]
    fn narrow_width_asked_is_kept_when_it_fits() {
        let f = fit_text(
            "one two three four five six",
            32.0,
            100.0,
            100.0,
            Some(200.0),
            SLIDE,
            None,
        );
        assert!(f.w <= 200.0 + 8.0 + 0.5, "{}", f.w);
        assert!(f.h > 32.0 * LINE_HEIGHT * 1.5, "wrapped onto several lines");
    }
}
