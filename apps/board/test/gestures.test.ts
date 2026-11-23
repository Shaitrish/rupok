// Unit tests for the classroom gesture recogniser (src/gestures.ts) with
// synthetic touch streams: palm vs finger vs pinch vs swipe vs pen.
// Run: node --test apps/board/test/   (Node 23.6+ strips the types; bun test works too)

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_GESTURES, GestureRecognizer, calibrate, resolveGestures, sanitizeGestures, thresholds,
  type Feedback, type FingerRole, type GestureConfig, type GestureHost, type Verdict,
} from "../src/gestures.ts";

type Call = [string, ...unknown[]];

function setup(opts: { config?: Partial<GestureConfig>; role?: FingerRole; mode?: string; canEdit?: boolean } = {}) {
  const calls: Call[] = [];
  const feedback: Feedback[] = [];
  let clock = 0;
  let cam = [0, 0, 1];
  const timers: { due: number; fn: () => void; id: number }[] = [];
  let nextId = 1;
  const state = { role: opts.role ?? "draw", mode: opts.mode ?? "prepare", canEdit: opts.canEdit ?? true };
  const config: GestureConfig = { ...DEFAULT_GESTURES, ...opts.config };
  const host: GestureHost = {
    oneFinger: () => state.role as FingerRole,
    mode: () => state.mode,
    canEdit: () => state.canEdit,
    cancel: (id) => calls.push(["cancel", id]),
    camera: () => [...cam],
    setCamera: (c) => { calls.push(["setCamera", ...c]); cam = [...c]; },
    erase: (x0, y0, x1, y1, r) => { calls.push(["erase", x0, y0, x1, y1, r]); return 1; },
    eraseEnd: () => { calls.push(["eraseEnd"]); return calls.filter((c) => c[0] === "erase").length; },
    next: () => calls.push(["next"]),
    prev: () => calls.push(["prev"]),
    undo: () => calls.push(["undo"]),
    redo: () => calls.push(["redo"]),
    fit: () => calls.push(["fit"]),
    summonDock: (x, y) => calls.push(["dock", x, y]),
    feedback: (f) => feedback.push(f),
    setTimer: (fn, ms) => { const id = nextId++; timers.push({ due: clock + ms, fn, id }); return id; },
    clearTimer: (id) => { const i = timers.findIndex((t) => t.id === id); if (i >= 0) timers.splice(i, 1); },
  };
  const rec = new GestureRecognizer(host, () => config);
  const advance = (to: number) => {
    clock = Math.max(clock, to);
    for (;;) {
      timers.sort((a, b) => a.due - b.due);
      const t = timers[0];
      if (!t || t.due > clock) break;
      timers.shift();
      t.fn();
    }
  };
  const ev = (kind: "down" | "move" | "up" | "cancel", id: number, x: number, y: number, t: number, size = 0, type = "touch"): Verdict => {
    advance(t);
    return rec.handle(kind, { id, type, x, y, t, w: size, h: size, board: true });
  };
  const count = (name: string) => calls.filter((c) => c[0] === name).length;
  const fired = () => feedback.flatMap((f) => (f.kind === "fired" ? [f.gesture] : []));
  return { rec, calls, feedback, ev, advance, count, fired, state, config, pan: (x: number, y: number) => { cam = [x, y, cam[2]]; } };
}

/** A circular scrub: `n` samples starting at t, 16 ms apart. */
function circle(cx: number, cy: number, r: number, n: number, t0: number) {
  return Array.from({ length: n }, (_, i) => {
    const a = (i / 8) * Math.PI * 2;
    return { x: cx + Math.cos(a) * r, y: cy + Math.sin(a) * r, t: t0 + (i + 1) * 16 };
  });
}

