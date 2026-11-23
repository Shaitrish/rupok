#!/usr/bin/env node
// Records the landing page's hero demo: a teacher taking a class on the real
// board, as a silent looping video (MP4/H.264 + WebM/VP9) plus a WebP poster,
// in a light version (white page) and a dark one (dark theme, blackboard page).
//
//   node scripts/record-demo.mjs                  # both themes -> apps/board/public/demo/
//   node scripts/record-demo.mjs --theme dark     # one theme
//   node scripts/record-demo.mjs --preview        # 30 fps draft + stills, no final encode
//
// See scripts/README-demo.md for the storyboard, options and requirements.
//
// How it captures: the board runs in headless Chrome on virtual time. An init
// script in every frame (the board and each Smart Object iframe) replaces
// requestAnimationFrame, timers, performance.now, Date, Event.timeStamp,
// Math.random and the Web Animations clock, so the page only moves when the
// recorder steps it by exactly 1/60 s. Each step is screenshotted and piped
// to ffmpeg. The result is frame-perfect 60 fps however slow or busy the
// machine is (real-time screen capture on a loaded machine drops frames).
// Pen input is real CDP pen events with pressure, at 240 Hz.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";
import { hershey } from "./demo-hershey.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

// ------------------------------------------------------------------ options

const argv = process.argv.slice(2);
const opt = (name, def) => {
  const i = argv.findIndex((a) => a === `--${name}` || a.startsWith(`--${name}=`));
  if (i < 0) return def;
  const a = argv[i];
  if (a.includes("=")) return a.slice(a.indexOf("=") + 1);
  const next = argv[i + 1];
  return next && !next.startsWith("--") ? next : true;
};
if (opt("help", false)) {
  console.log(fs.readFileSync(fileURLToPath(import.meta.url), "utf8").split("\n").slice(1, 11).join("\n").replace(/^\/\/ ?/gm, ""));
  process.exit(0);
}
const PREVIEW = !!opt("preview", false);
const THEMES = String(opt("theme", "both")) === "both" ? ["light", "dark"] : String(opt("theme")).split(",");
if (THEMES.some((t) => t !== "light" && t !== "dark")) throw new Error(`--theme must be light, dark or both`);
const URL_ = String(opt("url", process.env.RUPOK_URL || "http://127.0.0.1:3737/board/"));
const FPS = Number(opt("fps", PREVIEW ? 30 : 60));
const [VW, VH] = String(opt("size", "1920x1080")).split("x").map(Number);
const OUT = path.resolve(String(opt("out", path.join(ROOT, "apps/board/public/demo"))));
const WORK = path.resolve(String(opt("work", process.env.RUPOK_DEMO_WORK || path.join(fs.existsSync("/dev/shm") ? "/dev/shm" : os.tmpdir(), "rupok-demo"))));
const STILLS = path.join(WORK, "stills");
/** Target size of each delivered video file. */
const MAX_BYTES = Number(opt("max-mb", 2.9)) * 1e6;
const CHROME = process.env.CHROME || "/usr/bin/google-chrome";
const GPU = String(opt("gpu", "swiftshader")); // swiftshader (default, works anywhere) | vulkan | default
const NAME = "rupok-demo";
/** Pen samples per video frame: 240 Hz pen input at 60 fps, like a real
 * stylus (the board draws straight segments between samples). */
const SUB = 4;
const HZ = FPS * SUB;
const STILLS_ON = PREVIEW || !!opt("stills", false);
/** Stop after this scene (1–6): quick checks of the first scenes. */
const UNTIL = Number(opt("until", 6));

/** Teacher's name on the cursor. */
const TEACHER = "Ms. Rahman";
/** Scene captions (burned in). */
const CAPTIONS = [
  "Write on any slide",
  "Drop in a live lesson from your AI",
  "Step through with your clicker",
  "Annotate on top",
  "Present full screen",
  "Replay every class",
];
/** Where to annotate the projectile preset at step 2, as fractions of the
 * slide frame (0..1). Re-measure these if examples/objects/physics-projectile.html
 * changes its layout: run with --preview and check stills/light-annotated.png. */
const SPOTS = {
  peak: { u: 0.4429, v: 0.468, ru: 0.064, rv: 0.082 }, // ellipse round the apex and its "v_y = 0" tag
  arrow: [[0.4975, 0.4197], [0.7295, 0.3394], [0.8424, 0.7318]], // a high arc from the ellipse down onto the "R = 40.8 m" tag
  highlight: [[0.2529, 0.1975], [0.4685, 0.1975]], // "equal steps sideways" in the caption
  launch: [0.094, 0.815], // the cannon, where the laser starts
  land: [0.795, 0.815],
};

// --------------------------------------------------------------- playwright

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
      return mod.chromium ? mod : mod.default;
    } catch { /* next */ }
  }
  console.error("playwright-core not found. Install it somewhere (npm i playwright-core) and run this from there, or set PLAYWRIGHT_CORE.");
  process.exit(1);
}

// ------------------------------------------------------------- virtual time
// Injected into every frame before any page script. Real time flows until the
// recorder's first step(); from then on the frame's clock only moves in step().

