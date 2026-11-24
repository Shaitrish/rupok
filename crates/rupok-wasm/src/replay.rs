//! Step-by-step replay of a page, built from item creation times and per-point
//! timing inside strokes. Idle gaps are compressed so a 2-hour class replays
//! in minutes while each stroke still draws at the teacher's real speed.

use std::collections::HashSet;

use rupok_core::{Item, ItemData};

use crate::engine::{InkSink, WetStroke};

/// Longest pause kept between two actions during replay (ms).
const MAX_GAP_MS: f64 = 900.0;

struct Event {
    start: f64,
    dur: f64,
    item: Item,
}

pub struct Replay {
    events: Vec<Event>,
    pub duration: f64,
    pub t: f64,
    /// Events fully applied to the sink.
    applied: usize,
    pub playing: bool,
    pub speed: f64,
    pub last_tick: f64,
}

impl Replay {
    pub fn new(mut items: Vec<Item>) -> Self {
        items.sort_by(|a, b| {
            a.created
                .total_cmp(&b.created)
                .then_with(|| a.id.cmp(&b.id))
        });
        let mut events = Vec::with_capacity(items.len());
        let mut cursor = 0.0;
        let mut prev_end: Option<f64> = None;
        for item in items {
            let dur = match &item.data {
                ItemData::Stroke(s) => f64::from(s.points.last().map(|p| p.dt).unwrap_or(0)),
                _ => 0.0,
            };
            if let Some(pe) = prev_end {
                cursor += (item.created - pe).clamp(0.0, MAX_GAP_MS);
            }
            prev_end = Some(item.created + dur);
            events.push(Event {
                start: cursor,
                dur,
                item,
            });
            cursor += dur;
        }
        Self {
            events,
            duration: cursor,
            t: 0.0,
            applied: 0,
            playing: false,
            speed: 1.0,
            last_tick: 0.0,
        }
    }

    pub fn reset(&mut self, sink: &mut impl InkSink) {
        sink.clear();
        self.applied = 0;
    }

    pub fn seek(&mut self, t: f64, sink: &mut impl InkSink) {
        let t = t.clamp(0.0, self.duration);
        if t < self.t {
            self.reset(sink);
        }
        self.t = t;
        while let Some(ev) = self.events.get(self.applied) {
            if ev.start + ev.dur > t {
                break;
            }
            if let ItemData::Stroke(s) = &ev.item.data {
                sink.add_stroke(&ev.item.id, s);
            }
            self.applied += 1;
        }
    }

    /// The stroke currently being drawn, cut at the replay time.
    pub fn partial(&self) -> Option<WetStroke> {
        let ev = self.events.get(self.applied)?;
        if ev.start > self.t {
            return None;
        }
        let ItemData::Stroke(s) = &ev.item.data else {
            return None;
        };
        let local = (self.t - ev.start) as u32;
        let pts: Vec<_> = s
            .points
            .iter()
            .take_while(|p| p.dt <= local)
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
            brush: s.brush(),
        })
    }

    /// DOM items (objects, text, images) that have appeared by now.
    pub fn visible_dom_ids(&self) -> HashSet<&str> {
        self.events
            .iter()
            .take_while(|e| e.start <= self.t)
            .filter(|e| !matches!(e.item.data, ItemData::Stroke(_)))
            .map(|e| e.item.id.as_str())
            .collect()
    }

    pub fn visible_dom_count(&self) -> usize {
        self.visible_dom_ids().len()
    }
}
