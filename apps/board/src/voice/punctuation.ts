// Spoken punctuation and joining dictated text into what is already typed.
// Pure functions (no DOM), unit-tested in apps/board/test/voice.test.ts.
//
// "comma", "full stop", "question mark", "new line" (and the Bangla words
// কমা, দাঁড়ি, প্রশ্নবোধক চিহ্ন, নতুন লাইন …) become marks. Chrome's Web Speech
// does not punctuate on its own; Whisper and Apple's recogniser often do, and
// then there is simply nothing left to convert.

import type { VoiceLang } from "./types";

type Rule = [words: string[], mark: string | ((lang: VoiceLang) => string)];

const fullStop = (lang: VoiceLang) => (lang.startsWith("bn") ? "।" : ".");

// Longer phrases first, so "question mark" wins over "mark" and
// "new paragraph" over "new".
const RULES: Rule[] = [
  [["new paragraph", "নতুন অনুচ্ছেদ", "নতুন প্যারা"], "\n\n"],
  [["new line", "next line", "নতুন লাইন", "পরের লাইন"], "\n"],
  [["question mark", "প্রশ্নবোধক চিহ্ন", "প্রশ্নচিহ্ন", "প্রশ্ন চিহ্ন"], "?"],
  [["exclamation mark", "exclamation point", "বিস্ময়সূচক চিহ্ন", "আশ্চর্যবোধক চিহ্ন", "বিস্ময় চিহ্ন"], "!"],
  [["full stop", "দাঁড়ি", "দাড়ি", "পূর্ণচ্ছেদ", "পূর্ণ বিরাম"], fullStop],
  [["semicolon", "সেমিকোলন"], ";"],
  [["colon", "কোলন"], ":"],
  [["comma", "কমা"], ","],
  [["open bracket", "open parenthesis", "বন্ধনী শুরু"], "("],
  [["close bracket", "close parenthesis", "বন্ধনী শেষ"], ")"],
  [["hyphen", "হাইফেন"], "-"],
];

/** Marks that attach to the word before them (no space before). */
const CLOSERS = ",.;:?!।)";

/** One spelling for Bangla letters with a nukta (ড় can be one code point
 * or ড + ়), so "দাঁড়ি" matches whichever form the recogniser sends. */
export function normalizeBangla(s: string): string {
  return s.normalize("NFD").normalize("NFC")
    .replace(/ড়/g, "ড়")
    .replace(/ঢ়/g, "ঢ়")
    .replace(/য়/g, "য়");
}

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** Replace spoken punctuation words with marks and tidy the spacing. */
export function applySpokenPunctuation(text: string, lang: VoiceLang): string {
  let s = normalizeBangla(text);
  for (const [words, mark] of RULES) {
    const m = typeof mark === "function" ? mark(lang) : mark;
    for (const w of words) {
      // A spoken word stands alone: start or space before, end, space or a
      // mark after. (\b does not work for Bangla.)
      const re = new RegExp(`(^|\\s)${escape(normalizeBangla(w)).replace(/ /g, "\\s+")}(?=$|[\\s,.;:?!।])`, "giu");
      s = s.replace(re, (_all, pre: string) => `${pre}\u0000${m}\u0000`);
    }
  }
  return tidy(s);
}

/** Collapse the placeholders: no space before closers, one after; none
 * around new lines; none after "(" or before ")". */
function tidy(s: string): string {
  return s
    .replace(/\s*\u0000(\n+)\u0000\s*/g, "$1")
    .replace(/\s*\u0000([,.;:?!।)])\u0000/g, "$1")
    .replace(/\u0000\(\u0000\s*/g, "(")
    .replace(/\s*\u0000-\u0000\s*/g, "-")
    .replace(/\u0000/g, "")
    // A space after a closer when a word follows it directly.
    .replace(/([,;:?!।])(?=[^\s,.;:?!।)\d])/gu, "$1 ")
    .replace(/(\.)(?=[^\s,.;:?!।)\d.])/gu, "$1 ")
    .replace(/[ \t]{2,}/g, " ")
    .replace(/^[ \t]+/, "");
}

/** Capitalise sentence starts (Latin script only; Bangla has no case). */
export function capitalizeSentences(text: string, startOfSentence: boolean): string {
  let cap = startOfSentence;
  let out = "";
  for (const ch of text) {
    if (cap && /\p{L}/u.test(ch)) {
      out += ch.toLocaleUpperCase();
      cap = false;
      continue;
    }
    if (/[.?!।\n]/.test(ch)) cap = true;
    else if (!/\s|["'(]/.test(ch)) cap = false;
    out += ch;
  }
  return out;
}

/**
 * The text to insert at the caret so it reads naturally after `before`
 * (the text left of the caret) and before `after`:
 * a space between words, none before a closing mark, a capital at the start
 * of a sentence (when `autoPunctuation` is on).
 */
export function joinDictation(
  before: string,
  text: string,
  after: string,
  opts: { lang: VoiceLang; autoPunctuation: boolean },
): string {
  let t = opts.autoPunctuation ? applySpokenPunctuation(text, opts.lang) : text.trim();
  t = t.replace(/^[ \t]+|[ \t]+$/g, "");
  if (!t) return "";
  const prev = before.slice(-1);
  const sentenceStart = before.trim() === "" || /[.?!।]\s*$|\n\s*$/.test(before);
  if (opts.autoPunctuation) t = capitalizeSentences(t, sentenceStart);
  const needsSpaceBefore = prev !== "" && !/[\s(\["'“‘]/.test(prev) && !CLOSERS.includes(t[0]) && t[0] !== "\n";
  if (needsSpaceBefore) t = " " + t;
  const next = after.slice(0, 1);
  if (next && !/[\s,.;:?!।)\]]/.test(next) && !/[\s(\n]$/.test(t)) t += " ";
  return t;
}