test("a large contact is a palm: consumed, erases along its rub, one commit", () => {
  const g = setup();
  assert.equal(g.ev("down", 1, 500, 400, 0, 90), "consume");
  assert.equal(g.rec.state, "palm");
  for (const p of circle(500, 400, 40, 24, 0)) assert.equal(g.ev("move", 1, p.x, p.y, p.t, 92), "consume");
  assert.ok(g.count("erase") > 10, "the rub erases as it goes");
  const radii = g.calls.filter((c) => c[0] === "erase").map((c) => c[5] as number);
  assert.ok(radii.every((r) => r > 35 && r < 60), `radius follows the contact (≈46): ${radii.slice(0, 3)}`);
  assert.equal(g.count("eraseEnd"), 0);
  assert.equal(g.ev("up", 1, 500, 400, 500), "consume");
  assert.equal(g.count("eraseEnd"), 1, "one gesture, one undo step");
  assert.equal(g.count("cancel"), 0, "the engine never saw it");
  assert.deepEqual(g.fired(), ["palm"]);
  assert.ok(g.feedback.some((f) => f.kind === "palm"), "the eraser cursor is shown");
  assert.ok(g.feedback.some((f) => f.kind === "palm-end"));
});

test("a resting palm that does not move erases nothing", () => {
  const g = setup();
  g.ev("down", 1, 500, 400, 0, 90);
  g.ev("move", 1, 503, 401, 100, 90);
  g.ev("up", 1, 503, 401, 600);
  assert.equal(g.count("erase"), 0);
  assert.equal(g.count("eraseEnd"), 1);
  assert.deepEqual(g.fired(), [], "no first-time hint for a hand that only rested");
});

test("a contact that grows into a palm takes the touch back from the engine", () => {
  const g = setup();
  assert.equal(g.ev("down", 1, 300, 300, 0, 14), "pass");
  assert.equal(g.ev("move", 1, 310, 305, 16, 20), "pass");
  assert.equal(g.ev("move", 1, 320, 310, 32, 80), "consume");
  assert.deepEqual(g.calls.find((c) => c[0] === "cancel"), ["cancel", 1], "the half-drawn stroke is cancelled");
  assert.equal(g.rec.state, "palm");
  assert.ok(g.count("erase") > 0, "erases where it has been (retroactively)");
});

test("a finger draws: every event passes, nothing erased or cancelled", () => {
  const g = setup();
  assert.equal(g.ev("down", 1, 100, 100, 0, 12), "pass");
  for (let i = 1; i <= 30; i++) assert.equal(g.ev("move", 1, 100 + i * 10, 100 + Math.sin(i) * 20, i * 16, 12), "pass");
  assert.equal(g.ev("up", 1, 400, 100, 500), "pass");
  assert.equal(g.calls.length, 0);
  assert.deepEqual(g.fired(), []);
});

test("a fast one-finger scribble while fingers draw is ink, not a scrub", () => {
  const g = setup({ role: "draw" });
  g.ev("down", 1, 300, 300, 0);
  for (let i = 1; i <= 40; i++) assert.equal(g.ev("move", 1, 300 + (i % 2 ? 60 : 0), 300 + i * 2, i * 12), "pass");
  g.ev("up", 1, 300, 380, 600);
  assert.equal(g.count("erase"), 0);
});

test("pinch: two fingers spreading pass through, no swipe", () => {
  const g = setup();
  g.ev("down", 1, 500, 400, 0);
  g.ev("down", 2, 700, 400, 10);
  for (let i = 1; i <= 10; i++) {
    assert.equal(g.ev("move", 1, 500 - i * 20, 400, 10 + i * 16), "pass");
    assert.equal(g.ev("move", 2, 700 + i * 20, 400, 10 + i * 16), "pass");
  }
  g.ev("up", 1, 300, 400, 200);
  g.ev("up", 2, 900, 400, 210);
  g.advance(2000);
  assert.equal(g.count("next") + g.count("prev") + g.count("undo"), 0);
});

