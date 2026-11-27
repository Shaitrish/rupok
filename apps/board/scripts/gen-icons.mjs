// Extracts the Solar icons the UI uses (all style variants) into a small TS
// module, so the app bundles ~40 icons instead of the 1 MB icon set.
// Run: node scripts/gen-icons.mjs
import { readFileSync, readdirSync, writeFileSync } from "node:fs";

const set = JSON.parse(readFileSync(new URL("../node_modules/@iconify-json/solar/icons.json", import.meta.url)));
const VARIANTS = ["linear", "line-duotone", "broken", "bold", "bold-duotone", "outline"];

// logical name -> candidate Solar base names (first that exists in every variant wins)
const MAP = {
  pen: ["pen-2", "pen"],
  highlighter: ["highlighter"],
  eraser: ["eraser"],
  select: ["cursor-square", "cursor"],
  laser: ["target", "magic-stick-2"],
  text: ["text-square", "text"],
  rect: ["stop"],
  ellipse: ["record", "record-circle"],
  line: ["minus"],
  arrow: ["arrow-right-up"],
  hand: ["hand-shake", "hand-stars"],
  interact: ["cursor-click", "cursor"],
  undo: ["undo-left-round"],
  redo: ["undo-right-round"],
  save: ["diskette"],
  open: ["folder-open"],
  pdf: ["document-add"],
  image: ["gallery-add"],
  object: ["magic-stick-3", "stars"],
  replay: ["history"],
  play: ["play"],
  pause: ["pause"],
  fullscreen: ["full-screen"],
  settings: ["settings"],
  sun: ["sun-2"],
  moon: ["moon"],
  plus: ["add-square", "add-circle"],
  trash: ["trash-bin-minimalistic"],
  close: ["close-circle"],
  sidebar: ["sidebar-minimalistic", "sidebar-code"],
  zoomIn: ["magnifer-zoom-in"],
  zoomOut: ["magnifer-zoom-out"],
  download: ["download-minimalistic"],
  palette: ["pallete-2"],
  layers: ["layers"],
  monitor: ["monitor"],
  keyboard: ["keyboard"],
  info: ["info-circle"],
  insert: ["widget-add"],
  more: ["menu-dots"],
  duplicate: ["copy"],
  front: ["layers-arrow-up", "layer-arrow-up"],
  prev: ["alt-arrow-left"],
  next: ["alt-arrow-right"],
  eye: ["eye"],
  up: ["alt-arrow-up"],
  down: ["alt-arrow-down"],
  group: ["link-round-angle", "link"],
  ungroup: ["link-broken", "unlink"],
  merge: ["union"],
  import: ["import"],
  export: ["export"],
  restart: ["restart"],
  exit: ["exit", "logout"],
  overlay: ["layers-minimalistic", "layers"],
  smartink: ["ruler-cross-pen", "ruler-pen"],
  totext: ["text-field-focus", "text-field"],
  grow: ["maximize-square-minimalistic", "maximize-square"],
  shrink: ["minimize-square-minimalistic", "minimize-square"],
  broom: ["broom"],
  pin: ["pin"],
  pages: ["widget-4"],
  // Rich media (src/media/, components/Media*.tsx)
  video: ["videocamera-record"],
  audio: ["music-note-2"],
  cube: ["box"],
  globe: ["global"],
  volume: ["volume-loud"],
  mute: ["volume-cross"],
  openLink: ["square-arrow-right-up"],
  replace: ["gallery-edit", "refresh"],
  camera3d: ["camera-rotate"],
  stamp: ["medal-star", "medal-ribbon-star"],
  celebrate: ["confetti", "confetti-minimalistic"],
  heart: ["heart"],
  // The Classic layout (components/classic/)
  menu: ["hamburger-menu"],
  share: ["share"],
  search: ["magnifer"],
  toolbox: ["case-round", "case"],
  recognize: ["magic-stick", "magic-stick-2"],
  print: ["printer-minimalistic", "printer"],
  help: ["question-circle"],
  star: ["star"],
  file: ["file"],
  folder: ["folder-with-files"],
  organise: ["sort-vertical"],
  pipette: ["pipette"],
  library: ["library"],
};

