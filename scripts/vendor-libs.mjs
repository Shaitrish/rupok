#!/usr/bin/env node
// Vendors the offline Smart Object libraries (`rupoklib://<name>/<file>`,
// spec/smart-object.md "Offline libraries") into apps/board/public/rupoklib/,
// plus the theme fonts the host injects into objects (rupoklib/fonts/).
//
//   cd apps/board && bun install && bun run vendor
//
// The libraries are pinned devDependencies of apps/board; this script copies
// the files the spec guarantees (plus a few extras) out of node_modules, so the
// board serves them from its own origin at /rupoklib/. The output is generated
// and git-ignored. Each library keeps its own licence: the licence file is
// copied next to it and listed in rupoklib/README.md.
//
// No dependencies: Node 20+ standard library only.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const BOARD = path.join(ROOT, "apps/board");
const MODULES = path.join(BOARD, "node_modules");
const OUT = path.join(BOARD, "public/rupoklib");
const OBJECT_RS = path.join(ROOT, "crates/rupok-format/src/object.rs");
const OBJECT_FONTS_TS = path.join(BOARD, "src/objectFonts.ts");

/**
 * Theme fonts the host injects into every object (apps/board/src/objectFonts.ts,
 * spec/smart-object.md "Fonts"). They are not a rupoklib library (objects never
 * reference them; the host's @font-face rules do), so they are not in LIBS.
 * The file names come from objectFonts.ts; each one must start with a prefix below.
 */
const FONTS = [
  { family: "Inter (variable)", pkg: "@fontsource-variable/inter", prefix: "inter-", license: "LICENSE", out: "LICENSE-Inter.txt" },
  { family: "Hind Siliguri", pkg: "@fontsource/hind-siliguri", prefix: "hind-siliguri-", license: "LICENSE", out: "LICENSE-HindSiliguri.txt" },
];

/** three.js addons vendored under rupoklib://three/addons/. Their bare
 * `from "three"` imports are rewritten to a relative path, so they work in a
 * sandboxed object without an import map. Only files without other imports. */
const THREE_ADDONS = [
  "controls/OrbitControls.js",
  "controls/TrackballControls.js",
  "renderers/CSS2DRenderer.js",
  "environments/RoomEnvironment.js",
];

/**
 * name:    the rupoklib name (must match LIBS in crates/rupok-format/src/object.rs)
 * pkg:     npm package (a pinned devDependency of apps/board)
 * files:   [source inside the package, destination inside rupoklib/<name>/]
 * dirs:    whole directories to copy, same shape
 * license: licence file inside the package, or null (see note)
 */
const LIBS = [
  {
    name: "three",
    pkg: "three",
    use: 'ES module: import * as THREE from "rupoklib://three/three.module.js"',
    files: [
      ["build/three.module.js", "three.module.js"],
      ["build/three.core.js", "three.core.js"], // imported by three.module.js
    ],
    addons: THREE_ADDONS,
    license: "LICENSE",
  },
  {
    name: "d3",
    pkg: "d3",
    use: "global d3",
    files: [["dist/d3.min.js", "d3.min.js"]],
    license: "LICENSE",
  },
  {
    name: "katex",
    pkg: "katex",
    use: "global katex; katex.min.css loads fonts/",
    files: [
      ["dist/katex.min.js", "katex.min.js"],
      ["dist/katex.min.css", "katex.min.css"],
      ["dist/contrib/auto-render.min.js", "auto-render.min.js"],
    ],
    dirs: [["dist/fonts", "fonts"]],
    license: "LICENSE",
  },
  {
    name: "mermaid",
    pkg: "mermaid",
    use: "global mermaid",
    files: [["dist/mermaid.min.js", "mermaid.min.js"]],
    license: "LICENSE",
  },
  {
    name: "chart.js",
    pkg: "chart.js",
    use: "global Chart",
    files: [["dist/chart.umd.min.js", "chart.umd.min.js"]],
    license: "LICENSE.md",
  },
  {
    name: "gsap",
    pkg: "gsap",
    use: "global gsap",
    files: [["dist/gsap.min.js", "gsap.min.js"]],
    // GSAP ships no licence file; its terms are in the file header.
    license: null,
    licenseNote: "GSAP Standard License, https://gsap.com/standard-license (see the header of gsap.min.js)",
  },
  {
    name: "p5",
    pkg: "p5",
    use: "global p5 (global mode works too)",
    files: [["lib/p5.min.js", "p5.min.js"]],
    license: "license.txt",
  },
  {
    name: "matter-js",
    pkg: "matter-js",
    use: "global Matter",
    files: [["build/matter.min.js", "matter.min.js"]],
    license: "LICENSE",
  },
  {
    name: "animejs",
    pkg: "animejs",
    use: "global anime (v4 API: anime.animate, anime.createTimeline, anime.stagger)",
    files: [["dist/bundles/anime.umd.min.js", "anime.min.js"]],
    license: "LICENSE.md",
  },
  {
    name: "tailwind",
    pkg: "@tailwindcss/browser",
    use: "Tailwind browser build",
    files: [["dist/index.global.js", "tailwind.js"]],
    license: "LICENSE",
  },
  {
    name: "lucide",
    pkg: "lucide",
    use: "global lucide; lucide.createIcons()",
    files: [["dist/umd/lucide.min.js", "lucide.min.js"]],
    license: "LICENSE",
  },
];