test("a slow two-finger pan is a pan, not a swipe", () => {
  const g = setup();
  g.ev("down", 1, 500, 400, 0);
  g.ev("down", 2, 600, 400, 5);
  for (let i = 1; i <= 30; i++) {
    g.ev("move", 1, 500 - i * 8, 400, 5 + i * 33);
    g.ev("move", 2, 600 - i * 8, 400, 5 + i * 33);
  }
  g.ev("up", 1, 260, 400, 1100);
  g.ev("up", 2, 360, 400, 1100);
  assert.equal(g.count("next") + g.count("prev"), 0);
});

test("two-finger swipe left: next; the engine's pan is undone", () => {
  const g = setup();
  g.ev("down", 1, 900, 400, 0);
  g.ev("down", 2, 900, 520, 8);
  let v: Verdict = "pass";
  for (let i = 1; i <= 10; i++) {
    g.pan(i * 3, 0); // the engine pans while it still has the fingers
    v = g.ev("move", 1, 900 - i * 30, 400 + i, 8 + i * 16);
    v = g.ev("move", 2, 900 - i * 30, 520 + i, 8 + i * 16);
  }
  assert.equal(v, "consume");
  assert.equal(g.rec.state, "swipe");
  assert.deepEqual(g.calls.filter((c) => c[0] === "cancel").map((c) => c[1]).sort(), [1, 2]);
  assert.deepEqual(g.calls.find((c) => c[0] === "setCamera"), ["setCamera", 0, 0, 1], "camera back where it started");
  assert.equal(g.count("next"), 0, "navigates on release");
  g.ev("up", 1, 600, 410, 200);
  g.ev("up", 2, 600, 530, 205);
  assert.equal(g.count("next"), 1);
  assert.deepEqual(g.fired(), ["swipe"]);
  assert.ok(g.feedback.some((f) => f.kind === "swipe" && f.dir === 1));
});

test("two-finger swipe right: previous; reversing before lifting cancels", () => {
  const g = setup();
  g.ev("down", 1, 300, 400, 0);
  g.ev("down", 2, 300, 500, 0);
  for (let i = 1; i <= 8; i++) {
    g.ev("move", 1, 300 + i * 35, 400, i * 16);
    g.ev("move", 2, 300 + i * 35, 500, i * 16);
  }
  g.ev("up", 1, 580, 400, 150);
  g.ev("up", 2, 580, 500, 150);
  assert.equal(g.count("prev"), 1);

  const h = setup();
  h.ev("down", 1, 300, 400, 0);
  h.ev("down", 2, 300, 500, 0);
  for (let i = 1; i <= 8; i++) {
    h.ev("move", 1, 300 + i * 35, 400, i * 16);
    h.ev("move", 2, 300 + i * 35, 500, i * 16);
  }
  for (let i = 1; i <= 8; i++) {
    h.ev("move", 1, 580 - i * 35, 400, 150 + i * 16);
    h.ev("move", 2, 580 - i * 35, 500, 150 + i * 16);
  }
  h.ev("up", 1, 300, 400, 300);
  h.ev("up", 2, 300, 500, 300);
  assert.equal(h.count("prev") + h.count("next"), 0, "changed their mind");
  assert.ok(h.feedback.some((f) => f.kind === "swipe-end" && !f.committed));
});

test("three-finger swipe when set to 3 fingers; 2 fingers then do nothing", () => {
  const g = setup({ config: { swipeFingers: 3 } });
  g.ev("down", 1, 900, 400, 0);
  g.ev("down", 2, 960, 420, 4);
  g.ev("down", 3, 1020, 400, 8);
  for (let i = 1; i <= 10; i++) for (const [id, x, y] of [[1, 900, 400], [2, 960, 420], [3, 1020, 400]]) g.ev("move", id, x - i * 30, y, 8 + i * 16);
  for (const id of [1, 2, 3]) g.ev("up", id, 0, 0, 200);
  assert.equal(g.count("next"), 1);
  assert.equal(g.count("erase"), 0, "a straight fast swipe is not a rub");

  const h = setup({ config: { swipeFingers: 3 } });
  h.ev("down", 1, 900, 400, 0);
  h.ev("down", 2, 900, 520, 0);
  for (let i = 1; i <= 10; i++) { h.ev("move", 1, 900 - i * 30, 400, i * 16); h.ev("move", 2, 900 - i * 30, 520, i * 16); }
  h.ev("up", 1, 0, 0, 200);
  h.ev("up", 2, 0, 0, 200);
  assert.equal(h.count("next"), 0);
});