function virtualTime(seed) {
  if (window.__vt) return;
  const W = window;
  const perf = W.performance;
  const rNow = perf.now.bind(perf);
  const RDate = W.Date;
  const rRAF = W.requestAnimationFrame.bind(W);
  const rSetInterval = W.setInterval.bind(W);
  const report = (e) => (W.reportError ? W.reportError(e) : console.error(e));
  const vt = { manual: false, now: rNow(), seq: 1, timers: new Map(), rafs: new Map(), ts: new WeakMap(), anims: new WeakMap() };
  Object.defineProperty(W, "__vt", { value: vt });
  const now = () => (vt.manual ? vt.now : rNow());
  perf.now = now;
  const dateBase = RDate.now() - rNow();
  function VDate(...a) {
    if (!new.target) return new RDate(dateBase + now()).toString();
    return a.length ? new RDate(...a) : new RDate(dateBase + now());
  }
  VDate.prototype = RDate.prototype;
  VDate.now = () => Math.floor(dateBase + now());
  VDate.parse = RDate.parse;
  VDate.UTC = RDate.UTC;
  W.Date = VDate;
  const add = (fn, ms, args, repeat) => {
    const id = vt.seq++;
    const d = Math.max(0, Number(ms) || 0);
    vt.timers.set(id, { fn: typeof fn === "function" ? fn : () => (0, eval)(String(fn)), at: now() + d, ms: d, args, repeat });
    return id;
  };
  W.setTimeout = (fn, ms, ...args) => add(fn, ms, args, false);
  W.setInterval = (fn, ms, ...args) => add(fn, ms, args, true);
  W.clearTimeout = W.clearInterval = (id) => void vt.timers.delete(id);
  W.requestAnimationFrame = (cb) => {
    const id = vt.seq++;
    vt.rafs.set(id, cb);
    return id;
  };
  W.cancelAnimationFrame = (id) => void vt.rafs.delete(id);
  vt.runTimers = (upto) => {
    for (let guard = 0; guard < 20000; guard++) {
      let best = null, bestId = 0;
      for (const [id, t] of vt.timers) if (t.at <= upto && (!best || t.at < best.at)) { best = t; bestId = id; }
      if (!best) return;
      if (vt.manual) vt.now = Math.max(vt.now, best.at);
      if (best.repeat) best.at += Math.max(best.ms, 1);
      else vt.timers.delete(bestId);
      try { best.fn(...best.args); } catch (e) { report(e); }
    }
  };
  vt.runRaf = (t) => {
    const list = [...vt.rafs.values()];
    vt.rafs.clear();
    for (const cb of list) try { cb(t); } catch (e) { report(e); }
  };
  // Free-running (real time) until the recorder takes over.
  const pump = () => {
    if (!vt.manual) { vt.runTimers(rNow()); vt.runRaf(rNow()); }
    rRAF(pump);
  };
  rRAF(pump);
  rSetInterval(() => { if (!vt.manual) vt.runTimers(rNow()); }, 4);
  const tsGet = Object.getOwnPropertyDescriptor(Event.prototype, "timeStamp").get;
  Object.defineProperty(Event.prototype, "timeStamp", {
    configurable: true,
    enumerable: true,
    get() {
      if (!vt.manual) return tsGet.call(this);
      let v = vt.ts.get(this);
      if (v === undefined) { v = vt.now; vt.ts.set(this, v); }
      return v;
    },
  });
  let s = seed >>> 0;
  Math.random = () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
  // CSS animations and transitions: paused, and moved by hand in virtual time.
  vt.stepAnims = (dt) => {
    if (!document.getAnimations) return;
    for (const a of document.getAnimations()) {
      let st = vt.anims.get(a);
      if (!st) {
        const ct = Number(a.currentTime) || 0;
        st = { t: ct < 300 ? 0 : ct, ours: false };
        vt.anims.set(a, st);
      }
      if (a.playState === "running") { a.pause(); st.ours = true; }
      if (!st.ours || a.playState === "finished") continue;
      st.t += dt * (a.playbackRate || 1);
      const end = a.effect && a.effect.getComputedTiming ? a.effect.getComputedTiming().endTime : Infinity;
      if (Number.isFinite(end) && st.t >= end) a.finish();
      else a.currentTime = st.t;
    }
  };
  vt.step = (dt, raf) => {
    if (!vt.manual) { vt.manual = true; vt.now = rNow(); }
    const target = vt.now + dt;
    vt.runTimers(target);
    vt.now = target;
    vt.stepAnims(dt);
    if (raf) vt.runRaf(vt.now);
    return true;
  };
}

// ---------------------------------------------------------------- overlay
// Caption chip, keycap and cursor, drawn into the board page (burned in).

