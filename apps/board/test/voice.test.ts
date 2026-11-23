// Unit tests for voice input: provider selection (src/voice/select.ts) and
// spoken punctuation / joining (src/voice/punctuation.ts).
// Run: node --test apps/board/test/   (Node 23.6+ strips the types)

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  applySpokenPunctuation, capitalizeSentences, joinDictation, normalizeBangla,
} from "../src/voice/punctuation.ts";
import {
  type VoiceEnv, audioPath, chooseProvider, detectPlatform, osDictationHint, webSpeechUsable,
} from "../src/voice/select.ts";

const UA = {
  chromeWin: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
  edgeWin: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0",
  safariMac: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
  firefoxLinux: "Mozilla/5.0 (X11; Linux x86_64; rv:133.0) Gecko/20100101 Firefox/133.0",
  webkitGtk: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)",
  android: "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36",
};

const env = (ua: string, o: Partial<VoiceEnv> = {}): VoiceEnv => ({
  ...detectPlatform(ua),
  desktop: false, speechApi: true, whisperBuilt: false, whisperModel: null, ...o,
});

// ------------------------------------------------------------ selection

test("platform detection", () => {
  assert.deepEqual(detectPlatform(UA.chromeWin), { os: "windows", engine: "chromium", edge: false });
  assert.deepEqual(detectPlatform(UA.edgeWin), { os: "windows", engine: "chromium", edge: true });
  assert.deepEqual(detectPlatform(UA.safariMac), { os: "mac", engine: "webkit", edge: false });
  assert.deepEqual(detectPlatform(UA.firefoxLinux), { os: "linux", engine: "gecko", edge: false });
  assert.equal(detectPlatform(UA.android).os, "android");
});

test("browsers: Chrome/Edge/Safari use Web Speech, Firefox gets the hint", () => {
  assert.equal(chooseProvider(env(UA.chromeWin)), "webspeech");
  assert.equal(chooseProvider(env(UA.edgeWin)), "webspeech");
  assert.equal(chooseProvider(env(UA.safariMac)), "webspeech");
  assert.equal(chooseProvider(env(UA.firefoxLinux, { speechApi: false })), "hint");
  // Firefox with the flag on still is not trusted (no service behind it).
  assert.equal(chooseProvider(env(UA.firefoxLinux, { speechApi: true })), "hint");
});

test("desktop: WebView2 and WebKitGTK never use Web Speech; WKWebView does", () => {
  assert.equal(chooseProvider(env(UA.edgeWin, { desktop: true })), "hint");
  assert.equal(chooseProvider(env(UA.webkitGtk, { desktop: true, speechApi: false })), "hint");
  assert.equal(chooseProvider(env(UA.safariMac, { desktop: true })), "webspeech");
});

test("desktop: Whisper wins once a model is downloaded", () => {
  const linux = env(UA.webkitGtk, { desktop: true, speechApi: false, whisperBuilt: true });
  assert.equal(chooseProvider(linux), "hint", "built but no model yet");
  assert.equal(chooseProvider({ ...linux, whisperModel: "base" }), "whisper");
  const mac = env(UA.safariMac, { desktop: true, whisperBuilt: true, whisperModel: "tiny" });
  assert.equal(chooseProvider(mac), "whisper");
  assert.equal(chooseProvider(mac, "webspeech"), "webspeech", "a forced provider is honoured");
  // A model on disk but a build without whisper: not usable.
  assert.equal(chooseProvider(env(UA.webkitGtk, { desktop: true, speechApi: false, whisperModel: "base" })), "hint");
});

test("forced providers fall back when unavailable", () => {
  assert.equal(chooseProvider(env(UA.chromeWin), "whisper"), "webspeech");
  assert.equal(chooseProvider(env(UA.chromeWin), "os"), "hint");
  assert.equal(chooseProvider(env(UA.firefoxLinux, { speechApi: false }), "webspeech"), "hint");
});

test("a broken Web Speech session falls back", () => {
  const e = env(UA.chromeWin, { speechBroken: true });
  assert.equal(webSpeechUsable(e), false);
  assert.equal(chooseProvider(e), "hint");
});