test("swipes can be turned off", () => {
  const g = setup({ config: { swipe: false } });
  g.ev("down", 1, 900, 400, 0);
  g.ev("down", 2, 900, 520, 0);
  for (let i = 1; i <= 10; i++) { g.ev("move", 1, 900 - i * 30, 400, i * 16); g.ev("move", 2, 900 - i * 30, 520, i * 16); }
  g.ev("up", 1, 0, 0, 200);
  g.ev("up", 2, 0, 0, 200);
  assert.equal(g.count("next"), 0);
});

test("no navigation while the pen is drawing", () => {
  const g = setup();
  g.ev("down", 9, 200, 200, 0, 0, "pen");
  g.ev("down", 1, 900, 400, 10);
  g.ev("down", 2, 900, 520, 12);
  for (let i = 1; i <= 10; i++) {
    assert.equal(g.ev("move", 1, 900 - i * 30, 400, 12 + i * 16), "pass");
    g.ev("move", 2, 900 - i * 30, 520, 12 + i * 16);
  }
  g.ev("up", 1, 0, 0, 200);
  g.ev("up", 2, 0, 0, 200);
  g.ev("up", 9, 200, 200, 210, 0, "pen");
  assert.equal(g.count("next") + g.count("cancel"), 0);
});

test("a pen landing during a swipe cancels it", () => {
  const g = setup();
  g.ev("down", 1, 900, 400, 0);
  g.ev("down", 2, 900, 520, 0);
  for (let i = 1; i <= 8; i++) { g.ev("move", 1, 900 - i * 30, 400, i * 16); g.ev("move", 2, 900 - i * 30, 520, i * 16); }
  assert.equal(g.rec.state, "swipe");
  g.ev("down", 9, 200, 200, 140, 0, "pen");
  g.ev("up", 1, 0, 0, 200);
  g.ev("up", 2, 0, 0, 200);
  assert.equal(g.count("next"), 0);
});

test("the pen wins over a palm erase", () => {
  const g = setup();
  g.ev("down", 1, 500, 400, 0, 90);
  for (const p of circle(500, 400, 40, 8, 0)) g.ev("move", 1, p.x, p.y, p.t, 90);
  const before = g.count("erase");
  assert.ok(before > 0);
  assert.equal(g.ev("down", 9, 100, 100, 200, 0, "pen"), "pass", "the pen goes to the engine");
  assert.equal(g.count("eraseEnd"), 1, "the palm erase is committed at once");
  for (const p of circle(500, 400, 40, 8, 200)) assert.equal(g.ev("move", 1, p.x, p.y, p.t, 90), "consume", "the hand neither erases nor pans");
  assert.equal(g.count("erase"), before);
  g.ev("up", 1, 500, 400, 400);
  assert.equal(g.count("eraseEnd"), 1);
});

test("palm rejection: a palm-sized contact while writing is ignored, not an eraser", () => {
  const g = setup();
  g.ev("down", 9, 100, 100, 0, 0, "pen");
  assert.equal(g.ev("down", 1, 300, 500, 20, 90), "consume");
  for (const p of circle(300, 500, 30, 10, 20)) assert.equal(g.ev("move", 1, p.x, p.y, p.t, 90), "consume");
  g.ev("up", 1, 300, 500, 300);
  g.ev("up", 9, 100, 100, 320, 0, "pen");
  assert.equal(g.count("erase"), 0);
  // Right after the pen lifts the writing hand is still around.
  assert.equal(g.ev("down", 2, 300, 500, 400, 90), "consume");
  g.ev("up", 2, 300, 500, 450);
  assert.equal(g.count("erase") + g.count("eraseEnd"), 0);
});

