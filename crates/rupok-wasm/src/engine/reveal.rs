//! Progressive reveal of AI handwriting (`write_handwriting`): the strokes are
//! already in the document (one undo step), but the renderer only gets each
//! one when the pen reaches it, and the stroke being written is drawn growing
//! in the wet layer, so the class watches the solution being written.
//!
//! Timing comes from the strokes themselves: each stroke's `created` (offset
//! from the first) is when it starts, its points' `dt` how it is drawn. That
//! is also what replay uses, so replay shows the writing the same way.
//!
//! In step mode (Teach, "one line per click") the writing pauses before each
//! line until the teacher presses Next ([`Engine::reveal_next`]).

use rupok_core::{ItemData, ItemId};

use super::{Engine, InkSink, WetStroke};

#[derive(Clone, Debug)]
struct RevealItem {
    id: ItemId,
    /// ms from the start of the writing.
    start: f64,
    dur: f64,
    line: usize,
}

#[derive(Clone, Debug)]
pub struct Reveal {
    items: Vec<RevealItem>,
    /// Writing clock, ms.
    t: f64,
    last_tick: f64,
    /// Strokes handed to the renderer.
    shown: usize,
    /// Lines allowed to be written (step mode).
    gate: usize,
    step: bool,
    lines: usize,
}

impl Reveal {
    /// Where the clock must stop: the start of the first stroke of a line
    /// that is not allowed yet.
    fn limit(&self) -> f64 {
        if !self.step {
            return f64::INFINITY;
        }
        self.items
            .iter()
            .skip(self.shown)
            .find(|i| i.line >= self.gate)
            .map_or(f64::INFINITY, |i| i.start)
    }

    fn waiting(&self) -> bool {
        self.step
            && self
                .items
                .get(self.shown)
                .is_some_and(|i| i.line >= self.gate && self.t >= i.start)
    }
}

/// What the board shows about a reveal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RevealState {
    pub shown: usize,
    pub total: usize,
    /// Step mode and paused before the next line.
    pub waiting: bool,
    /// Lines released so far / lines in all.
    pub gate: usize,
    pub lines: usize,
}

impl<S: InkSink> Engine<S> {
    /// Start revealing strokes `ids` (in writing order) on the current page.
    /// `lines[i]` is stroke i's line; with `step`, only the first line is
    /// written until [`Self::reveal_next`]. Strokes not on this page are skipped.
    pub fn reveal_start(&mut self, ids: &[ItemId], lines: &[usize], step: bool, now_ms: f64) {
        self.reveal_finish();
        let mut items: Vec<RevealItem> = Vec::with_capacity(ids.len());
        for (k, id) in ids.iter().enumerate() {
            let Some(it) = self.scene.get(id) else {
                continue;
            };
            let ItemData::Stroke(s) = &it.data else {
                continue;
            };
            items.push(RevealItem {
                id: id.clone(),
                start: it.created,
                dur: f64::from(s.points.last().map_or(0, |p| p.dt)),
                line: lines.get(k).copied().unwrap_or(0),
            });
        }
        if items.is_empty() || self.replay.is_some() {
            return;
        }
        items.sort_by(|a, b| a.start.total_cmp(&b.start));
        let t0 = items[0].start;
        for i in &mut items {
            i.start -= t0;
            self.sink.remove_stroke(&i.id);
        }
        let n_lines = items.iter().map(|i| i.line + 1).max().unwrap_or(1);
        self.reveal = Some(Reveal {
            items,
            t: 0.0,
            last_tick: now_ms,
            shown: 0,
            gate: 1,
            step,
            lines: n_lines,
        });
        self.bump();
    }

    /// Advance the writing clock and hand finished strokes to the renderer.
    pub(super) fn reveal_tick(&mut self, now_ms: f64) -> bool {
        if self.replay.is_some() {
            return false;
        }
        let Some(r) = &mut self.reveal else {
            return false;
        };
        let dt = (now_ms - r.last_tick).clamp(0.0, 250.0);
        r.last_tick = now_ms;
        r.t = (r.t + dt).min(r.limit());
        let mut done: Vec<ItemId> = Vec::new();
        while let Some(i) = r.items.get(r.shown) {
            if i.start + i.dur > r.t {
                break;
            }
            done.push(i.id.clone());
            r.shown += 1;
        }
        let finished = r.shown >= r.items.len();
        for id in &done {
            if let Some(it) = self.scene.get(id)
                && let ItemData::Stroke(s) = &it.data
            {
                self.sink.add_stroke(id, s);
            }
        }
        if finished {
            self.reveal = None;
            self.bump();
        } else if !done.is_empty() {
            self.dirty = true;
        }
        true
    }

    /// The stroke being written, cut at the writing clock.
    pub(super) fn reveal_partial(&self) -> Option<WetStroke> {
        let r = self.reveal.as_ref()?;
        let i = r.items.get(r.shown)?;
        // Paused before a line (step mode): no pen on the board yet.
        if i.start > r.t || r.waiting() {
            return None;
        }
        let it = self.scene.get(&i.id)?;
        let ItemData::Stroke(s) = &it.data else {
            return None;
        };
        let local = r.t - i.start;
        let pts: Vec<_> = s
            .points
            .iter()
            .take_while(|p| f64::from(p.dt) <= local)
            .map(|p| rupok_ink::StrokePoint {
                x: p.x + s.offset[0],
                y: p.y + s.offset[1],
                ..*p
            })
            .collect();
        (!pts.is_empty()).then_some(WetStroke {
            points: pts,
            width: s.width,
            color: s.color,
            opacity: s.opacity,
        })
    }

    /// True while a reveal is writing (not paused in step mode).
    pub(super) fn reveal_animating(&self) -> bool {
        self.reveal.as_ref().is_some_and(|r| !r.waiting())
    }

    /// Step mode: release the next line. False when nothing is waiting for it.
    pub fn reveal_next(&mut self, now_ms: f64) -> bool {
        let Some(r) = &mut self.reveal else {
            return false;
        };
        if !r.step || r.gate >= r.lines {
            return false;
        }
        r.gate += 1;
        r.last_tick = now_ms;
        self.dirty = true;
        true
    }

    /// Show everything at once and stop revealing.
    pub fn reveal_finish(&mut self) {
        let Some(r) = self.reveal.take() else { return };
        for i in &r.items[r.shown..] {
            if let Some(it) = self.scene.get(&i.id)
                && let ItemData::Stroke(s) = &it.data
            {
                self.sink.add_stroke(&i.id, s);
            }
        }
        self.bump();
    }

    pub fn reveal_state(&self) -> Option<RevealState> {
        self.reveal.as_ref().map(|r| RevealState {
            shown: r.shown,
            total: r.items.len(),
            waiting: r.waiting(),
            gate: r.gate,
            lines: r.lines,
        })
    }
}