function installOverlay({ dark, teacher }) {
  const css = `
  #rk-demo { position: fixed; inset: 0; pointer-events: none; z-index: 2147483647; font-family: "Inter Variable", Inter, system-ui, sans-serif; }
  #rk-demo .cap { position: absolute; left: 50%; top: 96px; display: flex; align-items: center; gap: 14px; height: 62px; padding: 0 28px 0 12px;
    border-radius: 999px; white-space: nowrap; font: 700 28px/1 "Inter Variable", Inter, sans-serif; letter-spacing: -0.015em;
    background: ${dark ? "#ffffff" : "#111418"}; color: ${dark ? "#111418" : "#ffffff"};
    box-shadow: 0 0 0 2px ${dark ? "rgba(17,20,24,0.9)" : "rgba(255,255,255,0.9)"};
    transform: translate(-50%, 0); transition: opacity 240ms ease, transform 360ms cubic-bezier(.2,.8,.2,1); }
  #rk-demo .cap.pre { opacity: 0; transform: translate(-50%, 16px) scale(0.96); }
  #rk-demo .cap.out { opacity: 0; transform: translate(-50%, -12px) scale(0.98); }
  #rk-demo .n { width: 40px; height: 40px; border-radius: 999px; display: grid; place-items: center; font-size: 21px; font-weight: 800;
    background: ${dark ? "#20c997" : "#3ee6b0"}; color: #04241b; }
  #rk-demo .key { margin-left: 6px; height: 40px; min-width: 44px; padding: 0 12px; border-radius: 11px; display: grid; place-items: center;
    font: 700 19px/1 "Inter Variable", Inter, sans-serif; letter-spacing: 0;
    background: ${dark ? "#f1f3f5" : "#2b2f36"}; color: ${dark ? "#111418" : "#ffffff"};
    box-shadow: inset 0 -3px 0 ${dark ? "rgba(17,20,24,0.18)" : "rgba(0,0,0,0.45)"}, 0 0 0 1.5px ${dark ? "rgba(17,20,24,0.16)" : "rgba(255,255,255,0.2)"};
    transition: transform 80ms ease, background-color 80ms ease, color 80ms ease; }
  #rk-demo .key.down { transform: translateY(2px) scale(0.92); background: ${dark ? "#20c997" : "#3ee6b0"}; color: #04241b; }
  #rk-demo .cur { position: absolute; left: 0; top: 0; display: flex; align-items: flex-start; gap: 2px; will-change: transform; transition: opacity 160ms ease; }
  #rk-demo .cur.hide { opacity: 0; }
  #rk-demo .cur svg { width: 30px; height: 30px; display: block; }
  #rk-demo .cur path { fill: ${dark ? "#ffffff" : "#111418"}; stroke: ${dark ? "#111418" : "#ffffff"}; stroke-width: 1.6; stroke-linejoin: round; }
  /* The board's own toasts would sit under the caption chip. */
  .toast { display: none !important; }
  #rk-demo .cur span { margin-top: 22px; padding: 7px 12px; border-radius: 999px 999px 999px 5px; font: 700 18px/1 "Inter Variable", Inter, sans-serif;
    background: ${dark ? "#20c997" : "#3ee6b0"}; color: #04241b; }
  `;
  const root = document.createElement("div");
  root.id = "rk-demo";
  root.innerHTML = `<style>${css}</style><div class="cur hide"><svg viewBox="0 0 24 24"><path d="M4 2l16 7-7 2.6L10 20z"/></svg><span></span></div>`;
  root.querySelector(".cur span").textContent = teacher;
  document.body.appendChild(root);
  const cur = root.querySelector(".cur");
  let chip = null;
  window.__demo = {
    caption(n, text, key) {
      if (chip) {
        const old = chip;
        old.classList.add("out");
        setTimeout(() => old.remove(), 400);
      }
      chip = document.createElement("div");
      chip.className = "cap pre";
      chip.innerHTML = `<span class="n"></span><span class="t"></span>${key ? '<span class="key"></span>' : ""}`;
      chip.querySelector(".n").textContent = String(n);
      chip.querySelector(".t").textContent = text;
      if (key) chip.querySelector(".key").textContent = key;
      root.appendChild(chip);
      void chip.offsetWidth;
      chip.classList.remove("pre");
    },
    key(down) {
      chip?.querySelector(".key")?.classList.toggle("down", down);
    },
    cursor(x, y, show) {
      cur.style.transform = `translate(${x - 5}px, ${y - 2.5}px)`;
      cur.classList.toggle("hide", !show);
    },
  };
}

// --------------------------------------------------------------- ink paths

function rng(seed) {
  let s = seed >>> 0;
  return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296);
}

/** Corner-cutting (Chaikin), keeping the end points. */
function chaikin(pts, iterations = 1) {
  let p = pts;
  for (let k = 0; k < iterations && p.length > 2; k++) {
    const q = [p[0]];
    for (let i = 0; i < p.length - 1; i++) {
      const [a, b] = [p[i], p[i + 1]];
      q.push([a[0] * 0.75 + b[0] * 0.25, a[1] * 0.75 + b[1] * 0.25], [a[0] * 0.25 + b[0] * 0.75, a[1] * 0.25 + b[1] * 0.75]);
    }
    q.push(p[p.length - 1]);
    p = q;
  }
  return p;
}

/** Centripetal Catmull-Rom through `pts`, sampled about every `step` px. */
function spline(pts, step = 1.2) {
  if (pts.length < 2) return pts.slice();
  const out = [];
  const P = [pts[0], ...pts, pts[pts.length - 1]];
  const d = (a, b) => Math.max(1e-4, Math.hypot(b[0] - a[0], b[1] - a[1]) ** 0.5);
  for (let i = 1; i < P.length - 2; i++) {
    const [p0, p1, p2, p3] = [P[i - 1], P[i], P[i + 1], P[i + 2]];
    const t0 = 0, t1 = t0 + d(p0, p1), t2 = t1 + d(p1, p2), t3 = t2 + d(p2, p3);
    const n = Math.max(1, Math.ceil(Math.hypot(p2[0] - p1[0], p2[1] - p1[1]) / step));
    for (let k = 0; k < n; k++) {
      const t = t1 + ((t2 - t1) * k) / n;
      const L = (a, b, ta, tb) => [((tb - t) * a[0] + (t - ta) * b[0]) / (tb - ta), ((tb - t) * a[1] + (t - ta) * b[1]) / (tb - ta)];
      const A1 = L(p0, p1, t0, t1), A2 = L(p1, p2, t1, t2), A3 = L(p2, p3, t2, t3);
      out.push(L(L(A1, A2, t0, t2), L(A2, A3, t1, t3), t1, t2));
    }
  }
  out.push(pts[pts.length - 1]);
  return out;
}

/** Pen samples ([x, y, pressure] at `hz`) along a dense path: faster on
 * straights, slower in tight curves (the 2/3 power law), eased at both ends. */