test("hand cluster: 3 touches close together that rub erase (no contact size)", () => {
  const g = setup();
  g.ev("down", 1, 500, 400, 0);
  g.ev("down", 2, 550, 420, 6);
  g.ev("down", 3, 520, 460, 12);
  const offs = [[0, 0], [50, 20], [20, 60]];
  let v: Verdict = "pass";
  for (const p of circle(500, 400, 30, 20, 12)) {
    for (let k = 0; k < 3; k++) v = g.ev("move", k + 1, p.x + offs[k][0] - 30, p.y + offs[k][1], p.t);
  }
  assert.equal(v, "consume");
  assert.equal(g.rec.state, "palm");
  assert.equal(g.count("cancel"), 3, "all three touches are taken back from the engine");
  assert.ok(g.count("erase") > 5);
  const r = g.calls.filter((c) => c[0] === "erase").map((c) => c[5] as number);
  assert.ok(Math.max(...r) >= 44, "the eraser covers the hand");
  for (const id of [1, 2, 3]) g.ev("up", id, 0, 0, 500);
  assert.equal(g.count("eraseEnd"), 1);
});

test("hand cluster can be turned off; touches spread wide are not a hand", () => {
  const g = setup({ config: { handCluster: false } });
  g.ev("down", 1, 500, 400, 0);
  g.ev("down", 2, 550, 420, 6);
  g.ev("down", 3, 520, 460, 12);
  for (const p of circle(500, 400, 30, 20, 12)) for (const id of [1, 2, 3]) g.ev("move", id, p.x, p.y, p.t);
  assert.equal(g.count("erase"), 0);

  const h = setup();
  h.ev("down", 1, 100, 400, 0);
  h.ev("down", 2, 600, 420, 6);
  h.ev("down", 3, 1100, 460, 12);
  for (const p of circle(0, 0, 30, 20, 12)) for (const [id, x] of [[1, 100], [2, 600], [3, 1100]]) h.ev("move", id, x + p.x, 400 + p.y, p.t);
  assert.equal(h.count("erase"), 0);
});

test("thumb scrub: one touch rubbing fast erases when fingers don't draw", () => {
  const g = setup({ role: "pan" });
  g.ev("down", 1, 400, 400, 0);
  let v: Verdict = "pass";
  for (let i = 1; i <= 24; i++) v = g.ev("move", 1, 400 + (i % 2 ? 50 : -10), 400 + (i % 4 < 2 ? 0 : 30), i * 20);
  assert.equal(v, "consume");
  assert.equal(g.rec.state, "palm");
  assert.equal(g.count("cancel"), 1);
  g.ev("up", 1, 400, 400, 600);
  assert.equal(g.count("eraseEnd"), 1);
});

test("Teach: a one-finger flick changes the slide when fingers don't draw", () => {
  const g = setup({ mode: "teach", role: "pan" });
  g.ev("down", 1, 900, 500, 0);
  for (let i = 1; i <= 10; i++) g.ev("move", 1, 900 - i * 35, 500 + i, i * 16);
  g.ev("up", 1, 550, 510, 170);
  assert.equal(g.count("next"), 1);
  assert.deepEqual(g.fired(), ["flick"]);

  for (const [mode, role] of [["prepare", "pan"], ["teach", "draw"], ["teach", "select"]] as const) {
    const h = setup({ mode, role });
    h.ev("down", 1, 900, 500, 0);
    for (let i = 1; i <= 10; i++) h.ev("move", 1, 900 - i * 35, 500, i * 16);
    h.ev("up", 1, 550, 500, 170);
    assert.equal(h.count("next"), 0, `${mode}/${role}: one finger never flicks`);
  }
  const off = setup({ mode: "teach", role: "pan", config: { flick: false } });
  off.ev("down", 1, 900, 500, 0);
  for (let i = 1; i <= 10; i++) off.ev("move", 1, 900 - i * 35, 500, i * 16);
  off.ev("up", 1, 550, 500, 170);
  assert.equal(off.count("next"), 0);
});