const errors = [];
const fail = (msg) => errors.push(msg);

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

/** LIBS from the Rust validator, so the two lists cannot drift apart. */
function rustLibs() {
  const src = fs.readFileSync(OBJECT_RS, "utf8");
  const m = src.match(/pub const LIBS: &\[&str\] = &\[([\s\S]*?)\];/);
  if (!m) throw new Error(`LIBS not found in ${OBJECT_RS}`);
  return [...m[1].matchAll(/"([^"]+)"/g)].map((x) => x[1]);
}

/** Every file this run writes; anything else under OUT is pruned at the end. */
const written = new Set();

function writeFile(to, data) {
  fs.mkdirSync(path.dirname(to), { recursive: true });
  fs.writeFileSync(to, data);
  written.add(to);
}

function copyFile(from, to) {
  fs.mkdirSync(path.dirname(to), { recursive: true });
  fs.copyFileSync(from, to);
  written.add(to);
}

/** Delete files under `dir` this run did not write, then empty directories.
 * Updating in place (instead of deleting OUT first) keeps a running Vite dev
 * server's public-file index intact when the script is re-run. */
function prune(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      prune(p);
      if (!fs.readdirSync(p).length) fs.rmdirSync(p);
    } else if (!written.has(p)) fs.rmSync(p, { force: true });
  }
}

function copyDir(from, to) {
  for (const entry of fs.readdirSync(from, { withFileTypes: true })) {
    const a = path.join(from, entry.name);
    const b = path.join(to, entry.name);
    if (entry.isDirectory()) copyDir(a, b);
    else if (entry.isFile()) copyFile(a, b);
  }
}

function dirSize(dir) {
  let n = 0;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name);
    n += entry.isDirectory() ? dirSize(p) : fs.statSync(p).size;
  }
  return n;
}

const mib = (n) => `${(n / 1048576).toFixed(2)} MiB`;

// ------------------------------------------------------------------ checks

const pinned = readJson(path.join(BOARD, "package.json")).devDependencies ?? {};
const expected = rustLibs();
const ours = LIBS.map((l) => l.name);
for (const n of expected) if (!ours.includes(n)) fail(`library "${n}" is in LIBS (object.rs) but not vendored here`);
for (const n of ours) if (!expected.includes(n)) fail(`library "${n}" is vendored here but missing from LIBS (object.rs)`);

// ------------------------------------------------------------------ copy

fs.mkdirSync(OUT, { recursive: true });