function body(name) {
  const icon = set.icons[name];
  if (icon) return icon.body;
  const alias = set.aliases?.[name];
  if (alias) return body(alias.parent);
  return null;
}

// Composed icons: a Solar icon made smaller in the top-left corner, with a
// mark drawn in the free bottom-right corner (same stroke as the variant).
// voiceText: the Voice text tool (src/voice/board.ts), a mic with a T.
const COMPOSE = {
  voiceText: { base: "microphone", mark: "M14.5 15H21.5M18 15V21.5" },
};
const STROKED = new Set(["linear", "line-duotone", "broken"]);

function compose({ base, mark }, v) {
  const k = 0.74;
  let b = body(`${base}-${v}`);
  // Keep the stroke width the eye expects after scaling down.
  if (STROKED.has(v)) b = b.replace(/stroke-width="1\.5"/g, `stroke-width="${(1.5 / k).toFixed(2)}"`);
  const w = STROKED.has(v) ? 1.5 : 2;
  return `<g transform="translate(0.4 0.2) scale(${k})">${b}</g><path fill="none" stroke="currentColor" stroke-linecap="round" stroke-width="${w}" d="${mark}"/>`;
}

const out = {};
const missing = [];
for (const [logical, candidates] of Object.entries(MAP)) {
  const base = candidates.find((c) => VARIANTS.every((v) => body(`${c}-${v}`)));
  if (!base) {
    missing.push(logical);
    continue;
  }
  out[logical] = Object.fromEntries(VARIANTS.map((v) => [v, body(`${base}-${v}`)]));
}
for (const [logical, spec] of Object.entries(COMPOSE)) {
  if (!VARIANTS.every((v) => body(`${spec.base}-${v}`))) {
    missing.push(logical);
    continue;
  }
  out[logical] = Object.fromEntries(VARIANTS.map((v) => [v, compose(spec, v)]));
}
if (missing.length) {
  console.error("missing icons:", missing.join(", "));
  process.exit(1);
}
// Our own drawn art (icons/drawn/*.svg): flat, multi-tone illustrations
// (the Classic layout's pen tiles). Colours are CSS variables (--art-*),
// so one file works in light and dark; --art-ink is the pen's colour.
const drawnDir = new URL("../icons/drawn/", import.meta.url);
const drawn = {};
for (const f of readdirSync(drawnDir).filter((f) => f.endsWith(".svg")).sort()) {
  const src = readFileSync(new URL(f, drawnDir), "utf8").trim();
  const m = /^<svg[^>]*viewBox="([^"]+)"[^>]*>([\s\S]*)<\/svg>$/.exec(src);
  if (!m) {
    console.error(`icons/drawn/${f}: expected one <svg viewBox="…"> element`);
    process.exit(1);
  }
  drawn[f.replace(/\.svg$/, "")] = { viewBox: m[1], body: m[2].trim() };
}
const header = `// Generated by scripts/gen-icons.mjs from Solar icons (CC BY 4.0, 480 Design)\n// and Rupok's own drawings (apps/board/icons/drawn/). Do not edit by hand.\n`;
writeFileSync(
  new URL("../src/icons.generated.ts", import.meta.url),
  `${header}export type IconVariant = ${VARIANTS.map((v) => JSON.stringify(v)).join(" | ")};\nexport type IconName = ${Object.keys(out).map((k) => JSON.stringify(k)).join(" | ")};\nexport const ICONS: Record<IconName, Record<IconVariant, string>> = ${JSON.stringify(out)};\nexport type DrawnName = ${Object.keys(drawn).map((k) => JSON.stringify(k)).join(" | ")};\nexport const DRAWN: Record<DrawnName, { viewBox: string; body: string }> = ${JSON.stringify(drawn)};\n`,
);
console.log(`wrote ${Object.keys(out).length} icons × ${VARIANTS.length} variants and ${Object.keys(drawn).length} drawings`);