test("a steep flick is not a swipe (angle)", () => {
  const g = setup({ mode: "teach", role: "pan" });
  g.ev("down", 1, 900, 200, 0);
  for (let i = 1; i <= 10; i++) g.ev("move", 1, 900 - i * 30, 200 + i * 30, i * 16);
  g.ev("up", 1, 600, 500, 170);
  assert.equal(g.count("next"), 0);
});

test("two-finger tap: undo (after the double-tap window)", () => {
  const g = setup();
  g.ev("down", 1, 500, 400, 0);
  g.ev("down", 2, 600, 400, 30);
  g.ev("move", 1, 502, 401, 60);
  g.ev("up", 1, 502, 401, 120);
  g.ev("up", 2, 600, 400, 130);
  assert.equal(g.count("undo"), 0, "waits to see whether a second tap follows");
  g.advance(130 + thresholds(g.config).doubleTapMs + 1);
  assert.equal(g.count("undo"), 1);
  assert.deepEqual(g.fired(), ["tapUndo"]);
});

test("two-finger double-tap: fit, without undoing", () => {
  const g = setup();
  for (const t0 of [0, 220]) {
    g.ev("down", 1, 500, 400, t0);
    g.ev("down", 2, 600, 400, t0 + 20);
    g.ev("up", 1, 500, 400, t0 + 90);
    g.ev("up", 2, 600, 400, t0 + 100);
  }
  g.advance(2000);
  assert.equal(g.count("fit"), 1);
  assert.equal(g.count("undo"), 0);
});

test("with double-tap off, a two-finger tap undoes at once", () => {
  const g = setup({ config: { doubleTapFit: false } });
  g.ev("down", 1, 500, 400, 0);
  g.ev("down", 2, 600, 400, 20);
  g.ev("up", 1, 500, 400, 90);
  g.ev("up", 2, 600, 400, 100);
  assert.equal(g.count("undo"), 1);
});

test("three-finger tap: redo; slow or moving taps do nothing", () => {
  const g = setup();
  g.ev("down", 1, 500, 400, 0);
  g.ev("down", 2, 560, 380, 20);
  g.ev("down", 3, 620, 400, 40);
  for (const id of [1, 2, 3]) g.ev("up", id, 0 + 500 + (id - 1) * 60, 400, 150);
  assert.equal(g.count("redo"), 1);

  const slow = setup();
  slow.ev("down", 1, 500, 400, 0);
  slow.ev("down", 2, 600, 400, 20);
  slow.ev("up", 1, 500, 400, 700);
  slow.ev("up", 2, 600, 400, 700);
  slow.advance(3000);
  assert.equal(slow.count("undo"), 0, "a held two-finger touch is not a tap");

  const staggered = setup();
  staggered.ev("down", 1, 500, 400, 0);
  staggered.ev("up", 1, 500, 400, 60);
  staggered.ev("down", 2, 600, 400, 80);
  staggered.ev("up", 2, 600, 400, 140);
  staggered.advance(3000);
  assert.equal(staggered.count("undo"), 0, "two one-finger taps are not a two-finger tap");
});

test("taps never undo in the student view", () => {
  const g = setup({ canEdit: false, role: "pan", mode: "view" });
  g.ev("down", 1, 500, 400, 0);
  g.ev("down", 2, 600, 400, 20);
  g.ev("up", 1, 500, 400, 90);
  g.ev("up", 2, 600, 400, 100);
  g.advance(3000);
  assert.equal(g.count("undo"), 0);
});