const rows = [];
for (const lib of LIBS) {
  const pkgDir = path.join(MODULES, lib.pkg);
  const pkgJson = path.join(pkgDir, "package.json");
  if (!fs.existsSync(pkgJson)) {
    fail(`${lib.pkg} is not installed (run \`bun install\` in apps/board)`);
    continue;
  }
  const meta = readJson(pkgJson);
  const want = pinned[lib.pkg];
  if (!want) fail(`${lib.pkg} is not a devDependency of apps/board`);
  else if (want !== meta.version) fail(`${lib.pkg}: package.json pins ${want} but ${meta.version} is installed`);

  const dest = path.join(OUT, lib.name);
  for (const [from, to] of lib.files) {
    const src = path.join(pkgDir, from);
    if (!fs.existsSync(src)) fail(`${lib.pkg}@${meta.version}: missing ${from}`);
    else copyFile(src, path.join(dest, to));
  }
  for (const [from, to] of lib.dirs ?? []) {
    const src = path.join(pkgDir, from);
    if (!fs.existsSync(src)) fail(`${lib.pkg}@${meta.version}: missing ${from}/`);
    else copyDir(src, path.join(dest, to));
  }
  for (const rel of lib.addons ?? []) {
    const src = path.join(pkgDir, "examples/jsm", rel);
    if (!fs.existsSync(src)) {
      fail(`${lib.pkg}@${meta.version}: missing addon ${rel}`);
      continue;
    }
    const outFile = path.join(dest, "addons", rel);
    let code = fs.readFileSync(src, "utf8");
    let core = path.posix.relative(path.posix.dirname(`addons/${rel}`), "three.module.js");
    if (!core.startsWith(".")) core = `./${core}`;
    code = code.replace(/(\bfrom\s*)(['"])three\2/g, `$1$2${core}$2`);
    // Any other import would break in the sandbox: refuse rather than ship it.
    const stray = code.match(/^\s*(?:import|export)[^;]*?\bfrom\s*['"](?!\.)[^'"]+['"]/m);
    if (stray) fail(`three addon ${rel} has an unresolved import: ${stray[0].trim()}`);
    writeFile(outFile, code);
  }
  let licence = lib.licenseNote ?? meta.license ?? "see package";
  if (lib.license) {
    const src = path.join(pkgDir, lib.license);
    if (!fs.existsSync(src)) fail(`${lib.pkg}: missing licence file ${lib.license}`);
    else {
      copyFile(src, path.join(dest, "LICENSE.txt"));
      licence = `${meta.license} (\`${lib.name}/LICENSE.txt\`)`;
    }
  }
  const entries = lib.files.map(([, to]) => `\`rupoklib://${lib.name}/${to}\``).join("<br>");
  rows.push({ name: lib.name, pkg: lib.pkg, version: meta.version, entries, use: lib.use, licence, size: dirSize(dest) });
}

// ------------------------------------------------------------------ fonts

/** woff2 file names the host's @font-face rules reference (objectFonts.ts). */
function hostFontFiles() {
  const src = fs.readFileSync(OBJECT_FONTS_TS, "utf8");
  const files = [...new Set([...src.matchAll(/file:\s*"([^"]+\.woff2)"/g)].map((x) => x[1]))];
  if (!files.length) throw new Error(`no font files found in ${OBJECT_FONTS_TS}`);
  return files;
}

const FONTS_OUT = path.join(OUT, "fonts");
const fontRows = [];
const fontFiles = hostFontFiles();
for (const file of fontFiles) {
  if (!FONTS.some((f) => file.startsWith(f.prefix))) fail(`font ${file} (objectFonts.ts) has no package in FONTS`);
}
for (const font of FONTS) {
  const pkgDir = path.join(MODULES, font.pkg);
  const pkgJson = path.join(pkgDir, "package.json");
  const files = fontFiles.filter((f) => f.startsWith(font.prefix));
  if (!files.length) {
    fail(`font package ${font.pkg} is in FONTS but objectFonts.ts uses none of its files`);
    continue;
  }
  if (!fs.existsSync(pkgJson)) {
    fail(`${font.pkg} is not installed (run \`bun install\` in apps/board)`);
    continue;
  }
  const meta = readJson(pkgJson);
  let size = 0;
  for (const file of files) {
    const src = path.join(pkgDir, "files", file);
    if (!fs.existsSync(src)) fail(`${font.pkg}@${meta.version}: missing files/${file}`);
    else {
      copyFile(src, path.join(FONTS_OUT, file));
      size += fs.statSync(src).size;
    }
  }
  const lic = path.join(pkgDir, font.license);
  if (!fs.existsSync(lic)) fail(`${font.pkg}: missing licence file ${font.license}`);
  else copyFile(lic, path.join(FONTS_OUT, font.out));
  fontRows.push({ ...font, version: meta.version, licence: meta.license ?? "OFL-1.1", files, size });
}

// ------------------------------------------------------------------ notice

prune(OUT);
const total = dirSize(OUT);
const readme = `# rupoklib: offline libraries for Smart Objects

Generated by \`scripts/vendor-libs.mjs\`. Do not edit; do not commit (git-ignored).
Re-run with \`bun run vendor\` in \`apps/board\` after changing a pinned version.

Smart Objects reference these as \`rupoklib://<name>/<file>\`
(spec/smart-object.md, "Offline libraries"). The board serves them from
\`/rupoklib/\` on its own origin, so lessons work without internet.

Each library is the unmodified upstream build and keeps its own licence. The only
change is in \`three/addons/\`: the bare \`"three"\` import is rewritten to a
relative path so the addons load without an import map.

| Name | npm package | Version | Files | Use as | Licence | Size |
|---|---|---|---|---|---|---|
${rows.map((r) => `| \`${r.name}\` | \`${r.pkg}\` | ${r.version} | ${r.entries} | ${r.use} | ${r.licence} | ${mib(r.size)} |`).join("\n")}

three.js addons: ${THREE_ADDONS.map((a) => `\`rupoklib://three/addons/${a}\``).join(", ")}.

## Fonts

\`fonts/\` holds the theme fonts. They are not a library: objects never reference
them. The host injects \`@font-face\` rules for them into every object
(\`apps/board/src/objectFonts.ts\`), so \`var(--rupok-font-body)\` renders in Inter and
Hind Siliguri even on machines without those fonts installed. Unmodified
fontsource subsets, licensed under the SIL Open Font License 1.1.

| Family | npm package | Version | Files | Licence | Size |
|---|---|---|---|---|---|
${fontRows.map((r) => `| ${r.family} | \`${r.pkg}\` | ${r.version} | ${r.files.map((f) => `\`fonts/${f}\``).join("<br>")} | ${r.licence} (\`fonts/${r.out}\`) | ${mib(r.size)} |`).join("\n")}

Total: ${mib(total)} uncompressed.
`;
writeFile(path.join(OUT, "README.md"), readme);

// ------------------------------------------------------------------ report

for (const r of rows) console.log(`  ${r.name.padEnd(10)} ${r.version.padEnd(9)} ${mib(r.size).padStart(10)}`);
for (const r of fontRows) console.log(`  fonts      ${r.family} ${r.version}, ${r.files.length} files, ${mib(r.size)}`);
console.log(`rupoklib: ${rows.length} libraries + ${fontFiles.length} font files, ${mib(total)} -> ${path.relative(ROOT, OUT)}/`);
if (errors.length) {
  for (const e of errors) console.error(`error: ${e}`);
  process.exit(1);
}
