// Rupok website: fonts, the draw pad, live Smart Object tabs and the replay demo.
import "@fontsource-variable/inter";
import "@fontsource/hind-siliguri/700.css";
import "./site.css";
import "./pages.css"; // FAQ and five-column footer, shared with the static pages
import sdkSource from "../../../packages/object-sdk/sdk.js?raw";
import { objectFontStyle } from "./objectFonts";
import { PRESETS } from "./presets";
import { currentTheme, onThemeChange, type Theme } from "./theme";
import "./download"; // platform-matched download buttons

// Ink that reads on the current theme. Colours are written as their light
// versions; on a dark page they become the board's dark-board inks.
const DARK_INK: Record<string, string> = { "#111418": "#eef0f2", "#1c7ed6": "#74c0fc", "#e03131": "#ff8787", "#12b886": "#3ee6b0" };
const inkFor = (c: string) => (currentTheme() === "dark" ? (DARK_INK[c] ?? c) : c);

// ------------------------------------------------------------------ pad ----
// A tiny ink pad: smoothed, pressure-aware strokes so visitors feel the pen.
const pad = document.getElementById("pad") as HTMLCanvasElement | null;
if (pad) {
  const ctx = pad.getContext("2d")!;
  let ink = "#111418";
  // Every segment drawn, so a resize or a theme switch can repaint the ink.
  const segs: { x0: number; y0: number; x1: number; y1: number; w: number; ink: string }[] = [];
  const paint = (s: (typeof segs)[number]) => {
    ctx.strokeStyle = inkFor(s.ink);
    ctx.lineWidth = s.w;
    ctx.beginPath();
    ctx.moveTo(s.x0, s.y0);
    ctx.lineTo(s.x1, s.y1);
    ctx.stroke();
  };
  const repaint = () => {
    ctx.clearRect(0, 0, pad.width, pad.height);
    segs.forEach(paint);
  };
  const fit = () => {
    const r = pad.getBoundingClientRect();
    const d = devicePixelRatio || 1;
    pad.width = r.width * d;
    pad.height = r.height * d;
    ctx.setTransform(d, 0, 0, d, 0, 0);
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    segs.forEach(paint);
  };
  fit();
  addEventListener("resize", fit);
  let last: { x: number; y: number; w: number } | null = null;
  const pt = (e: PointerEvent) => {
    const r = pad.getBoundingClientRect();
    const p = e.pointerType === "pen" && e.pressure > 0 ? e.pressure : 0.5;
    return { x: e.clientX - r.left, y: e.clientY - r.top, w: 2 + p * 6 };
  };
  pad.addEventListener("pointerdown", (e) => {
    pad.setPointerCapture(e.pointerId);
    last = pt(e);
  });
  pad.addEventListener("pointermove", (e) => {
    if (!last) return;
    for (const c of e.getCoalescedEvents ? e.getCoalescedEvents() : [e]) {
      const p = pt(c);
      const w: number = last.w * 0.6 + p.w * 0.4; // smooth width
      const s = { x0: last.x, y0: last.y, x1: p.x, y1: p.y, w, ink };
      segs.push(s);
      paint(s);
      last = { ...p, w };
    }
  });
  addEventListener("pointerup", () => (last = null));
  const swatches = () =>
    document.querySelectorAll<HTMLButtonElement>(".inks button").forEach((b) => {
      b.style.setProperty("--c", inkFor(b.dataset.ink!));
      // The default ink is black on a light page and white on a dark one.
      if (b.dataset.ink === "#111418") b.setAttribute("aria-label", currentTheme() === "dark" ? "White" : "Black");
    });
  swatches();
  onThemeChange(() => {
    swatches();
    repaint();
  });
  document.querySelectorAll<HTMLButtonElement>(".inks button").forEach((b) => {
    b.addEventListener("click", () => {
      document.querySelectorAll(".inks button").forEach((x) => x.classList.remove("on"));
      b.classList.add("on");
      ink = b.dataset.ink!;
    });
  });
  document.querySelector(".clear")?.addEventListener("click", () => {
    segs.length = 0;
    repaint();
  });
}