test("two-finger long-press summons the dock at that spot", () => {
  const g = setup();
  g.ev("down", 1, 500, 400, 0);
  g.ev("down", 2, 600, 400, 20);
  g.ev("move", 1, 503, 402, 300);
  g.advance(20 + thresholds(g.config).longPressMs + 1);
  assert.deepEqual(g.calls.find((c) => c[0] === "dock"), ["dock", 551.5, 401]);
  assert.equal(g.count("cancel"), 2);
  assert.equal(g.ev("move", 1, 520, 420, 800), "consume", "the rest of the touch is ours");
  g.ev("up", 1, 520, 420, 900);
  g.ev("up", 2, 600, 400, 900);
  g.advance(3000);
  assert.equal(g.count("undo"), 0);

  const moved = setup();
  moved.ev("down", 1, 500, 400, 0);
  moved.ev("down", 2, 600, 400, 20);
  moved.ev("move", 1, 560, 420, 200);
  moved.advance(2000);
  assert.equal(moved.count("dock"), 0, "a pinch that started is not a long-press");
});

test("a board that reports one size for every touch never turns fingers into palms", () => {
  const g = setup();
  // Four ordinary finger strokes teach it what a finger looks like here…
  for (let k = 0; k < 4; k++) {
    const t0 = k * 1000;
    g.ev("down", 1, 100, 100, t0, 48);
    g.ev("move", 1, 200, 100, t0 + 50, 48);
    g.ev("up", 1, 200, 100, t0 + 100);
  }
  assert.equal(g.ev("down", 1, 100, 100, 5000, 48), "pass", "same size as a finger: a finger");
  g.ev("up", 1, 100, 100, 5100);
  assert.equal(g.ev("down", 1, 100, 100, 6000, 110), "consume", "much bigger: a palm");
});

test("calibration says the board does not report size: size detection is off", () => {
  const g = setup({ config: { calibration: { finger: 0, palm: 0, contacts: 4, spread: 60, reportsSize: false, at: 0 } } });
  assert.equal(g.ev("down", 1, 100, 100, 0, 90), "pass");
});

test("no palm erase in the student view", () => {
  const g = setup({ canEdit: false, mode: "view", role: "pan" });
  assert.equal(g.ev("down", 1, 500, 400, 0, 90), "pass");
  for (const p of circle(500, 400, 40, 10, 0)) g.ev("move", 1, p.x, p.y, p.t, 90);
  assert.equal(g.count("erase"), 0);
});

test("calibrate(): picks a threshold from finger and palm sizes", () => {
  const a = calibrate({ size: 16 }, { size: 96, contacts: 1, spread: 0 }, 1);
  assert.equal(a.reportsSize, true);
  assert.ok(a.patch.palmSize! > 20 && a.patch.palmSize! < 77, `threshold ${a.patch.palmSize}`);
  assert.match(a.message, /reports touch size/);

  const b = calibrate({ size: 1 }, { size: 1, contacts: 4, spread: 70 }, 1);
  assert.equal(b.reportsSize, false);
  assert.match(b.message, /does not report touch size, using hand-cluster detection/);
  assert.equal(b.patch.handCluster, true);
  assert.equal(b.patch.calibration?.spread, 70);

  const c = calibrate({ size: 30 }, { size: 32, contacts: 1, spread: 0 }, 1);
  assert.equal(c.reportsSize, false, "a constant size is not a real size");
  assert.equal(c.patch.thumbScrub, true);
});

test("sanitizeGestures keeps valid values only", () => {
  const s = sanitizeGestures({ palmErase: false, palmSize: 9999, swipeFingers: 4, flick: "yes", eraserScale: 1.5, junk: 1, calibration: { finger: "x" } });
  assert.deepEqual(s, { palmErase: false, palmSize: 200, eraserScale: 1.5 });
  assert.deepEqual(sanitizeGestures(null), {});
  assert.equal(resolveGestures({ swipeFingers: 3 }).swipeFingers, 3);
  assert.equal(resolveGestures("nope").palmErase, true);
});

test("sensitivity: eager swipes need less distance and speed than strict ones", () => {
  const strict = thresholds({ ...DEFAULT_GESTURES, swipeSensitivity: 0 });
  const eager = thresholds({ ...DEFAULT_GESTURES, swipeSensitivity: 1 });
  assert.ok(eager.swipeDistance < strict.swipeDistance && eager.swipeVelocity < strict.swipeVelocity);
});
