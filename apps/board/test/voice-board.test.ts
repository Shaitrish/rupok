// Unit tests for Voice on the board: line breaking inside the box
// (src/voice/wrap.ts). Run: node --test apps/board/test/

import { test } from "node:test";
import assert from "node:assert/strict";
import { splitLines, tidyText, wrapLines, wrapText } from "../src/voice/wrap.ts";

/** Every character 10 units wide. */
const mono = (s: string) => [...s].length * 10;

test("wraps at spaces inside the width", () => {
  assert.deepEqual(wrapLines("the quick brown fox jumps", 110, mono), ["the quick", "brown fox", "jumps"]);
  for (const l of wrapLines("the quick brown fox jumps over the lazy dog", 110, mono)) assert.ok(mono(l) <= 110, l);
});

test("keeps spoken new lines and breaks long words", () => {
  assert.deepEqual(wrapLines("one\ntwo three", 200, mono), ["one", "two three"]);
  assert.deepEqual(wrapLines("supercalifragilistic", 50, mono), ["super", "calif", "ragil", "istic"]);
  assert.equal(wrapText("a b", 0, mono), "a b");
});

test("Bangla wraps at spaces and keeps the danda", () => {
  const s = "আমি স্কুলে যাই। তুমি কোথায় যাও?";
  const lines = wrapLines(s, 120, mono);
  assert.ok(lines.length > 1);
  assert.equal(lines.join(" "), s);
  assert.ok(lines.every((l) => !l.startsWith("।")) && lines.some((l) => l.includes("যাই।")), JSON.stringify(lines));
});

test("a Bangla word is never split inside a conjunct", () => {
  // Grapheme clusters: ক্ষ (ka + virama + ssa) stays whole when a word must break.
  const lines = wrapLines("ক্ষক্ষক্ষক্ষ", 20, (s) => [...new Intl.Segmenter(undefined, { granularity: "grapheme" }).segment(s)].length * 10);
  assert.ok(lines.every((l) => !l.startsWith("্")), JSON.stringify(lines));
});

test("splitLines: final words and the live tail map onto the wrapped lines", () => {
  const lines = splitLines("Hello class, today we", " learn fractions", 120, mono);
  assert.deepEqual(lines.map((l) => l.done + l.live), ["Hello class,", "today we", "learn", "fractions"]);
  assert.deepEqual(lines[1], { done: "today we", live: "" });
  assert.deepEqual(lines[2], { done: "", live: "learn" });
  // A line holding both.
  const mixed = splitLines("Hi", " there", 200, mono);
  assert.deepEqual(mixed, [{ done: "Hi", live: " there" }]);
  // A spoken new line.
  assert.deepEqual(splitLines("one\n", "two", 200, mono).map((l) => l.done + l.live), ["one", "two"]);
});

test("tidyText collapses spaces around line breaks", () => {
  assert.equal(tidyText("a  b \n  c"), "a b\nc");
});