function timeStroke(pts, { speed = 1300, hz = HZ, pressure = 0.55 } = {}) {
  const seg = [];
  let total = 0;
  for (let i = 1; i < pts.length; i++) {
    const l = Math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]);
    seg.push(l);
    total += l;
  }
  const curv = pts.map((p, i) => {
    if (i === 0 || i === pts.length - 1) return 0;
    const a = pts[Math.max(0, i - 3)], b = pts[Math.min(pts.length - 1, i + 3)];
    const v1 = [p[0] - a[0], p[1] - a[1]], v2 = [b[0] - p[0], b[1] - p[1]];
    const ang = Math.abs(Math.atan2(v1[0] * v2[1] - v1[1] * v2[0], v1[0] * v2[0] + v1[1] * v2[1]));
    return ang / (Math.hypot(...v1) + Math.hypot(...v2) + 1e-3);
  });
  const dt = 1 / hz;
  const out = [[pts[0][0], pts[0][1], pressure * 0.45]];
  let i = 0, along = 0, acc = 0, t = 0;
  while (i < seg.length) {
    const done = acc + along;
    let v = speed * Math.min(1, Math.max(0.3, Math.pow(0.02 / Math.max(curv[i], 1e-4), 1 / 3)));
    v *= Math.min(1, done / 28 + 0.3, (total - done) / 34 + 0.35);
    let move = v * dt;
    while (move > 0 && i < seg.length) {
      const left = seg[i] - along;
      if (move < left) { along += move; move = 0; } else { move -= left; acc += seg[i]; along = 0; i++; }
    }
    t += dt;
    const p = i < seg.length
      ? [pts[i][0] + ((pts[i + 1][0] - pts[i][0]) * along) / (seg[i] || 1), pts[i][1] + ((pts[i + 1][1] - pts[i][1]) * along) / (seg[i] || 1)]
      : pts[pts.length - 1];
    const pr = Math.min(0.85, pressure * (0.8 + 0.45 * (1 - v / speed))) * Math.min(1, t / 0.04 + 0.4);
    out.push([p[0], p[1], pr]);
  }
  const tail = Math.min(5, out.length - 1);
  for (let k = 1; k <= tail; k++) out[out.length - k][2] *= 0.55 + (0.45 * (k - 1)) / tail;
  return out;
}

/** Handwriting for `text`, centred on (cx, cy) with cap height `height`:
 * cursive strokes joined, i-dots and t-bars added after each word. */
function handwriting(text, { cx, cy, height, seed = 7, speed = 1750 }) {
  const rand = rng(seed);
  const s = height / 21;
  const GAP = 9;
  const words = [];
  let wx = 0;
  for (const word of text.split(" ")) {
    const g = hershey(word);
    words.push(g.strokes.map((st) => st.map(([x, y]) => [x + wx, y])));
    wx += g.width + GAP;
  }
  const width = wx - GAP;
  const len = (st) => st.reduce((a, p, i) => (i ? a + Math.hypot(p[0] - st[i - 1][0], p[1] - st[i - 1][1]) : 0), 0);
  const order = [];
  for (const strokes of words) {
    const joined = [];
    for (const st of strokes) {
      const last = joined[joined.length - 1];
      const e = last?.[last.length - 1];
      if (e && Math.hypot(e[0] - st[0][0], e[1] - st[0][1]) <= 1.01) last.push(...st.slice(1));
      else joined.push(st.slice());
    }
    const longest = Math.max(...joined.map(len));
    order.push(...joined.filter((st) => len(st) >= longest * 0.22), ...joined.filter((st) => len(st) < longest * 0.22));
  }
  return order.map((st) => {
    const dy = (rand() - 0.5) * 0.6, k = 1 + (rand() - 0.5) * 0.03;
    const pts = st.map(([x, y]) => [cx + (x - width / 2) * s * k, cy + (y - 1.5 + dy) * s * k]);
    return timeStroke(spline(chaikin(pts, 1), 1.2), { speed });
  });
}

const ellipsePath = (cx, cy, rx, ry, from = -2.2, turn = 2 * Math.PI + 0.35) => {
  const n = 64;
  return Array.from({ length: n + 1 }, (_, i) => {
    const a = from + (turn * i) / n;
    const wob = 1 + 0.025 * Math.sin(a * 2.3);
    return [cx + Math.cos(a) * rx * wob, cy + Math.sin(a) * ry * wob];
  });
};
const quad = (a, c, b, n = 40) =>
  Array.from({ length: n + 1 }, (_, i) => {
    const t = i / n;
    return [(1 - t) ** 2 * a[0] + 2 * (1 - t) * t * c[0] + t * t * b[0], (1 - t) ** 2 * a[1] + 2 * (1 - t) * t * c[1] + t * t * b[1]];
  });
const ease = (t) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);

// ------------------------------------------------------------------ recorder

