// Unit tests for the Classic layout's config (src/modes.ts: the islands dock
// style, the Classic preset) and the edge swipe's starting rule
// (src/components/classic/edgeRule.ts).
// Run: node --test apps/board/test/   (Node 23.6+ strips the types; bun test works too)

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  CLASSIC_DOCK, DEFAULT_MODES, DOCK_PRESETS, ISLAND_CORE, applyDockPreset, dockWarnings, layoutPins, sanitizeMode, sanitizeModes,
} from "../src/modes.ts";
import { EDGE, TRAVEL, WINDOW, edgeStart, edgeVerdict } from "../src/components/classic/edgeRule.ts";

const classic = DOCK_PRESETS.find((p) => p.name === "Classic")!;

test("the Classic preset makes three islands in Prepare and Teach", () => {
  const modes = applyDockPreset(undefined, classic);
  for (const m of ["prepare", "teach"] as const) {
    const d = modes[m]!.dock;
    assert.equal(d.style, "islands");
    assert.equal(d.size, "l");
    assert.deepEqual(d.groups!.map((g) => g.id), ["left", "centre", "right"]);
    assert.deepEqual(d.groups![0].items, ["menu", "mode", "toolbox", "share", "zoom", "ai"]);
    assert.deepEqual(d.groups![2].items, ["addpage", "pager"]);
    // The island buttons are the mode's tools too.
    for (const t of ["menu", "toolbox", "recognize", "addpage", "ai"] as const) assert.ok(modes[m]!.tools.includes(t), `${m} offers ${t}`);
  }
});

test("Default after Classic is the Smart Dock again", () => {
  const back = applyDockPreset(applyDockPreset(undefined, classic), DOCK_PRESETS[0]);
  assert.equal(back.prepare!.dock.style, undefined);
  assert.equal(back.prepare!.dock.groups, undefined);
});

test("islands keep pen, eraser and undo, in the island with the pen", () => {
  const c = sanitizeMode("prepare", { dock: { style: "islands", groups: [{ id: "a", items: ["menu"] }, { id: "b", items: ["lasso", "pen"] }, { id: "c", items: ["pager"] }] } });
  const pins = layoutPins(c.dock);
  for (const t of ISLAND_CORE) assert.ok(pins.includes(t), `keeps ${t}`);
  assert.deepEqual(c.dock.groups![1].items, ["lasso", "pen", "eraser", "undo"]);
  assert.deepEqual(c.dock.groups![0].items, ["menu"], "the menu island stays as it was");
  // Highlighter and laser are not forced onto islands (they live in the pen panel and the tools box).
  assert.ok(!pins.includes("highlighter") && !pins.includes("laser"));
});

test("the Smart Dock still forces every core tool", () => {
  const c = sanitizeMode("prepare", { dock: { groups: [{ id: "a", items: ["menu"] }] } });
  assert.deepEqual(c.dock.groups![0].items.slice(0, 5), ["pen", "highlighter", "eraser", "laser", "undo"]);
  assert.equal(c.dock.style, undefined);
});

test("islands without groups get the Classic islands", () => {
  const c = sanitizeMode("teach", { dock: { style: "islands" } });
  assert.deepEqual(c.dock.groups!.map((g) => g.id), CLASSIC_DOCK.groups!.map((g) => g.id));
});

test("untrusted island configs are cleaned, never fatal", () => {
  const c = sanitizeModes({
    prepare: { dock: { style: "islands", size: "xl", groups: [{ id: "BAD ID!", items: ["menu", "menu", "nope", 42, "rect"] }, "x", null, { items: [] }] } },
    teach: { dock: { style: "sideways", groups: [{ items: ["zoom"] }] } },
    view: { dock: { style: "islands" } },
    bogus: { dock: {} },
  });
  const p = c.prepare!.dock;
  assert.equal(p.style, "islands");
  assert.equal(p.size, undefined, "an unknown size is dropped");
  assert.equal(p.groups![0].id, "g1", "an invalid id is replaced");
  assert.deepEqual(p.groups![0].items.slice(0, 2), ["menu", "shapes"], "duplicates and unknown ids go; shape tools fold into Shapes");
  assert.equal(c.teach!.dock.style, undefined, "an unknown style is the Smart Dock");
  assert.ok(!("bogus" in c));
});

test("Student view allows the menu and zoom islands but no editing", () => {
  const v = sanitizeMode("view", { tools: ["menu", "zoom", "pen", "addpage", "ai", "pager"] });
  assert.deepEqual(v.tools, ["menu", "zoom", "pager"]);
});

test("Teach with a menu or mode button has a way out", () => {
  const modes = applyDockPreset(undefined, classic);
  assert.deepEqual(dockWarnings("teach", modes.teach!), []);
  assert.deepEqual(dockWarnings("prepare", modes.prepare!), []);
});

test("the default modes are untouched by the Classic layout", () => {
  assert.equal(DEFAULT_MODES.prepare.dock.style, undefined);
  assert.equal(DEFAULT_MODES.teach.dock.style, undefined);
});

test("edge swipe: where it may start", () => {
  assert.equal(edgeStart(4, 1920, "touch", 0), "left");
  assert.equal(edgeStart(1915, 1920, "pen", 0), "right");
  assert.equal(edgeStart(EDGE + 1, 1920, "touch", 0), null, "outside the strip: ink");
  assert.equal(edgeStart(4, 1920, "mouse", 0), null, "never the mouse");
  assert.equal(edgeStart(4, 1920, "touch", 1), null, "a second finger is another gesture");
  assert.equal(edgeStart(30, 1920, "touch", 0, 24), "left", "the safe area widens the strip");
});

test("edge swipe: inward, sideways and quick", () => {
  const s = { id: 1, x: 1915, y: 500, t: 0, side: "right" as const };
  assert.equal(edgeVerdict(s, 1915 - 20, 502, 50), "wait");
  assert.equal(edgeVerdict(s, 1915 - TRAVEL, 505, 120), "swipe");
  assert.equal(edgeVerdict(s, 1915 - TRAVEL, 560, 120), "no", "too much up or down: writing");
  assert.equal(edgeVerdict(s, 1915 - TRAVEL, 500, WINDOW + 1), "no", "too slow: writing");
  assert.equal(edgeVerdict(s, 1925, 500, 50), "no", "outward");
  const l = { id: 2, x: 3, y: 300, t: 0, side: "left" as const };
  assert.equal(edgeVerdict(l, 3 + TRAVEL + 4, 310, 200), "swipe");
});