test("privacy: cloud vs on-device", () => {
  assert.equal(audioPath("webspeech", env(UA.chromeWin)), "cloud");
  assert.equal(audioPath("webspeech", env(UA.safariMac)), "device-or-cloud");
  assert.equal(audioPath("whisper", env(UA.webkitGtk, { desktop: true })), "device");
});

test("OS dictation hints", () => {
  assert.deepEqual(osDictationHint("windows").keys, ["Win", "H"]);
  assert.match(osDictationHint("mac").text, /Dictation/);
  assert.match(osDictationHint("linux").text, /offline/);
});

// ----------------------------------------------------------- punctuation

test("English spoken punctuation", () => {
  assert.equal(applySpokenPunctuation("hello comma world full stop", "en-US"), "hello, world.");
  assert.equal(applySpokenPunctuation("is it a byte question mark", "en-US"), "is it a byte?");
  assert.equal(applySpokenPunctuation("wow exclamation mark next", "en-US"), "wow! next");
  assert.equal(applySpokenPunctuation("first line new line second", "en-US"), "first line\nsecond");
  assert.equal(applySpokenPunctuation("a colon b semicolon c", "en-US"), "a: b; c");
  assert.equal(applySpokenPunctuation("f open bracket x close bracket", "en-US"), "f (x)");
  assert.equal(applySpokenPunctuation("Comma", "en-US"), ",");
});

test("words that merely contain a keyword are left alone", () => {
  assert.equal(applySpokenPunctuation("commas and periods", "en-US"), "commas and periods");
  assert.equal(applySpokenPunctuation("the colonial period", "en-US"), "the colonial period");
});

test("Bangla spoken punctuation", () => {
  assert.equal(applySpokenPunctuation("আমার সোনার বাংলা দাঁড়ি", "bn-BD"), "আমার সোনার বাংলা।");
  assert.equal(applySpokenPunctuation("তুমি কেমন আছ প্রশ্নবোধক চিহ্ন", "bn-BD"), "তুমি কেমন আছ?");
  assert.equal(applySpokenPunctuation("এক কমা দুই কমা তিন", "bn-BD"), "এক, দুই, তিন");
  assert.equal(applySpokenPunctuation("প্রথম নতুন লাইন দ্বিতীয়", "bn-BD"), "প্রথম\nদ্বিতীয়");
  // "full stop" said in English while dictating Bangla gives a দাঁড়ি.
  assert.equal(applySpokenPunctuation("বাংলা full stop", "bn-BD"), "বাংলা।");
});

test("Bangla nukta spellings match either way", () => {
  const composed = "দাঁড়ি"; // ড় as one code point
  const decomposed = "দাঁড়ি"; // ড + nukta
  assert.equal(normalizeBangla(composed), normalizeBangla(decomposed));
  assert.equal(applySpokenPunctuation(`ঠিক ${composed}`, "bn-BD"), "ঠিক।");
  assert.equal(applySpokenPunctuation(`ঠিক ${decomposed}`, "bn-BD"), "ঠিক।");
});

test("sentence capitals (Latin only)", () => {
  assert.equal(capitalizeSentences("hello. how are you? fine", true), "Hello. How are you? Fine");
  assert.equal(capitalizeSentences("বাংলা। english", true), "বাংলা। English");
});

test("joining dictation into existing text", () => {
  const o = { lang: "en-US" as const, autoPunctuation: true };
  assert.equal(joinDictation("", "the cpu comma memory", "", o), "The cpu, memory");
  assert.equal(joinDictation("Binary", "numbers", "", o), " numbers");
  assert.equal(joinDictation("Binary ", "numbers", "", o), "numbers");
  assert.equal(joinDictation("It works.", "next one", "", o), " Next one");
  assert.equal(joinDictation("word", "comma then", "", o), ", then");
  assert.equal(joinDictation("left", "middle", "right", o), " middle ");
  assert.equal(joinDictation("x", "   ", "", o), "");
  // Off: text goes in as heard (trimmed), still spaced.
  assert.equal(joinDictation("a", "b comma c", "", { lang: "en-US", autoPunctuation: false }), " b comma c");
});