async function record(chromium, theme) {
  const dark = theme === "dark";
  const tag = `[${theme}]`;
  const args = ["--hide-scrollbars", "--force-color-profile=srgb", "--force-device-scale-factor=1", "--disable-renderer-backgrounding", "--disable-background-timer-throttling", "--ignore-gpu-blocklist"];
  if (GPU === "vulkan") args.push("--use-angle=vulkan", "--enable-features=Vulkan,DefaultANGLEVulkan,VulkanFromANGLE");
  else if (GPU !== "default") args.push("--use-angle=swiftshader");
  const browser = await chromium.launch({ executablePath: CHROME, headless: true, args });
  const ctx = await browser.newContext({ viewport: { width: VW, height: VH }, deviceScaleFactor: 1, colorScheme: dark ? "dark" : "light", reducedMotion: "no-preference" });
  const page = await ctx.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.addInitScript(virtualTime, 20251123);
  await page.addInitScript((p) => {
    if (window !== window.top) return;
    // No Vite HMR: an edit elsewhere in the tree must not reload the page mid-take.
    window.WebSocket = class extends EventTarget { constructor() { super(); this.readyState = 0; } send() {} close() {} };
    try { localStorage.setItem("rupok.prefs", JSON.stringify(p)); } catch {}
  }, { theme, smartShapes: "hold", scratchOut: false, showStats: false });
  await page.goto(URL_, { waitUntil: "networkidle" });
  // The Smart Dock (bottom) is the sign the board is ready.
  await page.waitForSelector(".dock", { timeout: 60000 });
  await page.evaluate(() => document.fonts.ready);

  // ---- set the stage (not recorded): two blank pages, page 1 current.
  const setup = await page.evaluate(async ({ dark }) => {
    // The board's own modules, exactly as the app loaded them (with Vite's ?t= stamps).
    const mod = (name) => performance.getEntriesByType("resource").map((e) => e.name).find((n) => new RegExp(`/src/${name}\\.ts(\\?|$)`).test(n));
    const { controller } = await import(mod("controller"));
    window.__ctl = controller;
    window.__st = await import(mod("state"));
    const b = controller.board;
    b.set_title("Class 9 · Physics");
    b.add_page("plain");
    if (dark) for (const i of [1, 0]) { b.goto_page(i); controller.setBoardColor(0x17191cff); }
    b.goto_page(0);
    controller.updateInsets(true);
    return { smart: typeof b.set_smart_ink === "function" };
  }, { dark });
  // Tool, size and colour as the dock would set them (keyboard tool switches
  // leave the ink alone).
  const ink = (tool, width, color = null) =>
    page.evaluate(([tool, width, color]) => {
      const c = window.__ctl, st = window.__st, hl = tool === "highlighter";
      c.setTool(tool);
      if (color !== null) (hl ? st.setHlColor : st.setColor)(color);
      st.setSizes({ ...st.sizes(), [hl ? "highlighter" : "pen"]: width });
      if (!hl) st.setWidth(width);
      c.board.set_color(hl ? st.hlColor() : st.color());
      c.board.set_width(width);
    }, [tool, width, color]);
  const PEN_W = 8, HL_W = 34, RED = 0xe5484dff;
  await ink("highlighter", HL_W);
  await ink("pen", PEN_W);
  await page.mouse.move(VW * 0.62, VH * 0.7);
  await page.evaluate(installOverlay, { dark, teacher: TEACHER });
  await page.waitForTimeout(600);

  const cdp = await ctx.newCDPSession(page);
  const frameRect = () =>
    page.evaluate(() => {
      const c = window.__ctl, [cx, cy, z] = c.board.camera(), r = c.root.getBoundingClientRect();
      const f = JSON.parse(c.board.pages_json())[c.board.page_index()].frame ?? { w: 1920, h: 1080 };
      return { x: r.left - cx * z, y: r.top - cy * z, w: f.w * z, h: f.h * z };
    });
  const at = (F, u, v) => [F.x + F.w * u, F.y + F.h * v];

  // ---- ffmpeg: PNG frames in, near-lossless master out.
  fs.mkdirSync(WORK, { recursive: true });
  const master = path.join(WORK, `${NAME}-${theme}-master.mkv`);
  const ff = spawn("ffmpeg", ["-y", "-hide_banner", "-loglevel", "error", "-f", "image2pipe", "-framerate", String(FPS), "-c:v", "png", "-i", "-",
    "-c:v", "libx264", "-preset", "veryfast", "-crf", PREVIEW ? "16" : "4", "-pix_fmt", "yuv444p", master], { stdio: ["pipe", "inherit", "inherit"] });
  const ffDone = new Promise((r) => ff.on("exit", r));

  // ---- the frame clock
  const subDt = 1000 / FPS / SUB;
  let frame = 0;
  const cursor = { x: VW * 0.62, y: VH * 0.7, show: false };
  const marks = {};
  const stepAll = async (dt, raf) => {
    const main = page.mainFrame();
    await Promise.all(
      page.frames().map((f) =>
        f.evaluate(([dt, raf, c, isMain]) => {
          if (!window.__vt) return;
          if (isMain) window.__demo?.cursor(c.x, c.y, c.show);
          window.__vt.step(dt, raf);
        }, [dt, raf, cursor, f === main]).catch(() => {}),
      ),
    );
  };
  /** One video frame; `inputs` are up to SUB functions, one per sub-step. */
  const tick = async (inputs = []) => {
    if (!inputs.length) await stepAll(subDt * SUB, true);
    else
      for (let k = 0; k < SUB; k++) {
        if (inputs[k]) await inputs[k]();
        await stepAll(subDt, k === SUB - 1);
      }
    const { data } = await cdp.send("Page.captureScreenshot", { format: "png", optimizeForSpeed: true });
    const buf = Buffer.from(data, "base64");
    if (!ff.stdin.write(buf)) await new Promise((r) => ff.stdin.once("drain", r));
    frame++;
    if (frame % FPS === 0) process.stdout.write(`\r${tag} ${(frame / FPS).toFixed(0)} s recorded`);
  };
  const hold = async (sec) => { for (let n = Math.round(sec * FPS); n > 0; n--) await tick(); };
  const still = async (name) => {
    marks[name] = frame / FPS;
    if (!STILLS_ON) return;
    fs.mkdirSync(STILLS, { recursive: true });
    await page.screenshot({ path: path.join(STILLS, `${theme}-${name}.png`) });
  };
  const waitFor = async (fn, arg, maxSec = 8) => {
    for (let n = 0; n < maxSec * FPS; n++) {
      if (await page.evaluate(fn, arg).catch(() => false)) return true;
      await tick();
    }
    console.warn(`\n${tag} waitFor timed out: ${fn.toString().slice(0, 80)}`);
    return false;
  };

  // input
  const mouseEv = (type, x, y, extra = {}) => cdp.send("Input.dispatchMouseEvent", { type, x, y, pointerType: "mouse", ...extra });
  const penEv = (type, x, y, p) =>
    cdp.send("Input.dispatchMouseEvent", {
      type, x, y, pointerType: "pen", force: p,
      button: type === "mouseMoved" && p === 0 ? "none" : "left",
      buttons: type === "mouseReleased" || p === 0 ? 0 : 1,
      clickCount: type === "mouseMoved" ? 0 : 1,
    });
  /** Glide the pointer (mouse, or a hovering pen) to (x, y). */
  const glide = async (x, y, sec = 0.45, kind = "mouse") => {
    const [x0, y0] = [cursor.x, cursor.y];
    const n = Math.max(1, Math.round(sec * FPS));
    // a gentle arc, like a hand moving
    const bend = Math.min(60, Math.hypot(x - x0, y - y0) * 0.08);
    const nx = -(y - y0), ny = x - x0, nl = Math.hypot(nx, ny) || 1;
    cursor.show = true;
    for (let i = 1; i <= n; i++) {
      const t = ease(i / n), arc = Math.sin(Math.PI * t) * bend;
      const px = x0 + (x - x0) * t + (nx / nl) * arc, py = y0 + (y - y0) * t + (ny / nl) * arc;
      await tick([async () => {
        cursor.x = px; cursor.y = py;
        if (kind === "pen") await penEv("mouseMoved", px, py, 0);
        else await mouseEv("mouseMoved", px, py);
      }]);
    }
  };
  const click = async (x, y) => {
    await glide(x, y, Math.min(0.75, 0.3 + Math.hypot(x - cursor.x, y - cursor.y) / 2600));
    await tick([() => mouseEv("mousePressed", x, y, { button: "left", buttons: 1, clickCount: 1 })]);
    await tick();
    await tick([() => mouseEv("mouseReleased", x, y, { button: "left", buttons: 0, clickCount: 1 })]);
  };
  const center = async (locator) => {
    const b = await locator.boundingBox();
    if (!b) throw new Error(`${tag} not on screen: ${locator}`);
    return [b.x + b.width / 2, b.y + b.height / 2];
  };
  const clickOn = async (locator) => click(...(await center(locator)));
  const KEYS = { PageDown: [34, "PageDown"], F5: [116, "F5"], Escape: [27, "Escape"] };
  const keyEv = (type, key) => {
    const [code, name] = KEYS[key] ?? [key.toUpperCase().charCodeAt(0), `Key${key.toUpperCase()}`];
    return cdp.send("Input.dispatchKeyEvent", { type, key, code: name, windowsVirtualKeyCode: code, nativeVirtualKeyCode: code, ...(type === "keyDown" && key.length === 1 ? { text: key, type: "keyDown" } : {}) });
  };
  /** Press a key; `cap` animates the caption's keycap. */
  const press = async (key, cap = false) => {
    await tick([async () => { if (cap) await page.evaluate(() => window.__demo.key(true)); await keyEv("keyDown", key); }]);
    for (let n = 0; n < 5; n++) await tick();
    await tick([async () => { await keyEv("keyUp", key); if (cap) await page.evaluate(() => window.__demo.key(false)); }]);
  };
  /** Draw one pen stroke from timed samples; `holdSec` keeps the pen down and
   * still at the end (Smart shapes: hold to snap). */
  const stroke = async (samples, { holdSec = 0, travel = true } = {}) => {
    const [sx, sy] = samples[0];
    if (travel) await glide(sx, sy, Math.min(0.28, 0.05 + Math.hypot(sx - cursor.x, sy - cursor.y) / 4200), "pen");
    const evs = samples.map(([x, y, p], i) => async () => {
      cursor.x = x; cursor.y = y; cursor.show = true;
      await penEv(i === 0 ? "mousePressed" : "mouseMoved", x, y, Math.max(0.05, p));
    });
    for (let i = 0; i < evs.length; i += SUB) await tick(evs.slice(i, i + SUB));
    for (let n = Math.round(holdSec * FPS); n > 0; n--) await tick();
    const [ex, ey] = samples[samples.length - 1];
    await tick([() => penEv("mouseReleased", ex, ey, 0)]);
  };
  const say = (n) => page.evaluate(([n, text, key]) => window.__demo.caption(n, text, key), [n, CAPTIONS[n - 1], n === 3 ? "PgDn" : n === 5 ? "F5" : ""]);

  const finish = async () => {
    ff.stdin.end();
    await ffDone;
    await browser.close();
    process.stdout.write("\n");
    if (errors.length) console.warn(`${tag} page errors:\n  ${[...new Set(errors)].join("\n  ")}`);
    return { master, frames: frame, marks };
  };

  // ================================================================ action
  console.log(`${tag} recording at ${VW}x${VH}, ${FPS} fps${setup.smart ? ", smart shapes on" : ""}`);
  await stepAll(0, true); // virtual time from here on
  let F = await frameRect();

  // 1. Write on any slide
  await say(1);
  await hold(0.35);
  const H = F.h * 0.125;
  const words = handwriting("Projectile motion", { cx: F.x + F.w * 0.5, cy: F.y + F.h * 0.44, height: H });
  for (const s of words) await stroke(s);
  // a quick swoosh under it, below the descender of the "j"
  const ux0 = F.x + F.w * 0.5 - H * 4.3, ux1 = F.x + F.w * 0.5 + H * 4.5, uy = F.y + F.h * 0.44 + H * 0.97;
  const ul = ux1 - ux0;
  await stroke(timeStroke(spline([[ux0, uy + 2], [ux0 + ul * 0.3, uy + 12], [ux0 + ul * 0.7, uy + 9], [ux1, uy - 14]]), { speed: 2300, pressure: 0.5 }));
  await hold(0.25);
  await still("written");
  if (UNTIL <= 1) return finish();

  // 2. Drop in a live lesson from your AI
  await say(2);
  await clickOn(page.locator(".pages .page-thumb").nth(1));
  await hold(0.15);
  // Header: Insert ▾ → Smart Object…
  await clickOn(page.locator('button[aria-label="Insert"]:visible').first());
  await hold(0.3);
  await clickOn(page.locator('.hmenu [role="menuitem"]:has-text("Smart Object"):visible').first());
  await waitFor(() => [...document.querySelectorAll(".preset-card img")].every((i) => i.complete), null, 4);
  await hold(0.2);
  const card = page.locator(".preset-card", { hasText: "Projectile" }).first();
  await glide(...(await center(card)), 0.5);
  await hold(0.12);
  await click(cursor.x, cursor.y);
  await waitFor(() => [...(window.__ctl?.objects.frames.values() ?? [])].some((f) => f.ready), null, 10);
  await press("Escape");
  await glide(VW * 0.9, VH * 0.62, 0.5);
  cursor.show = false;
  await hold(0.6);
  await still("object");
  if (UNTIL <= 2) return finish();

  // 3. Step through with your clicker
  await say(3);
  await hold(0.45);
  await press("PageDown", true);
  await hold(2.2);
  await press("PageDown", true);
  await hold(3.0);
  await still("stepped");
  if (UNTIL <= 3) return finish();

  // 4. Annotate on top
  await say(4);
  await press("p");
  await ink("pen", PEN_W);
  const red = page.getByRole("button", { name: "Colour #e5484d" }).first();
  if (await red.isVisible().catch(() => false)) await clickOn(red);
  await ink("pen", PEN_W, RED);
  F = await frameRect();
  const pk = SPOTS.peak;
  const [ecx, ecy] = at(F, pk.u, pk.v);
  await stroke(timeStroke(spline(ellipsePath(ecx, ecy, F.w * pk.ru, F.h * pk.rv)), { speed: 1500 }), { holdSec: setup.smart ? 0.6 : 0 });
  const [a0, a1, a2] = SPOTS.arrow.map(([u, v]) => at(F, u, v));
  const shaft = quad(a0, a1, a2);
  await stroke(timeStroke(spline(shaft), { speed: 1700 }));
  // arrow head: one "V" stroke at the tip, along the shaft's direction
  const dir = Math.atan2(a2[1] - shaft[34][1], a2[0] - shaft[34][0]), hl = F.h * 0.035;
  const head = [[a2[0] - Math.cos(dir - 0.5) * hl, a2[1] - Math.sin(dir - 0.5) * hl], a2, [a2[0] - Math.cos(dir + 0.5) * hl, a2[1] - Math.sin(dir + 0.5) * hl]];
  await stroke(timeStroke(spline(chaikin(head, 1)), { speed: 900 }));
  await press("h");
  await ink("highlighter", HL_W);
  const [h0, h1] = SPOTS.highlight.map(([u, v]) => at(F, u, v));
  await stroke(timeStroke(spline([h0, [(h0[0] + h1[0]) / 2, h0[1] + 1.5], h1]), { speed: 1400, pressure: 0.6 }));
  await glide(cursor.x + 120, cursor.y + 160, 0.4, "pen");
  await hold(0.5);
  await still("annotated");
  if (UNTIL <= 4) return finish();

  // 5. Present full screen
  await say(5);
  await hold(0.35);
  cursor.show = false;
  await press("F5", true);
  await hold(1.0);
  F = await frameRect();
  await press("l");
  const [lx0, ly0] = at(F, ...SPOTS.launch), [lx1, ly1] = at(F, ...SPOTS.land);
  const [pkx, pky] = at(F, pk.u, pk.v + 0.03);
  const arc = quad([lx0 + F.w * 0.03, ly0 - F.h * 0.06], [pkx, pky - (ly0 - pky) * 0.95], [lx1, ly1 - F.h * 0.02], 60);
  cursor.x = arc[0][0]; cursor.y = arc[0][1];
  await stroke(timeStroke(arc, { speed: 1250 }), { travel: false });
  cursor.show = false;
  await hold(0.7);
  await still("teach");
  if (UNTIL <= 5) return finish();

  // 6. Replay every class
  await say(6);
  await press("Escape");
  await hold(0.35);
  await clickOn(page.locator('button[title^="Replay this page"]:visible').first());
  await waitFor(() => [...document.querySelectorAll('.dock-replay input[type="range"]')].some((e) => e.offsetParent), null, 3);
  await hold(0.2);
  const range = page.locator('.dock-replay input[type="range"]:visible').first();
  if (await range.count()) {
    const b = await range.boundingBox();
    const y = b.y + b.height / 2;
    await glide(b.x + 6, y, 0.4);
    await tick([() => mouseEv("mousePressed", b.x + 6, y, { button: "left", buttons: 1, clickCount: 1 })]);
    const n = Math.round(2.4 * FPS);
    for (let i = 1; i <= n; i++) {
      const x = b.x + 6 + (b.width - 12) * ease(i / n);
      await tick([async () => { cursor.x = x; await mouseEv("mouseMoved", x, y, { button: "left", buttons: 1 }); }]);
    }
    await tick([() => mouseEv("mouseReleased", b.x + b.width - 6, y, { button: "left", buttons: 0, clickCount: 1 })]);
  } else console.warn(`\n${tag} no replay bar`);
  await hold(0.9);
  await still("replayed");

  return finish();
}

