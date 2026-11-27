#!/usr/bin/env node
// Pen-to-pixel latency and smoothness harness for the board (docs/performance.md).
//
// Drives the production build (apps/board/dist) in Chrome with synthetic pen
// input through the DevTools protocol (Input.dispatchMouseEvent with
// pointerType "pen", paced at --rate Hz), then measures, per stroke sample:
//
//   handler   event timeStamp → the board's pointermove handler finished
//   frame     event timeStamp → the GPU frame that contains the sample is
//             submitted (end of the board's requestAnimationFrame loop)
//   tip gap   at each frame, how far the ink's tip (read back from the canvas)
//             trails the newest pen sample the frame was given ("smoothing
//             lag"), and the newest sample the browser had received
//             (pointerrawupdate) — in px and in ms at the pen's speed
//   pen-up    pixels that change between the last wet frame and the dry
//             stroke (the "compute then paste" jump)
//   samples   moves sent vs coalesced samples received (dropped or merged)
//   jank      long tasks and the slowest frame while writing, including the
//             first frames of a stroke that starts right after another
//
// The compositor's own latency (submit → photons) is outside a page's reach;
// --present estimates it with the Event Timing API on a headed browser.
//
//   node bench/latency/pen-latency.mjs                     # real GPU (Vulkan), headless
//   node bench/latency/pen-latency.mjs --swiftshader --check   # CI budgets
//   node bench/latency/pen-latency.mjs --dist /path/to/dist --json out.json
//
// Options: --dist DIR, --chrome PATH, --swiftshader, --headed, --present,
// --rate HZ (240), --speed PX_PER_S (400), --strokes N (12), --smoothing S,
// --smart (Smart shapes "auto"), --brush NAME (pen style, e.g. fountain),
// --json FILE, --check, --label TEXT,
// --lesson FILE.rpk (write over its first page, e.g. one with a page design),
// --prefs JSON (board settings, e.g. '{"wetLayer":"off","renderScale":"100"}'),
// --cpu N (throttle the page's main thread N times).
//
// --4k: the big-screen test (docs/performance.md, "Big screens and weak
// GPUs"). A 3840×2160 board (--dpr 2: 1920×1080 at 2×), handwriting and
// long strokes for --seconds S (8), and per frame: the interval to the next
// one while the pen is down (the rate that reaches the screen), which canvas
// it presented, how much of the screen the compositor had to redraw (from a
// trace) and, on Linux, the GPU time Chrome used. Frame rates only mean
// something with a real swap chain behind them: --wayland NAME runs the
// browser full screen on that Wayland display. virtual-display.sh starts a
// headless GNOME with a 4K virtual monitor for a set time, with its own
// home and runtime directories so nothing of it touches the desktop session:
//   bench/latency/virtual-display.sh /tmp/v4k 120 &
//   node bench/latency/pen-latency.mjs --4k --wayland wl-4k \
//     --runtime-dir /tmp/v4k/rt --home /tmp/v4k/home
// --writing is the same test at 1600×900. Its --check budgets are about the
// pipeline (frames that draw the whole canvas, screen area redrawn), so
// they hold under SwiftShader, where CI runs them (4K in software is a
// frame or two per second: too few to count).

