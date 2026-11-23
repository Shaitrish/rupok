//! Handwriting synthesis: text with inline maths → timed pen strokes.
//!
//! The opposite of [`crate::math`]: where the recogniser reads ink, this
//! writes it. An AI solving a problem on the board sends short lines such as
//! `"Subtract 3: $2x = 4$"` or `"$x = \frac{4}{2} = 2$"`; [`write`] turns them
//! into strokes that look handwritten and carry real pen timing, so the board
//! can animate them stroke by stroke and replay shows them the same way.
//!
//! ```text
//! line ─▶ parse (text + inline LaTeX) ─▶ layout (glyphs, fractions, scripts,
//!         roots, wrapping; deterministic variation) ─▶ slant, scale ─▶ pen timing
//! ```
//!
//! - **Glyphs** come from the teacher's own calibrated forms
//!   ([`GlyphSet::add_user`]; the same samples teach the recogniser), else
//!   from the Hershey single-stroke fonts, smoothed (see [`font`] for the
//!   data licence).
//! - **Variation** (slant, size, baseline wobble, rotation, spacing) is
//!   gentle and comes from a seeded generator: the same input and seed always
//!   give the same ink.
//! - **Timing**: slower in curves, eased at pen-down and lift, pauses between
//!   strokes, glyphs, words and lines; `speed` scales it all.
//!
//! Pure Rust with no clock, so it runs in `wasm32-unknown-unknown`.

pub mod font;
pub mod glyphs;
pub mod layout;
pub mod parse;
pub mod pen;

#[cfg(test)]
mod tests;

pub use glyphs::{GlyphSet, XH};
pub use pen::PenStroke;

use crate::math::geom::Pt;
use crate::math::synth::Rng;
use layout::{Layout, Pause, Variation};

/// Most lines one call writes.
pub const MAX_LINES: usize = 40;

/// How to write.
#[derive(Clone, Debug, PartialEq)]
pub struct WriteOptions {
    /// Height of a lowercase `x`, in output units (slide pixels).
    pub x_height: f32,
    /// Forward slant in degrees (positive leans right).
    pub slant_deg: f32,
    /// Natural variation: 0 = machine-perfect, 1 = clearly hand-made.
    pub jitter: f32,
    /// Pen speed multiplier (1 = brisk board writing, about 6 letters a second).
    pub speed: f32,
    /// Picks the variation; same seed, same ink.
    pub seed: u64,
    /// Wrap lines wider than this (output units), at spaces.
    pub max_width: Option<f32>,
    /// Distance between baselines, in x-heights.
    pub line_pitch: f32,
    /// Use the teacher's own glyphs where the set has them.
    pub prefer_user: bool,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions {
            x_height: 22.0,
            slant_deg: 8.0,
            jitter: 0.6,
            speed: 1.0,
            seed: 1,
            max_width: None,
            line_pitch: 3.0,
            prefer_user: true,
        }
    }
}

/// Box of one visual line in output units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineBox {
    /// Input line (solution step) it belongs to.
    pub line: usize,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub baseline: f32,
}

/// Synthesised writing. Coordinates start at (0, 0), the top-left of the
/// block; y grows downwards.
#[derive(Clone, Debug, PartialEq)]
pub struct Written {
    /// Strokes in writing order, with timing.
    pub strokes: Vec<PenStroke>,
    /// One box per visual line (a long input line may wrap into several).
    pub lines: Vec<LineBox>,
    pub width: f32,
    pub height: f32,
    /// When the last stroke ends, ms from the start.
    pub duration_ms: f64,
    /// Characters that had no glyph (written as `?`).
    pub missing: Vec<char>,
}

impl Written {
    /// Start time of each input line's first stroke, ms (`None` for a line
    /// without ink).
    pub fn line_starts(&self, lines: usize) -> Vec<Option<f64>> {
        let mut out = vec![None; lines];
        for s in &self.strokes {
            if let Some(slot) = out.get_mut(s.line)
                && slot.is_none()
            {
                *slot = Some(s.start_ms);
            }
        }
        out
    }
}

