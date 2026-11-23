#!/usr/bin/env node
// Renders the gallery thumbnails of the built-in Smart Object presets
// (apps/board/src/presets.ts) into apps/board/public/presets/:
// <id>-light.webp (white page) and <id>-dark.webp (blackboard page), 640x360.
//
//   cd apps/board && bun run vendor               # objects need rupoklib + fonts
//   node ../../scripts/preset-thumbs.mjs [id ...]  # all presets, or just these
//
// Each object runs exactly as on the board: an opaque-origin sandboxed iframe
// with the object CSP, the theme fonts and the SDK, rendered at 1920x1080 at
// its last step, then downscaled in the browser and encoded as WebP.
//
// Needs Node 22.18+ (it imports presets.ts and objectFonts.ts with type
// stripping), Chrome or Chromium (CHROME=/path/to/chrome, default
// /usr/bin/google-chrome) and playwright-core. playwright-core is not a
// project dependency: it is resolved from PLAYWRIGHT_CORE=/path/to/playwright-core,
// then from the current directory, then normally.

import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const BOARD = path.join(ROOT, "apps/board");
const PUBLIC = path.join(BOARD, "public");
const OUT = path.join(PUBLIC, "presets");
const SDK = fs.readFileSync(path.join(ROOT, "packages/object-sdk/sdk.js"), "utf8");

const { PRESETS } = await import(pathToFileURL(path.join(BOARD, "src/presets.ts")).href);
const { objectFontStyle } = await import(pathToFileURL(path.join(BOARD, "src/objectFonts.ts")).href);

/** Render size (the slide frame) and thumbnail size. */
const VIEW = { width: 1920, height: 1080 };
const THUMB = { width: 640, height: 360 };
/** How long to let build animations settle after `init`. */
const SETTLE_MS = Number(process.env.SETTLE_MS || 4000);
/** Step to show instead of the last one, where the last step is not the best picture. */
const THUMB_STEP = {
  "binary-counter": 9, // all bits and the sum; the last step auto-counts
  "quiz-networking": 0, // the first question; the last step is the empty score
};
/** Saved state to show, where the default state is not the best picture. */
const THUMB_STATE = {
  "binary-counter": { value: 42 },
};

async function loadPlaywright() {
  const tries = [];
  if (process.env.PLAYWRIGHT_CORE) tries.push(process.env.PLAYWRIGHT_CORE);
  try {
    tries.push(createRequire(path.join(process.cwd(), "noop.js")).resolve("playwright-core"));
  } catch { /* not installed here */ }
  tries.push("playwright-core");
  for (const spec of tries) {
    try {
      const mod = await import(spec.startsWith("/") ? pathToFileURL(spec).href : spec);
      return mod.chromium ? mod : mod.default; // CommonJS entry: exports on default
    } catch { /* next */ }
  }
  console.error("playwright-core not found. Install it somewhere (npm i playwright-core) and run this from there, or set PLAYWRIGHT_CORE.");
  process.exit(1);
}

// ------------------------------------------------------------------ server
// Serves the harness at / and public/rupoklib/ with CORS, like the board.

const MIME = {
  ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json",
  ".woff2": "font/woff2", ".woff": "font/woff", ".ttf": "font/ttf", ".svg": "image/svg+xml", ".png": "image/png",
};

const HARNESS = `<!doctype html><html><head><meta charset="utf-8"><link rel="icon" href="data:,"><style>
html, body { margin: 0; height: 100%; overflow: hidden; }
iframe { border: 0; width: 100%; height: 100%; display: block; }
</style></head><body><iframe id="f" sandbox="allow-scripts"></iframe><script>
window.start = (doc, init) => new Promise((resolve) => {
  const f = document.getElementById("f");
  document.body.style.background = init.theme["color.bg"];
  addEventListener("message", (e) => {
    const m = e.data;
    if (!m || m.rupok !== 1 || e.source !== f.contentWindow) return;
    if (m.type === "hello") {
      f.contentWindow.postMessage({ rupok: 1, type: "init", id: "thumb", mode: "present", ...init }, "*");
      resolve();
    }
  });
  f.srcdoc = doc;
});
</script></body></html>`;

function serve() {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, "http://x");
    if (url.pathname === "/") {
      res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
      return res.end(HARNESS);
    }
    const rel = decodeURIComponent(url.pathname);
    const file = path.join(PUBLIC, rel);
    if (!rel.startsWith("/rupoklib/") || !file.startsWith(path.join(PUBLIC, "rupoklib")) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
      res.writeHead(404);
      return res.end();
    }
    res.writeHead(200, {
      "Content-Type": MIME[path.extname(file)] ?? "application/octet-stream",
      "Access-Control-Allow-Origin": "*",
    });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server)));
}

// ------------------------------------------------------------------ object

/** The object document, as apps/board/src/objects.ts builds it. */
function srcdoc(html, lib) {
  const csp = [
    "default-src 'none'",
    `script-src 'unsafe-inline' 'unsafe-eval' blob: data: ${lib}`,
    `style-src 'unsafe-inline' data: ${lib}`,
    `img-src data: blob: ${lib}`,
    `font-src data: ${lib}`,
    "media-src data: blob:",
    "connect-src 'none'",
    "form-action 'none'",
  ].join("; ");
  const head =
    `<meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${csp}">` +
    `<meta name="viewport" content="width=device-width, initial-scale=1">` +
    objectFontStyle(lib) +
    `<meta name="rupok-lib" content="${lib}">` + // where the SDK loads KaTeX from (built-in LaTeX)
    `<script>${SDK.replaceAll("</script", "<\\/script")}</script>`;
  const body = html.replaceAll("rupoklib://", lib);
  return /<head[^>]*>/i.test(body) ? body.replace(/<head[^>]*>/i, (m) => m + head) : `<!doctype html><html><head>${head}</head><body>${body}</body></html>`;
}