// -------------------------------------------------------- live lessons ----
// Real Smart Object presets from examples/objects, running in the same
// sandbox + SDK as the board. New presets appear automatically.
const files = import.meta.glob("../../../examples/objects/*.html", { query: "?raw", import: "default" });
// Step counts come from PRESETS (presets.ts).
const META: Record<string, { label: string; color: string; prompt: string }> = {
  "physics-projectile": { label: "Physics", color: "#4dabf7", prompt: "Make a projectile-motion simulation for Class 9: angle and speed controls, Earth vs Moon vs Mars, and the equations step by step." },
  "chemistry-molecules-3d": { label: "Chemistry 3D", color: "#ff6b57", prompt: "Show water, methane and benzene in 3D. Build each molecule atom by atom and label the bond angles." },
  "physics-circuit": { label: "Circuits", color: "#ffd43b", prompt: "An Ohm’s law circuit: battery, resistor and bulb, with electrons that speed up when I raise the voltage." },
  "chemistry-reaction": { label: "Reactions", color: "#ff922b", prompt: "Animate methane burning at particle level and balance the equation step by step." },
  "math-graph": { label: "Maths", color: "#20c997", prompt: "A graph of y = a·sin(bx + c) with sliders, so students can see what each letter does." },
  "binary-counter": { label: "ICT", color: "#20c997", prompt: "A binary counter for Class 7: reveal the place values one by one, then let students toggle bits." },
  "computing-timeline": { label: "History", color: "#4dabf7", prompt: "A history-of-computing timeline from the abacus to AI, one era per click." },
  "quiz-networking": { label: "Quiz", color: "#ff6b57", prompt: "A four-question quiz on networking basics with instant feedback and a score at the end." },
};
const ORDER = ["physics-projectile", "chemistry-molecules-3d", "physics-circuit", "chemistry-reaction", "math-graph", "binary-counter", "computing-timeline", "quiz-networking"];

function srcdoc(html: string): string {
  const lib = `${location.origin}/rupoklib/`;
  const csp = [
    "default-src 'none'",
    `script-src 'unsafe-inline' 'unsafe-eval' blob: data: ${lib}`,
    `style-src 'unsafe-inline' data: ${lib}`,
    `img-src data: blob: ${lib}`,
    `font-src data: ${lib}`,
    "connect-src 'none'",
  ].join("; ");
  // Theme fonts (objectFonts.ts): the sandboxed frame can't see this page's fonts.
  const head = `<meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${csp}">${objectFontStyle(lib)}<meta name="rupok-lib" content="${lib}"><script>${sdkSource.replaceAll("</script", "<\\/script")}</script>`;
  const body = html.replaceAll("rupoklib://", lib);
  return /<head[^>]*>/i.test(body) ? body.replace(/<head[^>]*>/i, (m) => m + head) : `<!doctype html><html><head>${head}</head><body>${body}</body></html>`;
}

// The object theme the board sends (objects.ts theme()): a white page in the
// light theme, the blackboard page in the dark one. Mint accent.
const FONT = '"Inter Variable", Inter, "Hind Siliguri", system-ui, sans-serif';
const THEMES: Record<Theme, Record<string, string>> = {
  light: {} as Record<string, string>,
  dark: {
    scheme: "dark", "color.bg": "#17191c", "color.surface": "rgba(255,255,255,0.06)", "color.line": "rgba(255,255,255,0.14)",
    "color.fg": "#eef0f2", "color.muted": "#a6adb7", "color.accent": "#3ee6b0", "color.accent2": "#74c0fc",
    "color.accent-text": "#63e6be", "color.on-accent": "#04241b", "color.accent-soft": "#0b2b23",
    "color.success": "#69db7c", "color.danger": "#ff8787",
    "font.body": FONT, "font.heading": FONT, "font.mono": "ui-monospace, Menlo, monospace", radius: "12px",
  },
};
const THEME = {
  scheme: "light", "color.bg": "#ffffff", "color.surface": "rgba(17,20,24,0.04)", "color.line": "rgba(17,20,24,0.12)",
  "color.fg": "#111418", "color.muted": "#5f6670", "color.accent": "#0ca678", "color.accent2": "#1c7ed6",
  // Contrast-safe accent companions (spec "Theme"); dark pages use #3ee6b0 / #63e6be / #04241b / #0b2b23.
  "color.accent-text": "#087f5b", "color.on-accent": "#04241b", "color.accent-soft": "#e6fcf5",
  "color.success": "#2b8a3e", "color.danger": "#e03131",
  "font.body": '"Inter Variable", Inter, "Hind Siliguri", system-ui, sans-serif', "font.heading": '"Inter Variable", Inter, "Hind Siliguri", system-ui, sans-serif',
  "font.mono": "ui-monospace, Menlo, monospace", radius: "12px",
};

