// Theme fonts for Smart Objects (spec/smart-object.md, "Theme": "Fonts").
//
// Objects run in opaque-origin sandboxed frames, so they cannot see the app's
// @fontsource fonts. The host injects these @font-face rules into every
// object's srcdoc (objects.ts, site.ts), so the theme's font stack
// `"Inter Variable", Inter, "Hind Siliguri"` renders the same on every
// machine, installed fonts or not. Bangla text falls through Inter (latin
// only) to Hind Siliguri.
//
// The files are vendored from node_modules into public/rupoklib/fonts/ by
// scripts/vendor-libs.mjs, which reads the file names below (keep them as
// plain string literals). This module has no imports, so Node can load it
// directly (scripts/preset-thumbs.mjs).

/** Unicode ranges of the fontsource subsets. */
const LATIN =
  "U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD";
const BENGALI =
  "U+0951-0952, U+0964-0965, U+0980-09FE, U+1CD0, U+1CD2, U+1CD5-1CD6, U+1CD8, U+1CE1, U+1CEA, U+1CED, U+1CF2, U+1CF5-1CF7, U+200C-200D, U+20B9, U+25CC, U+A8F1";

export interface ObjectFontFace {
  family: string;
  /** A single weight, or a range for a variable font. */
  weight: string;
  /** File name inside rupoklib/fonts/. */
  file: string;
  range: string;
}

export const OBJECT_FONT_FACES: readonly ObjectFontFace[] = [
  // Inter, variable (wght 100–900), latin. Also answers to plain `Inter`.
  { family: "Inter Variable", weight: "100 900", file: "inter-latin-wght-normal.woff2", range: LATIN },
  { family: "Inter", weight: "100 900", file: "inter-latin-wght-normal.woff2", range: LATIN },
  // Hind Siliguri 400–700, Bengali and latin.
  { family: "Hind Siliguri", weight: "400", file: "hind-siliguri-bengali-400-normal.woff2", range: BENGALI },
  { family: "Hind Siliguri", weight: "500", file: "hind-siliguri-bengali-500-normal.woff2", range: BENGALI },
  { family: "Hind Siliguri", weight: "600", file: "hind-siliguri-bengali-600-normal.woff2", range: BENGALI },
  { family: "Hind Siliguri", weight: "700", file: "hind-siliguri-bengali-700-normal.woff2", range: BENGALI },
  { family: "Hind Siliguri", weight: "400", file: "hind-siliguri-latin-400-normal.woff2", range: LATIN },
  { family: "Hind Siliguri", weight: "500", file: "hind-siliguri-latin-500-normal.woff2", range: LATIN },
  { family: "Hind Siliguri", weight: "600", file: "hind-siliguri-latin-600-normal.woff2", range: LATIN },
  { family: "Hind Siliguri", weight: "700", file: "hind-siliguri-latin-700-normal.woff2", range: LATIN },
];

/**
 * The `<style>` block of @font-face rules for an object document.
 * `lib` is the library base URL ending in `/`, e.g. `${location.origin}/rupoklib/`;
 * the object CSP must allow it in `font-src`.
 */
export function objectFontStyle(lib: string): string {
  const rules = OBJECT_FONT_FACES.map(
    (f) =>
      `@font-face{font-family:"${f.family}";font-style:normal;font-weight:${f.weight};font-display:block;` +
      `src:url("${lib}fonts/${f.file}") format("woff2");unicode-range:${f.range}}`,
  );
  return `<style>${rules.join("")}</style>`;
}