/** The theme objects.ts sends for a white page and a blackboard page. */
function theme(scheme) {
  const dark = scheme === "dark";
  const font = '"Inter Variable", Inter, "Hind Siliguri", system-ui, "Segoe UI", Roboto, sans-serif';
  return {
    scheme,
    "color.bg": dark ? "#17191c" : "#ffffff",
    "color.surface": dark ? "rgba(255, 255, 255, 0.06)" : "rgba(17, 20, 24, 0.04)",
    "color.line": dark ? "rgba(255, 255, 255, 0.14)" : "rgba(17, 20, 24, 0.12)",
    "color.fg": dark ? "#eef0f2" : "#111418",
    "color.muted": dark ? "#a6adb7" : "#5f6670",
    "color.accent": dark ? "#3ee6b0" : "#0ca678",
    // contrast-safe accent companions, as the board sends them (objects.ts ACCENTS, mint)
    "color.accent-text": dark ? "#63e6be" : "#087f5b",
    "color.on-accent": "#04241b",
    "color.accent-soft": dark ? "#0b2b23" : "#e6fcf5",
    "color.accent2": dark ? "#74c0fc" : "#1c7ed6",
    "color.success": dark ? "#69db7c" : "#2b8a3e",
    "color.danger": dark ? "#ff8787" : "#e03131",
    "font.body": font,
    "font.heading": font,
    "font.mono": 'ui-monospace, "Cascadia Mono", Menlo, Consolas, monospace',
    radius: "12px",
  };
}

// ------------------------------------------------------------------ render

const only = process.argv.slice(2);
const presets = only.length ? PRESETS.filter((p) => only.includes(p.id)) : PRESETS;
if (only.length && presets.length !== only.length) {
  console.error(`unknown preset id; known: ${PRESETS.map((p) => p.id).join(", ")}`);
  process.exit(1);
}
if (!fs.existsSync(path.join(PUBLIC, "rupoklib/fonts"))) {
  console.error("public/rupoklib/fonts is missing: run `bun run vendor` in apps/board first.");
  process.exit(1);
}

const { chromium } = await loadPlaywright();
const server = await serve();
const origin = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch({
  executablePath: process.env.CHROME || "/usr/bin/google-chrome",
  headless: true,
  args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist", "--hide-scrollbars"],
});
fs.mkdirSync(OUT, { recursive: true });

async function render(preset, scheme) {
  const page = await browser.newPage({ viewport: VIEW, deviceScaleFactor: 1 });
  const problems = [];
  page.on("pageerror", (e) => problems.push(e.message));
  page.on("console", (m) => m.type() === "error" && problems.push(m.text()));
  page.on("requestfailed", (r) => problems.push(`${r.url()} ${r.failure()?.errorText}`));
  page.on("response", (r) => r.status() >= 400 && problems.push(`${r.status()} ${r.url()}`));
  await page.goto(`${origin}/`);
  const html = fs.readFileSync(path.join(ROOT, "examples/objects", `${preset.id}.html`), "utf8");
  const step = THUMB_STEP[preset.id] ?? preset.steps;
  await page.evaluate(([doc, init]) => window.start(doc, init), [srcdoc(html, `${origin}/rupoklib/`), { step, steps: preset.steps, state: THUMB_STATE[preset.id] ?? null, theme: theme(scheme) }]);
  const frame = page.frames().find((f) => f !== page.mainFrame());
  await frame?.evaluate(() => document.fonts.ready.then(() => true)).catch(() => {});
  await page.waitForTimeout(SETTLE_MS);
  const png = await page.screenshot({ type: "png" });
  // Downscale with high-quality resampling and encode WebP in the browser.
  const webp = await page.evaluate(async ([b64, w, h]) => {
    const blob = await (await fetch(`data:image/png;base64,${b64}`)).blob();
    const bmp = await createImageBitmap(blob, { resizeWidth: w, resizeHeight: h, resizeQuality: "high" });
    const canvas = new OffscreenCanvas(w, h);
    canvas.getContext("2d").drawImage(bmp, 0, 0);
    const out = await canvas.convertToBlob({ type: "image/webp", quality: 0.82 });
    const bytes = new Uint8Array(await out.arrayBuffer());
    let s = "";
    for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
    return btoa(s);
  }, [png.toString("base64"), THUMB.width, THUMB.height]);
  const file = path.join(OUT, `${preset.id}-${scheme}.webp`);
  fs.writeFileSync(file, Buffer.from(webp, "base64"));
  await page.close();
  const kb = (fs.statSync(file).size / 1024).toFixed(1);
  console.log(`  ${path.relative(ROOT, file)}  ${kb} KiB${problems.length ? `  (${problems.length} console problems: ${problems[0]})` : ""}`);
  return problems.length;
}

let problems = 0;
const jobs = presets.flatMap((p) => [[p, "light"], [p, "dark"]]);
const queue = [...jobs];
await Promise.all(
  Array.from({ length: 4 }, async () => {
    for (let job = queue.shift(); job; job = queue.shift()) {
      const n = await render(...job);
      problems += n;
    }
  }),
);
await browser.close();
server.close();
console.log(`preset thumbnails: ${jobs.length} files -> ${path.relative(ROOT, OUT)}/`);
if (problems) console.warn(`warning: ${problems} console problems while rendering (see above)`);