// ------------------------------------------------------------------- encode

const run = (args) => {
  const r = spawnSync("ffmpeg", ["-y", "-hide_banner", "-loglevel", "error", ...args], { stdio: "inherit", cwd: WORK });
  if (r.status !== 0) throw new Error(`ffmpeg failed: ${args.join(" ")}`);
};
const probe = (file) => {
  const r = spawnSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", file], { encoding: "utf8" });
  return Number(r.stdout.trim());
};

/** Loop seam: cross-fade the last 0.4 s into the first frame. */
function loopFilter(dur) {
  const fade = 0.4;
  return `[0:v]split[a][b];[b]trim=end_frame=1,setpts=PTS-STARTPTS,loop=loop=${Math.ceil(fade * FPS) + 2}:size=1:start=0,setpts=N/${FPS}/TB,fps=${FPS}[first];` +
    `[a]fps=${FPS},setpts=N/${FPS}/TB[main];[main][first]xfade=transition=fade:duration=${fade}:offset=${(dur - fade).toFixed(3)},format=yuv420p[v]`;
}

function encode(theme, { master, marks }) {
  fs.mkdirSync(OUT, { recursive: true });
  const dur = probe(master);
  const base = path.join(OUT, `${NAME}-${theme}`);
  const kbps = Math.floor((MAX_BYTES * 8) / dur / 1000) - 8;
  const loop = ["-filter_complex", loopFilter(dur), "-map", "[v]", "-an"];
  console.log(`[${theme}] encoding ${dur.toFixed(1)} s at ${kbps} kbps`);
  // MP4 / H.264, two passes to a size budget.
  const x264 = ["-c:v", "libx264", "-preset", "veryslow", "-tune", "animation", "-profile:v", "high", "-level", "4.2", "-b:v", `${kbps}k`, "-maxrate", `${kbps * 3}k`, "-bufsize", `${kbps * 6}k`, "-g", String(FPS * 4)];
  run(["-i", master, ...loop, ...x264, "-pass", "1", "-passlogfile", `x264-${theme}`, "-f", "null", "-"]);
  run(["-i", master, ...loop, ...x264, "-pass", "2", "-passlogfile", `x264-${theme}`, "-movflags", "+faststart", `${base}.mp4`]);
  // WebM / VP9, two passes.
  const vp9 = ["-c:v", "libvpx-vp9", "-b:v", `${kbps}k`, "-deadline", "good", "-row-mt", "1", "-threads", "8", "-tile-columns", "2", "-auto-alt-ref", "1", "-lag-in-frames", "25", "-g", String(FPS * 4)];
  run(["-i", master, ...loop, ...vp9, "-cpu-used", "4", "-pass", "1", "-passlogfile", `vp9-${theme}`, "-f", "null", "-"]);
  run(["-i", master, ...loop, ...vp9, "-cpu-used", "2", "-pass", "2", "-passlogfile", `vp9-${theme}`, `${base}.webm`]);
  // Poster: the annotated board, just before Teach.
  const t = Math.max(0, (marks.annotated ?? dur * 0.6) - 0.05);
  run(["-ss", t.toFixed(3), "-i", master, "-frames:v", "1", "-c:v", "libwebp", "-quality", "82", "-compression_level", "6", `${base}.webp`]);
  for (const ext of ["mp4", "webm", "webp"]) {
    const f = `${base}.${ext}`;
    console.log(`  ${path.relative(ROOT, f)}  ${(fs.statSync(f).size / 1e6).toFixed(2)} MB${ext === "webp" ? "" : `  ${probe(f).toFixed(2)} s`}`);
  }
  for (const f of fs.readdirSync(WORK)) if (/^(x264|vp9)-.*\.log/.test(f)) fs.rmSync(path.join(WORK, f));
}

// --------------------------------------------------------------------- main

const { chromium } = opt("encode-only", false) ? {} : await loadPlaywright();
// Record every take first (the slow, browser part), then encode.
const sidecar = (theme) => path.join(WORK, `${NAME}-${theme}-master.json`);
if (!opt("encode-only", false)) {
  for (const theme of THEMES) {
    const take = await record(chromium, theme);
    fs.writeFileSync(sidecar(theme), JSON.stringify(take));
    console.log(`[${theme}] ${take.frames} frames (${(take.frames / FPS).toFixed(1)} s); marks ${JSON.stringify(take.marks)}`);
    if (PREVIEW) console.log(`[${theme}] draft: ${take.master}\n[${theme}] stills: ${STILLS}`);
  }
}
if (!PREVIEW) {
  for (const theme of THEMES) {
    const take = JSON.parse(fs.readFileSync(sidecar(theme), "utf8"));
    encode(theme, take);
    if (!opt("keep", false)) for (const f of [take.master, sidecar(theme)]) fs.rmSync(f, { force: true });
  }
}