const demo = document.getElementById("demo") as HTMLIFrameElement | null;
const tabs = document.getElementById("tabs");
const promptEl = document.getElementById("demo-prompt");
let step = 0;
let steps = 0;
let demoReady = false;
THEMES.light = THEME;
// Re-theme the live object when the visitor switches the site theme.
onThemeChange((t) => {
  if (demo && demoReady) demo.contentWindow?.postMessage({ rupok: 1, type: "theme", theme: THEMES[t] }, "*");
});

addEventListener("message", (e) => {
  if (!demo || e.source !== demo.contentWindow || !e.data || e.data.rupok !== 1) return;
  const m = e.data;
  if (m.type === "hello") {
    demoReady = true;
    demo.contentWindow!.postMessage({ rupok: 1, type: "init", id: "demo", step, steps, state: null, theme: THEMES[currentTheme()], mode: "present" }, "*");
  } else if (m.type === "goto" && typeof m.step === "number") {
    step = Math.max(0, Math.min(steps, m.step));
    demo.contentWindow!.postMessage({ rupok: 1, type: "step", step }, "*");
  } else if (m.type === "steps" && typeof m.count === "number") {
    steps = m.count;
  } else if (m.type === "key" && (m.key === "ArrowRight" || m.key === "PageDown")) {
    step = Math.min(steps, step + 1);
    demo.contentWindow!.postMessage({ rupok: 1, type: "step", step }, "*");
  }
});

async function show(id: string) {
  const loader = Object.entries(files).find(([p]) => p.endsWith(`/${id}.html`))?.[1];
  if (!loader || !demo) return;
  const meta = META[id];
  steps = PRESETS.find((p) => p.id === id)?.steps ?? 0;
  // Start fully built so visitors see the finished object; they can step back.
  step = steps;
  demoReady = false;
  demo.srcdoc = srcdoc((await loader()) as string);
  if (promptEl) promptEl.textContent = `“${meta?.prompt ?? "Make an interactive lesson."}”`;
  tabs?.querySelectorAll("button").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.id === id)));
}

if (tabs) {
  const available = new Set(Object.keys(files).map((p) => p.split("/").pop()!.replace(/\.html$/, "")));
  const ids = ORDER.filter((id) => available.has(id));
  tabs.innerHTML = ids
    .map((id) => `<button role="tab" data-id="${id}" style="--c:${META[id].color}"><span class="dot"></span>${META[id].label}</button>`)
    .join("");
  tabs.querySelectorAll<HTMLButtonElement>("button").forEach((b) => b.addEventListener("click", () => show(b.dataset.id!)));
  // Load the first demo when the section comes into view.
  const io = new IntersectionObserver((es) => {
    if (es.some((e) => e.isIntersecting) && ids[0]) {
      show(ids[0]);
      io.disconnect();
    }
  }, { rootMargin: "200px" });
  io.observe(tabs);
}