fn fnv(s: &str, seed: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ seed;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Write `lines` (text with inline LaTeX, one solution step each).
pub fn write<S: AsRef<str>>(lines: &[S], glyphs: &GlyphSet, opts: &WriteOptions) -> Written {
    let xh = if opts.x_height.is_finite() {
        opts.x_height.clamp(2.0, 400.0)
    } else {
        22.0
    };
    let unit = xh / XH; // output units per writing unit
    let slant = opts.slant_deg.clamp(-30.0, 30.0).to_radians().tan();
    let jitter = if opts.jitter.is_finite() {
        opts.jitter.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let speed = if opts.speed.is_finite() {
        opts.speed
    } else {
        1.0
    };
    let pitch = opts.line_pitch.clamp(1.5, 8.0) * XH;
    let max_w = opts
        .max_width
        .filter(|w| w.is_finite() && *w > xh * 4.0)
        .map(|w| w / unit);

    let joined: String = lines
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join("\n");
    let mut lay = Layout {
        glyphs,
        var: Variation {
            jitter,
            prefer_user: opts.prefer_user,
        },
        rng: Rng::new(fnv(&joined, opts.seed)),
        missing: Vec::new(),
    };

    // Lay out every visual line in writing units, baseline at y = 0.
    let mut rows: Vec<(usize, layout::LBox)> = Vec::new();
    for (li, line) in lines.iter().take(MAX_LINES).enumerate() {
        let nodes = parse::parse_line(line.as_ref());
        let parts = match max_w {
            Some(w) => layout::wrap(&mut lay, &nodes, w),
            None => vec![nodes],
        };
        for part in parts {
            let b = lay.row(&part, 1.0);
            rows.push((li, b));
        }
    }

    // Stack the lines: a baseline every `pitch`, more when a line is tall
    // (fractions), never touching the line above.
    let mut strokes: Vec<(Vec<Pt>, Pause, usize)> = Vec::new();
    let mut boxes = Vec::with_capacity(rows.len());
    let mut baseline = 0.0f32;
    let mut prev_bottom = f32::NEG_INFINITY;
    let mut width = 0.0f32;
    for (k, (li, b)) in rows.into_iter().enumerate() {
        let top = b.top.min(-XH * 1.5);
        if k == 0 {
            baseline = -top;
        } else {
            baseline = (baseline + pitch).max(prev_bottom - top + XH * 0.5);
        }
        let wobble = 0.5 * jitter * lay.rng.normal().clamp(-2.0, 2.0);
        let drift = 0.004 * jitter * lay.rng.normal().clamp(-2.0, 2.0); // baseline tilt
        let mut first = true;
        let mut x1 = 0.0f32;
        for s in b.strokes {
            let pts: Vec<Pt> = s
                .pts
                .iter()
                .map(|p| {
                    let y = p.y + wobble + drift * p.x;
                    // Shear around the baseline: tops lean right.
                    let x = p.x - y * slant;
                    Pt::new(x * unit, (y + baseline) * unit)
                })
                .collect();
            for p in &pts {
                x1 = x1.max(p.x);
            }
            let pause = if first { Pause::Line } else { s.pause };
            first = false;
            strokes.push((pts, pause, li));
        }
        let bottom = b.bottom.max(XH * 0.5) + baseline;
        boxes.push(LineBox {
            line: li,
            x0: 0.0,
            y0: (baseline + top) * unit,
            x1: x1.max(b.w * unit),
            y1: bottom * unit,
            baseline: baseline * unit,
        });
        width = width.max(x1).max(b.w * unit);
        prev_bottom = bottom;
    }

    // Keep everything right of x = 0 and below y = 0 (slant and wobble can
    // push ink a little past the edges).
    let (min_x, min_y) = strokes
        .iter()
        .flat_map(|(p, _, _)| p.iter())
        .fold((0.0f32, 0.0f32), |(a, b), q| (a.min(q.x), b.min(q.y)));
    if min_x < 0.0 || min_y < 0.0 {
        for (p, _, _) in &mut strokes {
            for q in p {
                q.x -= min_x;
                q.y -= min_y;
            }
        }
        for b in &mut boxes {
            b.x1 -= min_x;
            b.y0 -= min_y;
            b.y1 -= min_y;
            b.baseline -= min_y;
        }
        width -= min_x;
    }
    // The block's height covers the ink of the last line too.
    let ink_bottom = strokes
        .iter()
        .flat_map(|(p, _, _)| p.iter().map(|q| q.y))
        .fold(0.0f32, f32::max);
    if let Some(last) = boxes.last_mut() {
        last.y1 = last.y1.max(ink_bottom);
    }

    let timed = pen::time_all(&strokes, xh, speed);
    let duration_ms = timed.last().map_or(0.0, PenStroke::end_ms);
    let height = boxes.last().map_or(0.0, |b| b.y1);
    Written {
        strokes: timed,
        lines: boxes,
        width,
        height,
        duration_ms,
        missing: lay.missing,
    }
}

/// Convert a teacher's sample drawn in a calibration box to writing units:
/// `baseline` and `x_height_line` are the guide lines' y positions in the
/// sample's own coordinates (x-height line above the baseline).
pub fn normalise_sample(strokes: &[Vec<Pt>], baseline: f32, x_height_line: f32) -> Vec<Vec<Pt>> {
    let h = baseline - x_height_line;
    if !h.is_finite() || h <= 0.0 {
        return Vec::new();
    }
    let k = XH / h;
    strokes
        .iter()
        .map(|s| {
            s.iter()
                .filter(|p| p.x.is_finite() && p.y.is_finite())
                .map(|p| Pt::new(p.x * k, (p.y - baseline) * k))
                .collect()
        })
        .collect()
}