import { chromium } from "playwright-core";
import { createServer } from "node:http";
import { readFile, stat, writeFile } from "node:fs/promises";
import { readFileSync, readdirSync } from "node:fs";
import { extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = fileURLToPath(new URL(".", import.meta.url));
const argv = process.argv.slice(2);
const flag = (n) => argv.includes(`--${n}`);
const opt = (n, d) => {
  const i = argv.indexOf(`--${n}`);
  return i >= 0 && argv[i + 1] !== undefined ? argv[i + 1] : d;
};
const DIST = resolve(opt("dist", join(here, "../../apps/board/dist")));
const RATE = Number(opt("rate", 240));
const SPEED = Number(opt("speed", 400));
const STROKES = Number(opt("strokes", 12));
const SMOOTHING = opt("smoothing", null);
const LABEL = opt("label", "");
const BRUSH = opt("brush", null);
// --writing: the frame test at the usual 1600×900; --4k: the same at 3840×2160.
const FOURK = flag("4k") || flag("writing");
const BOARD = flag("4k") ? [3840, 2160] : [1600, 900];
const DPR = Number(opt("dpr", 1));
const SECONDS = Number(opt("seconds", 8));
const WAYLAND = opt("wayland", null);

// Budgets for --4k --check: while writing, the board must not draw or have
// the compositor redraw the whole screen.
const BUDGETS_4K = [
  // Before the wet tile these were 100 % and about 85 %. With it, a long
  // stroke still redraws the canvas each time it leaves the tile, and slow
  // (software) frames make those a larger share.
  ["writing frames that drew the whole canvas (%)", (r) => r.fullFramePct, 40],
  ["screen redrawn per frame while writing (%)", (r) => r.damagePct, 50],
  ["pointermove handler p95 (ms)", (r) => r.processMs.p95, 2.0],
];

// Budgets for --check. They hold under SwiftShader on a CI runner, so they
// test the pipeline (no smoothing lag at the tip, no pen-up jump, no dropped
// samples, nothing heavy on the pen path), not the GPU.
const BUDGETS = [
  ["tip gap behind newest sample p95 (px)", (r) => r.tipGapSamplePx.p95, 1.5],
  // Prediction may lead the pen by about a frame, not overshoot by two.
  ["tip ahead of newest sample p50 (ms of travel)", (r) => -r.tipGapSampleMs.p50, 16],
  ["pen-up changed pixels (% of stroke)", (r) => r.penUpChangedPct, 0.15],
  // Smoothing 0 is raw: the dry ink covers every pen sample.
  ["smoothing 0: sample → dry ink p95 (px)", (r) => (r.env.smoothing === "0" ? r.pathErrorPx.p95 : 0), 0.5],
  ["samples dropped (%)", (r) => r.droppedPct, 1.0],
  ["pointermove handler p95 (ms)", (r) => r.processMs.p95, 2.0],
  ["first frame of next stroke, extra (ms)", (r) => r.nextStrokeExtraMs, 33.4],
];

// ------------------------------------------------------------------ server

const MIME = {
  ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css",
  ".wasm": "application/wasm", ".json": "application/json", ".svg": "image/svg+xml", ".png": "image/png",
  ".woff2": "font/woff2", ".woff": "font/woff", ".ttf": "font/ttf", ".ico": "image/x-icon", ".txt": "text/plain",
};
async function serve(root) {
  const server = createServer(async (req, res) => {
    let p = decodeURIComponent(new URL(req.url, "http://x").pathname);
    if (p.endsWith("/")) p += "index.html";
    const file = join(root, p);
    if (!file.startsWith(root)) return res.writeHead(403).end();
    try {
      const s = await stat(file);
      if (s.isDirectory()) return res.writeHead(302, { location: p + "/" }).end();
      res.writeHead(200, { "content-type": MIME[extname(file)] ?? "application/octet-stream", "access-control-allow-origin": "*" });
      res.end(await readFile(file));
    } catch {
      res.writeHead(404).end();
    }
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  return server;
}

// ------------------------------------------------------------ in-page probe

// Runs before the app. Wraps requestAnimationFrame to find the end of the
// board's frame loop (the callback named "loop") and reads the canvas back.
function probe() {
  const L = (window.__lat = {
    on: false, pending: [], samples: [], frames: [], moves: 0, received: 0, raw: null,
    captureNext: null, captures: {}, gaps: [], proc: [], longTasks: [], downs: [], firstFrameAfterDown: [], row: -1, loops: 0,
    down: false, lastLoop: 0, intervals: [], kinds: [0, 0, 0, 0], prevFull: false, fullIntervals: [],
  });
  const ink = () => document.querySelector("canvas.ink");
  // The wet tile: a small canvas over the ink canvas with the stroke being
  // written. `x`, `y`: its corner in the ink canvas's pixels.
  const wetTile = () => {
    const w = document.querySelector("canvas.ink-wet"), c = ink();
    if (!w || !c || getComputedStyle(w).display === "none") return null;
    const a = c.getBoundingClientRect(), b = w.getBoundingClientRect();
    const k = c.width / a.width;
    return { c: w, x: Math.round((b.left - a.left) * k), y: Math.round((b.top - a.top) * k) };
  };
  let cap = null;
  // One row of the canvas. WebGL: readPixels on the board's own context (the
  // default framebuffer the frame was just presented to), restoring the
  // bindings; drawImage of a WebGL canvas would copy the whole canvas.
  const readRow = (c, y) => {
    const w = c.width;
    if (y < 0 || y >= c.height) return -1;
    const rowBuf = new Uint8Array(w * 4);
    const gl = c.getContext("webgl2");
    if (gl) {
      const fb = gl.getParameter(gl.READ_FRAMEBUFFER_BINDING);
      const pb = gl.getParameter(gl.PIXEL_PACK_BUFFER_BINDING);
      gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
      gl.bindBuffer(gl.PIXEL_PACK_BUFFER, null);
      gl.readPixels(0, c.height - 1 - y, w, 1, gl.RGBA, gl.UNSIGNED_BYTE, rowBuf);
      gl.bindFramebuffer(gl.READ_FRAMEBUFFER, fb);
      gl.bindBuffer(gl.PIXEL_PACK_BUFFER, pb);
    } else {
      if (!cap) cap = document.createElement("canvas").getContext("2d", { willReadFrequently: true });
      if (cap.canvas.width !== w) { cap.canvas.width = w; cap.canvas.height = 1; }
      cap.clearRect(0, 0, w, 1);
      cap.drawImage(c, 0, y, w, 1, 0, 0, w, 1);
      rowBuf.set(cap.getImageData(0, 0, w, 1).data);
    }
    for (let x = w - 1; x >= 0; x--) if (rowBuf[x * 4 + 3] > 96) return x;
    return -1;
  };
  // The rightmost ink in row `y` of the board: the ink canvas or the tile.
  const inkEnd = (c, y) => {
    const t = wetTile();
    const inTile = t ? readRow(t.c, y - t.y) : -1;
    return Math.max(readRow(c, y), inTile >= 0 ? inTile + t.x : -1);
  };
  // The board's ink as the browser composites it: the tile over the canvas.
  const readAll = (c) => {
    const k = document.createElement("canvas");
    k.width = c.width; k.height = c.height;
    const g = k.getContext("2d", { willReadFrequently: true });
    g.drawImage(c, 0, 0);
    const t = wetTile();
    if (t) g.drawImage(t.c, t.x, t.y);
    return Array.from(g.getImageData(0, 0, k.width, k.height).data.filter((_, i) => i % 4 === 3));
  };
  addEventListener("pointerrawupdate", (e) => {
    if (L.on && e.pointerType === "pen") L.raw = { x: e.clientX, y: e.clientY, t: performance.now() };
  }, { capture: true });
  addEventListener("pointerdown", (e) => {
    L.down = true;
    L.lastLoop = 0;
    if (L.on && e.pointerType === "pen" && L.row < 0 && !L.noTime) L.downs.push({ ts: e.timeStamp, framed: false });
  }, { capture: true });
  addEventListener("pointerup", (e) => {
    L.down = false;
    if (L.on && e.pointerType === "pen") L.upSeen = true;
  }, { capture: true });
  addEventListener("pointermove", (e) => {
    if (!L.on || e.pointerType !== "pen" || !e.buttons) return;
    const now = performance.now();
    const list = e.getCoalescedEvents ? e.getCoalescedEvents() : [e];
    L.received += list.length || 1;
    L.cur = { at: now, list: (list.length ? list : [e]).map((c) => ({ ts: c.timeStamp, x: c.clientX, y: c.clientY })) };
  }, { capture: true });
  // Bubble phase: after the board's own handler on the input layer.
  addEventListener("pointermove", () => {
    if (!L.cur) return;
    const done = performance.now();
    for (const s of L.cur.list) L.pending.push({ ...s, handler: done - s.ts });
    L.proc.push(done - L.cur.at);
    L.cur = null;
  });
  try {
    new PerformanceObserver((l) => { for (const e of l.getEntries()) if (L.on) L.longTasks.push([Math.round(e.startTime), Math.round(e.duration)]); })
      .observe({ type: "longtask", buffered: false });
  } catch {}
  const raf = window.requestAnimationFrame.bind(window);
  window.requestAnimationFrame = (cb) => raf((t) => {
    const start = performance.now();
    // A canvas keeps its pixels only for the frame that drew it: a frame
    // that is read back whole must draw the ink canvas too, not the tile alone.
    if (cb.name === "loop" && L.captureNext) window.rupokPerf?.fullFrame?.();
    cb(t);
    if (cb.name !== "loop" || !L.on) return;
    L.loops++;
    const end = performance.now();
    // While the pen is down: the time between frames, and which canvas each
    // presented (1 the ink canvas, 2 the wet tile, 3 both; window.rupokPerf).
    const kind = L.pending.length ? (window.rupokPerf?.frameKind?.() ?? 1) : 0;
    if (L.down) {
      if (L.lastLoop) {
        L.intervals.push(t - L.lastLoop);
        // How long a frame that drew the whole ink canvas held up the next.
        if (L.prevFull) L.fullIntervals.push(t - L.lastLoop);
      }
      L.lastLoop = t;
      if (kind) L.kinds[kind & 3]++;
    }
    L.prevFull = (kind & 1) === 1;
    const c = ink();
    const dpr = c ? c.width / c.getBoundingClientRect().width : 1;
    if (L.row < 0) {
      for (const s of L.pending) L.samples.push({ frame: end - s.ts, handler: s.handler, full: (kind & 1) === 1 });
      for (const d of L.downs) if (!d.framed) { d.framed = true; L.firstFrameAfterDown.push(end - d.ts); }
      L.frames.push({ dur: end - start });
    }
    const newest = L.pending.at(-1);
    L.pending = [];
    if (c && L.row >= 0 && newest) {
      const inkX = inkEnd(c, Math.round(L.row * dpr));
      if (inkX >= 0) {
        L.gaps.push({
          gapSample: newest.x - inkX / dpr,
          gapRaw: L.raw ? L.raw.x - inkX / dpr : NaN,
        });
      }
    }
    // Read back only frames the board drew (an idle frame's buffer is blank):
    // the wet frame that renders the last samples, the dry one after pen-up.
    const drew = L.captureNext === "wet" ? !!newest : L.captureNext === "dry" ? L.upSeen : false;
    if (drew && c) { L.captures[L.captureNext] = readAll(c); L.captureNext = null; }
  });
}

// --------------------------------------------------------------------- run

const stats = (xs) => {
  const v = xs.filter(Number.isFinite).sort((a, b) => a - b);
  const q = (p) => (v.length ? v[Math.min(v.length - 1, Math.floor(p * v.length))] : NaN);
  return { n: v.length, p50: q(0.5), p95: q(0.95), max: v.at(-1) ?? NaN };
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
/** Busy-wait until hrtime `t` (ms) for steady input pacing. */
const until = async (t) => {
  while (performance.now() < t - 2) await sleep(1);
  while (performance.now() < t) {}
};

const server = await serve(DIST);
const port = server.address().port;
const gpuArgs = flag("swiftshader")
  ? ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"]
  : ["--enable-gpu", "--ignore-gpu-blocklist", "--use-angle=vulkan", "--enable-features=Vulkan"];
// --wayland NAME: a real window, full screen on that display (a clean
// environment: a desktop session's GTK settings can crash a second Chrome).
const wlEnv = WAYLAND && {
  HOME: opt("home", process.env.HOME), USER: process.env.USER, PATH: "/usr/bin:/bin", XDG_RUNTIME_DIR: opt("runtime-dir", process.env.XDG_RUNTIME_DIR),
  LANG: "C.UTF-8", XDG_DATA_DIRS: "/usr/local/share/:/usr/share/", GSETTINGS_BACKEND: "memory", WAYLAND_DISPLAY: WAYLAND,
};
const browser = await chromium.launch({
  executablePath: opt("chrome", process.env.CHROME_PATH || undefined),
  headless: !flag("headed") && !WAYLAND,
  env: wlEnv || process.env,
  args: [
    ...gpuArgs, "--disable-renderer-backgrounding", "--disable-background-timer-throttling",
    ...(WAYLAND ? ["--ozone-platform=wayland", "--enable-unsafe-webgpu", `--force-device-scale-factor=${DPR}`] : []),
  ],
});
const size = { width: BOARD[0] / (FOURK ? DPR : 1), height: BOARD[1] / (FOURK ? DPR : 1) };
const ctx = await browser.newContext(WAYLAND ? { viewport: null } : { viewport: size, deviceScaleFactor: FOURK ? DPR : 1 });
const page = await ctx.newPage();
if (WAYLAND) {
  const c = await ctx.newCDPSession(page);
  const { windowId } = await c.send("Browser.getWindowForTarget");
  await c.send("Browser.setWindowBounds", { windowId, bounds: { windowState: "fullscreen" } });
  await new Promise((r) => setTimeout(r, 800));
}
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
// Ink sharpness is fixed at 100 %: "Auto" resizes the canvas when frames are
// slow (they are, under SwiftShader), in the middle of the pixel comparisons.
// layoutChosen: the Modern layout, so numbers stay comparable with earlier
// releases (Classic, the default since v0.2.6, uses the same canvas and ink).
const prefs = { theme: "light", layoutChosen: true, smartShapes: flag("smart") ? "auto" : "off", showPages: FOURK, renderScale: "100", ...JSON.parse(opt("prefs", "{}")) };
if (SMOOTHING !== null) prefs.smoothing = Number(SMOOTHING);
if (BRUSH !== null) prefs.penBrush = BRUSH;
await page.addInitScript((v) => { if (window === window.top) localStorage.setItem("rupok.prefs", v); }, JSON.stringify(prefs));
await page.addInitScript(probe);
await page.goto(`http://127.0.0.1:${port}/board/`, { waitUntil: "load" });
await page.waitForSelector(".toolbar, .dock, .islands", { timeout: 60000 });
await page.waitForFunction(() => !document.querySelector(".loading"), null, { timeout: 60000 });
await sleep(Number(opt("settle", 3000)));
// The Library home may open over a fresh board: close it.
await page.keyboard.press("Escape");
// --lesson: write over that lesson's first page (e.g. one with a page design:
// a background image and a logo must not slow the pen).
const LESSON = opt("lesson", null);
if (LESSON) {
  const b64 = readFileSync(resolve(LESSON)).toString("base64");
  await page.evaluate((b64) => {
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    const dt = new DataTransfer();
    dt.items.add(new File([bytes], "lesson.rpk"));
    window.dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
  }, b64);
  await sleep(2000);
  await page.keyboard.press("Escape");
}
await page.keyboard.press("p");
await sleep(300);

const env = await page.evaluate(() => {
  const gl = document.createElement("canvas").getContext("webgl2");
  const ext = gl?.getExtension("WEBGL_debug_renderer_info");
  const c = document.querySelector("canvas.ink"), w = document.querySelector("canvas.ink-wet");
  return {
    gpu: gl ? gl.getParameter(ext ? ext.UNMASKED_RENDERER_WEBGL : gl.RENDERER) : "none",
    ua: navigator.userAgent.replace(/.*(Chrome\/[\d.]+).*/, "$1"),
    inkApi: !!navigator.ink,
    dpr: devicePixelRatio,
    screen: [innerWidth * devicePixelRatio, innerHeight * devicePixelRatio],
    canvas: c ? [c.width, c.height] : null,
    wetTile: w && getComputedStyle(w).display !== "none" ? [w.width, w.height] : null,
    backend: window.rupokPerf?.stats?.().backend ?? "",
  };
});

const cdp = await ctx.newCDPSession(page);
// --cpu N: the page's main thread N times slower (a weak processor).
if (opt("cpu", null)) await cdp.send("Emulation.setCPUThrottlingRate", { rate: Number(opt("cpu")) });
const frame = await page.locator(".page-frame").first().boundingBox();
const mouse = (type, x, y, force = 0.5) =>
  cdp.send("Input.dispatchMouseEvent", { type, x, y, button: "left", buttons: type === "mouseReleased" ? 0 : 1, clickCount: 1, pointerType: "pen", force });

/** One stroke along `path(u)`, u in 0..1, of length `len` px at SPEED px/s,
 * paced at RATE Hz. Returns the samples sent. */
async function stroke(path, len, { capture = false, pixels = false } = {}) {
  await page.evaluate((row) => { window.__lat.row = row; }, pixels ? path(0)[1] : -1);
  const dt = 1000 / RATE;
  const n = Math.max(2, Math.round((len / SPEED) * RATE));
  const inflight = [];
  const pts = [path(0)];
  await mouse("mousePressed", ...path(0));
  const t0 = performance.now();
  for (let i = 1; i <= n; i++) {
    await until(t0 + i * dt);
    if (capture && i === n) await page.evaluate(() => { window.__lat.captureNext = "wet"; });
    const p = path(i / n);
    pts.push(p);
    // A slight pressure swell, like real writing.
    inflight.push(mouse("mouseMoved", p[0], p[1], 0.4 + 0.3 * Math.sin((i / n) * Math.PI)));
  }
  await Promise.all(inflight);
  if (capture) {
    // The last wet frame (the pen still moving), then the dry stroke.
    await page.waitForFunction(() => window.__lat.captures.wet, null, { timeout: 5000 });
    await page.evaluate(() => { window.__lat.captureNext = "dry"; window.__lat.upSeen = false; });
  }
  await page.evaluate(() => { window.__lat.row = -1; });
  await mouse("mouseReleased", ...path(1));
  return pts;
}
/** One stroke along `path(u)` lasting `ms`, paced at RATE Hz. */
async function timedStroke(path, ms) {
  const n = Math.max(2, Math.round((ms / 1000) * RATE));
  const inflight = [];
  await mouse("mousePressed", ...path(0));
  const t0 = performance.now();
  for (let i = 1; i <= n; i++) {
    await until(t0 + (i * 1000) / RATE);
    const p = path(i / n);
    inflight.push(mouse("mouseMoved", p[0], p[1], 0.4 + 0.3 * Math.sin((i / n) * Math.PI)));
  }
  await Promise.all(inflight);
  await mouse("mouseReleased", ...path(1));
}

/** GPU time (ms) this browser's GPU process has used (Linux DRM fdinfo), or NaN. */
function chromeGpuMs() {
  const seen = new Map();
  try {
    for (const pid of readdirSync("/proc").filter((d) => /^\d+$/.test(d))) {
      let cmd = "";
      try { cmd = readFileSync(`/proc/${pid}/cmdline`, "utf8"); } catch { continue; }
      if (!cmd.includes("--type=gpu-process") || !cmd.includes("playwright_chromiumdev_profile")) continue;
      for (const fd of readdirSync(`/proc/${pid}/fdinfo`)) {
        let t = "";
        try { t = readFileSync(`/proc/${pid}/fdinfo/${fd}`, "utf8"); } catch { continue; }
        const id = t.match(/drm-client-id:\s*(\d+)/), ns = t.match(/drm-engine-gfx:\s*(\d+) ns/);
        if (id && ns) seen.set(id[1], Number(ns[1]) / 1e6);
      }
    }
  } catch { /* not Linux */ }
  return seen.size ? [...seen.values()].reduce((a, b) => a + b, 0) : NaN;
}

/** --writing, --4k: handwriting and long strokes; what each frame costs. */
async function run4k() {
  const S = Math.max(60, frame.height * 0.07);
  // Letters a line high, marching along lines; every sixth stroke a sweep
  // across the page (the same script as the board's own speed test).
  const letter = (cx, cy, k) => (u) => [cx + S * 0.5 * Math.sin(u * Math.PI * (2 + (k % 3))) + u * S * 0.6, cy + S * 0.5 * Math.cos(u * Math.PI * (1.5 + (k % 2)))];
  const sweep = (y) => (u) => [frame.x + frame.width * (0.08 + 0.84 * u), y + 40 * Math.sin(u * 9)];
  await browser.startTracing(page, { categories: ["viz", "cc", "gpu"] });
  await page.evaluate(() => { window.__lat.on = true; });
  const gpu0 = chromeGpuMs(), t0 = performance.now();
  let k = 0, col = 0, rowY = frame.y + S * 1.5;
  while (performance.now() < t0 + SECONDS * 1000) {
    if (k % 6 === 5) {
      await timedStroke(sweep(frame.y + frame.height * (0.15 + 0.7 * ((k * 0.37) % 1))), 900);
    } else {
      const cx = frame.x + frame.width * 0.08 + col * S * 1.3;
      await timedStroke(letter(cx, rowY, k), 320);
      col++;
      if (cx > frame.x + frame.width * 0.85) { col = 0; rowY += S * 1.8; if (rowY > frame.y + frame.height - S) rowY = frame.y + S * 1.5; }
    }
    k++;
    await sleep(90);
  }
  const wall = performance.now() - t0, gpuMs = chromeGpuMs() - gpu0;
  const trace = JSON.parse((await browser.stopTracing()).toString());
  const L = await page.evaluate(() => {
    const l = window.__lat;
    return { intervals: l.intervals, kinds: l.kinds, samples: l.samples, proc: l.proc, stats: window.rupokPerf?.stats?.() ?? null };
  });
  await browser.close();
  server.close();
  // How much of the screen the compositor redrew per frame (viz's damage).
  let frames = 0, area = 0, full = 0;
  const screenPx = env.screen[0] * env.screen[1];
  for (const e of trace.traceEvents ?? trace) {
    if (e.name !== "DirectRenderer::DrawFrame ProcessForOverlays") continue;
    const m = /(\d+)x(\d+)$/.exec(e.args?.root_damage_rect ?? "");
    if (!m || m[1] === "0") continue;
    frames++;
    area += m[1] * m[2];
    if (m[1] * m[2] > 0.8 * screenPx) full++;
  }
  const iv = stats(L.intervals);
  const writing = L.kinds[1] + L.kinds[2] + L.kinds[3];
  const r = {
    label: LABEL, mode: "4k", env: { ...env, seconds: SECONDS, strokes: k, rateHz: RATE },
    fps: { median: 1000 / iv.p50, p5: 1000 / iv.p95, min: 1000 / iv.max, mean: (1000 * L.intervals.length) / L.intervals.reduce((a, b) => a + b, 0) },
    frameMs: stats(L.samples.map((s) => s.frame)), processMs: stats(L.proc),
    tileFrames: L.kinds[2], fullFrames: L.kinds[1] + L.kinds[3],
    fullFramePct: writing ? (100 * (L.kinds[1] + L.kinds[3])) / writing : NaN,
    damagePct: frames ? (100 * area) / frames / screenPx : NaN, damageFullPct: frames ? (100 * full) / frames : NaN, compositorFrames: frames,
    chromeGpuMsPerS: (gpuMs / wall) * 1000,
    errors,
  };
  const f = (x, d = 1) => (Number.isFinite(x) ? x.toFixed(d) : "—");
  console.log(`\nRupok writing frames, ${BOARD.join("×")} board ${LABEL ? `— ${LABEL}` : ""}`);
  console.log(`GPU: ${env.gpu}\nBrowser: ${env.ua}${WAYLAND ? ` on ${WAYLAND}` : " (headless: frame rates are not a display's)"}, renderer ${env.backend || "?"}`);
  console.log(`Screen ${env.screen.join("×")} px, dpr ${env.dpr}; ink canvas ${env.canvas?.join("×")}${env.wetTile ? `, wet tile ${env.wetTile.join("×")}` : ", no wet tile"}`);
  console.log(`Input: pen ${RATE} Hz, ${k} strokes in ${SECONDS} s\n`);
  console.log(`frames while writing                       ${f(r.fps.median)} fps median, ${f(r.fps.p5)} p5, ${f(r.fps.min)} min, ${f(r.fps.mean)} mean  (n=${L.intervals.length})`);
  console.log(`event → GPU frame submitted                p50 ${f(r.frameMs.p50)}  p95 ${f(r.frameMs.p95)} ms`);
  console.log(`frames that drew the wet tile / the canvas ${r.tileFrames} / ${r.fullFrames}  (${f(r.fullFramePct)} % full-size)`);
  console.log(`screen redrawn by the compositor per frame ${f(r.damagePct)} %  (${f(r.damageFullPct)} % of ${frames} frames redrew all of it)`);
  console.log(`GPU time used by Chrome                    ${Number.isFinite(r.chromeGpuMsPerS) ? `${f(r.chromeGpuMsPerS, 0)} ms per second of writing` : "— (Linux DRM only)"}`);
  if (errors.length) console.log(`page errors:\n  ${errors.join("\n  ")}`);
  if (opt("json", null)) await writeFile(opt("json"), JSON.stringify(r, null, 2));
  let failed = errors.length ? 1 : 0;
  if (flag("check")) {
    console.log("\nbudgets");
    for (const [name, get, limit] of BUDGETS_4K) {
      const v = get(r);
      const ok = Number.isFinite(v) && v <= limit;
      if (!ok) failed++;
      console.log(`  ${ok ? "PASS" : "FAIL"} ${name.padEnd(46)} ${f(v, 2)} (limit ${limit})`);
    }
  }
  process.exit(failed ? 1 : 0);
}

const line = (x0, y, len) => (u) => [x0 + len * u, y];
/** Handwriting-like zig-zag ("wwww"): sharp corners every `step` px. */
const zigzag = (x0, y, len, step = 40, amp = 30) => (u) => {
  const x = len * u, k = Math.floor(x / step), f = x / step - k;
  return [x0 + x, y + (k % 2 ? amp * (1 - f) : amp * f) - amp / 2];
};

if (FOURK) await run4k();

await page.evaluate(() => { window.__lat.on = true; });
if (flag("profile")) {
  await cdp.send("Profiler.enable");
  await cdp.send("Profiler.setSamplingInterval", { interval: 200 });
  await cdp.send("Profiler.start");
}
let sent = 0;
const strokeLen = Math.min(900, frame.width * 0.7);
const x0 = frame.x + frame.width * 0.12;
for (let i = 0; i < STROKES; i++) {
  const y = frame.y + 60 + ((i * 37) % Math.max(60, frame.height - 120));
  sent += (await stroke(line(x0, y, strokeLen), strokeLen)).length;
  // Next stroke 120 ms after pen-up: pen-up work must not delay it.
  await sleep(120);
}
await sleep(200);
// Tip gap: strokes whose frames are read back (a GPU sync, so not timed).
for (let i = 0; i < Math.max(2, STROKES / 3); i++) {
  sent += (await stroke(line(x0, frame.y + 80 + i * 45, strokeLen), strokeLen, { pixels: true })).length;
  await sleep(150);
}
await sleep(200);
const longTasks = await page.evaluate(() => window.__lat.longTasks.slice());
// Pen-up jump and path fidelity: a zig-zag, captured wet and dry.
await page.evaluate(() => { window.__lat.noTime = true; });
const yCap = frame.y + frame.height - 70;
const zig = await stroke(zigzag(x0, yCap, strokeLen * 0.6), strokeLen * 0.6, { capture: true });
sent += zig.length;
await page.waitForFunction(() => window.__lat.captures.dry, null, { timeout: 5000 });
await sleep(300);

if (flag("profile")) {
  // Self time by function (and the JS caller chain of the hottest ones).
  const { profile } = await cdp.send("Profiler.stop");
  const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  const parent = new Map();
  for (const n of profile.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
  const self = new Map();
  const dts = profile.timeDeltas;
  profile.samples.forEach((id, i) => {
    const n = byId.get(id);
    let chain = [];
    for (let k = id; k !== undefined && chain.length < 6; k = parent.get(k)) {
      const cf = byId.get(k).callFrame;
      if (cf.functionName && !["(root)", "(program)", "(idle)"].includes(cf.functionName)) chain.push(cf.functionName);
    }
    const key = chain.slice(0, 4).join(" < ") || n.callFrame.functionName;
    self.set(key, (self.get(key) ?? 0) + (dts[i] ?? 0) / 1000);
  });
  const top = [...self.entries()].filter(([k]) => !/^\((idle|program|garbage)/.test(k)).sort((a, b) => b[1] - a[1]).slice(0, 25);
  console.log("profile (self ms, call chain):");
  for (const [k, v] of top) console.log(`  ${v.toFixed(0).padStart(6)}  ${k}`);
}
let present = null;
if (flag("present")) {
  // Event Timing: presentation time of a key press whose handler takes ~20 ms
  // (entries under 16 ms are not reported) minus the end of its processing.
  present = await page.evaluate(async () => {
    const out = [];
    new PerformanceObserver((l) => {
      for (const e of l.getEntries()) if (e.name === "keydown") out.push(e.startTime + e.duration - e.processingEnd);
    }).observe({ type: "event", durationThreshold: 16 });
    const div = document.body.appendChild(document.createElement("div"));
    div.style.cssText = "position:fixed;left:0;top:0;width:4px;height:4px;z-index:9999";
    addEventListener("keydown", (e) => {
      if (e.key !== "F13") return;
      const t = performance.now();
      while (performance.now() - t < 20) {}
      div.style.background = div.style.background === "red" ? "blue" : "red";
    });
    window.__presentOut = out;
    return true;
  });
  for (let i = 0; i < 30; i++) {
    await cdp.send("Input.dispatchKeyEvent", { type: "keyDown", key: "F13", code: "F13", windowsVirtualKeyCode: 124 });
    await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", key: "F13", code: "F13", windowsVirtualKeyCode: 124 });
    await sleep(70);
  }
  await sleep(300);
  present = stats(await page.evaluate(() => window.__presentOut));
}

const L = await page.evaluate((zig) => {
  const l = window.__lat;
  // Path fidelity: distance from each pen sample to the nearest dry ink pixel.
  const c = document.querySelector("canvas.ink");
  const fidelity = (() => {
    const a = l.captures.dry;
    if (!a || !c) return [];
    const r = c.getBoundingClientRect(), w = c.width, h = c.height, s = w / r.width;
    return zig.map(([x, y]) => {
      const cx = Math.round((x - r.left) * s), cy = Math.round((y - r.top) * s);
      let best = Infinity;
      for (let dy = -24; dy <= 24; dy++) for (let dx = -24; dx <= 24; dx++) {
        const X = cx + dx, Y = cy + dy;
        if (X < 0 || Y < 0 || X >= w || Y >= h || a[Y * w + X] <= 96) continue;
        best = Math.min(best, Math.hypot(dx, dy) / s);
      }
      return best;
    });
  })();
  const diff = (() => {
    const a = l.captures.wet, b = l.captures.dry;
    if (!a || !b) return null;
    let changed = 0, inked = 0;
    for (let i = 0; i < a.length; i++) {
      if (a[i] > 96 || b[i] > 96) inked++;
      if (Math.abs(a[i] - b[i]) > 96) changed++;
    }
    return { changed, inked };
  })();
  // Wet (red) over dry (blue), overlap black, cropped to the ink.
  const shot = (() => {
    const a = l.captures.wet, b = l.captures.dry;
    if (!a || !b || !c) return null;
    const w = c.width, h = c.height;
    let x0 = w, y0 = h, x1 = 0, y1 = 0;
    for (let i = 0; i < a.length; i++) if (a[i] > 96 || b[i] > 96) {
      const x = i % w, y = (i / w) | 0;
      x0 = Math.min(x0, x); y0 = Math.min(y0, y); x1 = Math.max(x1, x); y1 = Math.max(y1, y);
    }
    if (x1 < x0) return null;
    x0 = Math.max(0, x0 - 12); y0 = Math.max(0, y0 - 12); x1 = Math.min(w - 1, x1 + 12); y1 = Math.min(h - 1, y1 + 12);
    const k = document.createElement("canvas");
    const S = 3;
    k.width = (x1 - x0 + 1) * S; k.height = (y1 - y0 + 1) * S;
    const g = k.getContext("2d");
    g.fillStyle = "#fff"; g.fillRect(0, 0, k.width, k.height);
    for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) {
      const i = y * w + x, A = a[i] > 96, B = b[i] > 96;
      if (!A && !B) continue;
      g.fillStyle = A && B ? "#222" : A ? "#e5484d" : "#3e63dd";
      g.fillRect((x - x0) * S, (y - y0) * S, S, S);
    }
    g.fillStyle = "#12a594";
    for (const [px, py] of zig) {
      const r = c.getBoundingClientRect(), s = w / r.width;
      g.fillRect(((px - r.left) * s - x0) * S, ((py - r.top) * s - y0) * S, 1, 1);
    }
    return k.toDataURL("image/png");
  })();
  return {
    shot,
    samples: l.samples, frames: l.frames, gaps: l.gaps, proc: l.proc, received: l.received, longTasks: l.longTasks,
    firstFrameAfterDown: l.firstFrameAfterDown, diff, loops: l.loops, fidelity, fullIntervals: l.fullIntervals,
  };
}, zig);
await browser.close();
server.close();

const px2ms = (px) => (px / SPEED) * 1000;
const tipS = stats(L.gaps.map((f) => f.gapSample));
const tipR = stats(L.gaps.map((f) => f.gapRaw));
const frameDur = stats(L.frames.map((f) => f.dur));
const ffd = L.firstFrameAfterDown;
// The first frame after pen-down, for strokes that start 120 ms after another
// stroke's pen-up, compared with a typical frame (the median of all frames).
const firstFrame = stats(ffd);
const result = {
  label: LABEL,
  env: { ...env, rateHz: RATE, speedPxPerS: SPEED, strokes: STROKES, smoothing: SMOOTHING ?? "default", brush: BRUSH ?? "pen", smart: flag("smart"), renderer: flag("swiftshader") ? "SwiftShader" : "GPU" },
  handlerMs: stats(L.samples.map((s) => s.handler)),
  processMs: stats(L.proc),
  frameMs: stats(L.samples.map((s) => s.frame)),
  tipGapSamplePx: tipS,
  tipGapSampleMs: { p50: px2ms(tipS.p50), p95: px2ms(tipS.p95) },
  tipGapReceivedPx: tipR,
  tipGapReceivedMs: { p50: px2ms(tipR.p50), p95: px2ms(tipR.p95) },
  loopMs: frameDur,
  firstFrameAfterDownMs: firstFrame,
  firstFrameList: ffd.map((x) => Math.round(x)),
  // Against a typical frame, plus the one frame a pen-up may cost: after a
  // long stroke the whole ink canvas is drawn once (every frame was that
  // before the wet tile), and a stroke that starts during it waits for it.
  // Anything beyond that is pen-up work in the next stroke's way.
  nextStrokeExtraMs: Math.max(0, firstFrame.p95 - Math.max(stats(L.samples.map((s) => s.frame)).p95, stats(L.samples.filter((s) => s.full).map((s) => s.frame)).p95 || 0) - (stats(L.fullIntervals).p95 || 0)),
  penUpChangedPx: L.diff?.changed ?? NaN,
  penUpChangedPct: L.diff ? (100 * L.diff.changed) / Math.max(1, L.diff.inked) : NaN,
  sent,
  received: L.received,
  droppedPct: Math.max(0, (100 * (sent - L.received)) / sent),
  longTasks: { count: longTasks.length, maxMs: Math.max(0, ...longTasks.map((t) => t[1])), list: longTasks },
  pathErrorPx: stats(L.fidelity),
  penUpInkedPx: L.diff?.inked ?? 0,
  presentMs: present,
  errors,
};

const f = (x, d = 1) => (Number.isFinite(x) ? x.toFixed(d) : "—");
const row = (name, s, unit = "ms") => console.log(`${name.padEnd(42)} p50 ${f(s.p50).padStart(6)}  p95 ${f(s.p95).padStart(6)}  max ${f(s.max).padStart(6)} ${unit}  (n=${s.n})`);
console.log(`\nRupok pen latency ${LABEL ? `— ${LABEL}` : ""}`);
console.log(`GPU: ${env.gpu}\nBrowser: ${env.ua}, dpr ${env.dpr}, navigator.ink ${env.inkApi ? "yes" : "no"}`);
console.log(`Input: pen ${RATE} Hz, ${SPEED} px/s, ${STROKES} strokes, smoothing ${SMOOTHING ?? "default"}, brush ${BRUSH ?? "pen"}, smart shapes ${flag("smart") ? "auto" : "off"}\n`);
row("event → board handler done", result.handlerMs);
row("board pointermove handler (processing)", result.processMs);
row("event → GPU frame submitted", result.frameMs);
row("board frame loop (rAF callback)", result.loopMs);
row("ink tip behind newest sample in frame", tipS, "px");
console.log(`${"".padEnd(42)} = p50 ${f(result.tipGapSampleMs.p50)} ms, p95 ${f(result.tipGapSampleMs.p95)} ms of pen travel`);
row("ink tip behind newest received sample", tipR, "px");
console.log(`${"".padEnd(42)} = p50 ${f(result.tipGapReceivedMs.p50)} ms, p95 ${f(result.tipGapReceivedMs.p95)} ms of pen travel`);
row("pen-down → first frame", firstFrame);
console.log(`pen-up: ${result.penUpChangedPx} of ${result.penUpInkedPx} ink px changed between the last wet frame and the dry stroke (${f(result.penUpChangedPct, 2)}%)`);
row("zig-zag: pen sample → nearest dry ink", result.pathErrorPx, "px");
console.log(`samples: ${sent} sent, ${L.received} received (${f(result.droppedPct, 2)}% dropped)`);
console.log(`long tasks while writing: ${result.longTasks.count} (max ${f(result.longTasks.maxMs)} ms)${flag("verbose") ? " " + JSON.stringify(longTasks) : ""}`);
if (present) row("frame → presented (Event Timing, ±8 ms)", present);
if (errors.length) console.log(`page errors:\n  ${errors.join("\n  ")}`);

if (opt("shot", null) && L.shot) await writeFile(opt("shot"), Buffer.from(L.shot.split(",")[1], "base64"));
if (opt("json", null)) await writeFile(opt("json"), JSON.stringify(result, null, 2));
if (flag("check")) {
  let failed = 0;
  console.log("\nbudgets");
  for (const [name, get, limit] of BUDGETS) {
    const v = get(result);
    const ok = Number.isFinite(v) && v <= limit;
    if (!ok) failed++;
    console.log(`  ${ok ? "PASS" : "FAIL"} ${name.padEnd(42)} ${f(v, 2)} (limit ${limit})`);
  }
  if (errors.length) failed++;
  process.exit(failed ? 1 : 0);
}