// --------------------------------------------------------------- replay ----
// A short "class" replayed stroke by stroke: a slide title, a diagram and notes.
const rp = document.getElementById("rp") as HTMLCanvasElement | null;
if (rp) {
  const ctx = rp.getContext("2d")!;
  type P = { x: number; y: number; t: number };
  const strokes: { pts: P[]; color: string; w: number }[] = [];
  let clock = 0;
  const add = (fn: (u: number) => [number, number], n: number, color: string, w: number, dur: number, gap = 350) => {
    const pts: P[] = [];
    for (let i = 0; i <= n; i++) {
      const [x, y] = fn(i / n);
      pts.push({ x, y, t: clock + (dur * i) / n });
    }
    clock += dur + gap;
    strokes.push({ pts, color, w });
  };
  // Underline, a parabola, axes, an arrow and a circled answer.
  add((u) => [70 + u * 330, 92 + Math.sin(u * 9) * 2], 40, "#12b886", 6, 700);
  add((u) => [90, 330 - u * 210], 20, "#111418", 4, 400);
  add((u) => [90 + u * 470, 330], 30, "#111418", 4, 500);
  add((u) => [110 + u * 400, 330 - 4 * 190 * u * (1 - u)], 60, "#1c7ed6", 5, 1400);
  add((u) => [330 + Math.cos(u * 6.4) * 70, 140 + Math.sin(u * 6.4) * 30], 50, "#e03131", 4, 900);
  add((u) => [520 - u * 90, 220 - u * 50], 14, "#111418", 4, 350);
  add((u) => [430 + u * 18, 170 + u * 2], 6, "#111418", 4, 160, 120);
  add((u) => [430 + u * 4, 170 + u * 18], 6, "#111418", 4, 160);
  const total = clock;
  const range = document.getElementById("rp-range") as HTMLInputElement;
  const time = document.getElementById("rp-time")!;
  const btn = document.getElementById("rp-play") as HTMLButtonElement;
  let t = 0;
  let playing = false;
  let lastNow = 0;
  const fit = () => {
    const r = rp.getBoundingClientRect();
    const d = devicePixelRatio || 1;
    rp.width = r.width * d;
    rp.height = r.height * d;
    ctx.setTransform((r.width / 640) * d, 0, 0, (r.width / 640) * d, 0, 0);
  };
  const draw = () => {
    ctx.clearRect(0, 0, 640, 400);
    ctx.fillStyle = inkFor("#111418");
    ctx.font = "800 34px 'Inter Variable', Inter, sans-serif";
    ctx.fillText("Projectile motion", 70, 78);
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    for (const s of strokes) {
      ctx.strokeStyle = inkFor(s.color);
      ctx.lineWidth = s.w;
      ctx.beginPath();
      let started = false;
      for (const p of s.pts) {
        if (p.t > t) break;
        started ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y);
        started = true;
      }
      ctx.stroke();
    }
    range.value = String(Math.round((t / total) * 1000));
    const sec = Math.floor((t / total) * 42); // presented as a 42-second clip
    time.textContent = `0:${String(sec).padStart(2, "0")}`;
  };
  const tick = (now: number) => {
    if (playing) {
      t = Math.min(total, t + (now - lastNow));
      if (t >= total) {
        playing = false;
        btn.textContent = "▶";
      }
    }
    lastNow = now;
    draw();
    requestAnimationFrame(tick);
  };
  fit();
  addEventListener("resize", fit);
  btn.addEventListener("click", () => {
    if (t >= total) t = 0;
    playing = !playing;
    btn.textContent = playing ? "❚❚" : "▶";
  });
  range.addEventListener("input", () => {
    t = (Number(range.value) / 1000) * total;
    playing = false;
    btn.textContent = "▶";
  });
  // Auto-play once when scrolled into view.
  new IntersectionObserver((es) => {
    if (es.some((e) => e.isIntersecting) && t === 0) {
      playing = true;
      btn.textContent = "❚❚";
    }
  }, { threshold: 0.5 }).observe(rp);
  requestAnimationFrame(tick);
}

// ----------------------------------------------------------- hero video ----
// A silent looping demo that behaves like a GIF: it plays while on screen and
// pauses off screen. With reduced motion it waits for a click (the inline
// script after the <video> drops autoplay). It follows the site theme; both
// takes share one timeline, so a switch keeps the playback position.
const video = document.getElementById("hero-video") as HTMLVideoElement | null;
if (video) {
  const big = document.querySelector<HTMLButtonElement>(".vplay");
  const small = document.querySelector<HTMLButtonElement>(".vtoggle");
  let wanted = !matchMedia("(prefers-reduced-motion: reduce)").matches; // the visitor wants it playing
  let started = wanted;
  let visible = false;
  const render = () => {
    if (big) big.hidden = started;
    if (small) {
      small.hidden = !started;
      small.classList.toggle("paused", !wanted);
      small.setAttribute("aria-label", wanted ? "Pause the demo" : "Play the demo");
      small.title = wanted ? "Pause" : "Play";
    }
  };
  const sync = () => {
    if (wanted && visible) void video.play().catch(() => {});
    else if (!video.paused) video.pause();
    render();
  };
  const toggle = () => {
    wanted = !wanted;
    started = true;
    sync();
  };
  big?.addEventListener("click", toggle);
  small?.addEventListener("click", toggle);
  video.addEventListener("click", toggle);
  new IntersectionObserver((es) => {
    visible = es.some((e) => e.isIntersecting);
    sync();
  }, { threshold: 0.2 }).observe(video);
  onThemeChange((t) => {
    const at = video.currentTime;
    const swap = (u: string) => u.replace(/-(light|dark)\.(webm|mp4|webp)$/, `-${t}.$2`);
    video.poster = swap(video.poster);
    video.querySelectorAll("source").forEach((s) => (s.src = swap(s.src)));
    video.load();
    video.addEventListener("loadedmetadata", () => {
      if (at) video.currentTime = at;
      sync();
    }, { once: true });
  });
  render();
}
