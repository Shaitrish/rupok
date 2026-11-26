// The board speed test (Settings → About → Troubleshooting → Board speed
// test): what a teacher runs on a slow board instead of filming it.
//
// It writes on the board by itself for 10 seconds in each drawing mode
// (the wet-ink layer on and off, each ink sharpness), then asks the teacher
// to write for 10 seconds in each mode that matters, then does the scripted
// part once more with the other renderer (a reload in between), and ends
// with one text report: a table of frame rates and pen-to-frame times per
// mode, with the graphics card, screen and browser. The report can be
// copied or saved, and the desktop app also writes it to
// Documents/Rupok/speed-test-<date>.txt.
//
// The scripted pen calls the engine directly, with samples at 240 Hz handed
// over just before each frame, as the browser hands over a real pen's. It
// cannot exercise what happens before that (the digitiser, the OS, the
// browser's event delivery, the native wet-ink layer's own input): that is
// what the "your own writing" part is for.
//
// The lesson is put back exactly as it was (it is saved to memory first and
// loaded again), and so are the settings. See docs/board-test.md.

import { createSignal } from "solid-js";
import { controller } from "./controller";
import { APP_SLUG } from "./brand";
import { type Prefs, pageIndex, pages, prefs, setPrefs, showToast, tool } from "./state";
import { type WritingStats, endRun, graphicsAdapter, notePen, noteSamples, penIsDown, startRun } from "./inkLatency";
import { gov, refreshHz, webgpuTested } from "./perf";
import { notePointerDown, notePointerUp, trailState } from "./wetInk";
import { nativeInkAvailable, nativeInkFailure, nativeInkState } from "./nativeInk";
import { desktopInvoke, isDesktop } from "./platform";

const STORE = `${APP_SLUG}.speedtest`;
const RESUME = `${APP_SLUG}.speedtest.resume`;
/** Which test build wrote a report (bumped with each build sent to a board). */
const BUILD = "4k-perf-5";
const RUN_MS = 10_000;
const PEN_HZ = 240;
/** Rows older than this are another visit's. */
const KEEP_MS = 6 * 3600_000;

type PerfPrefs = Pick<Prefs, "renderScale" | "wetLayer" | "nativeWetInk">;
interface Mode {
  id: string;
  label: string;
  set: PerfPrefs;
}

const MODES: Mode[] = [
  { id: "before", label: "As before (no wet layer, 100 %)", set: { wetLayer: "off", renderScale: "100", nativeWetInk: false } },
  { id: "small", label: "Wet layer small, 100 %", set: { wetLayer: "small", renderScale: "100", nativeWetInk: false } },
  { id: "wet100", label: "Wet layer medium, 100 %", set: { wetLayer: "medium", renderScale: "100", nativeWetInk: false } },
  { id: "large", label: "Wet layer large, 100 %", set: { wetLayer: "large", renderScale: "100", nativeWetInk: false } },
  { id: "wet75", label: "Wet layer medium, 75 %", set: { wetLayer: "medium", renderScale: "75", nativeWetInk: false } },
  { id: "wet50", label: "Wet layer medium, 50 %", set: { wetLayer: "medium", renderScale: "50", nativeWetInk: false } },
  { id: "full50", label: "No wet layer, 50 %", set: { wetLayer: "off", renderScale: "50", nativeWetInk: false } },
];
const NATIVE: Mode = { id: "native", label: "Native wet ink (100 %)", set: { wetLayer: "medium", renderScale: "100", nativeWetInk: true } };
/** The modes the teacher writes in: fewer, each is 10 s of their time. */
const HAND = ["before", "wet100", "wet50"];

export interface Row {
  at: number;
  renderer: string;
  input: "scripted" | "hand";
  mode: string;
  canvas: string;
  note?: string;
  s: WritingStats;
}

export type SpeedTestView =
  | { kind: "running"; step: string; detail: string; hand: boolean }
  | { kind: "report"; text: string; path: string | null }
  | null;

const [view, setView] = createSignal<SpeedTestView>(null);
export const speedTestView = view;
export const closeSpeedTest = () => setView(null);
let skipHand = false;
/** "Skip" on the writing prompt: the rest of the hand-written runs are left out. */
export const skipWriting = () => {
  skipHand = true;
};

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

// ------------------------------------------------------------------- rows

function loadRows(): Row[] {
  try {
    const rows = JSON.parse(localStorage.getItem(STORE) || "[]") as Row[];
    return rows.filter((r) => Date.now() - r.at < KEEP_MS).slice(-80);
  } catch {
    return [];
  }
}
function saveRows(rows: Row[]) {
  try {
    localStorage.setItem(STORE, JSON.stringify(rows));
  } catch { /* storage unavailable */ }
}

function backend(): string {
  try {
    return String(JSON.parse(controller.board.stats_json()).backend).split(" · ")[0].replace("BrowserWebGpu", "WebGPU").replace(/^Gl$/, "WebGL2");
  } catch {
    return "?";
  }
}

function canvasNote(): string {
  try {
    const st = JSON.parse(controller.board.stats_json());
    return `${st.canvas[0]}×${st.canvas[1]}${st.tile ? ` + ${st.tile[0]}×${st.tile[1]}` : ""}`;
  } catch {
    return "?";
  }
}

// ---------------------------------------------------------- the scripted pen

/** Where the pen may write: the part of the page that is on screen (CSS px
 * from the board's corner). */
function writingArea(): [number, number, number, number] {
  const r = controller.root.getBoundingClientRect();
  const [t, rt, b, l] = controller.insets;
  let x0 = l + 16, y0 = t + 16, x1 = r.width - rt - 16, y1 = r.height - b - 16;
  const f = pages()[pageIndex()]?.frame;
  if (f) {
    const [cx, cy, z] = controller.board.camera();
    x0 = Math.max(x0, -cx * z + 16);
    y0 = Math.max(y0, -cy * z + 16);
    x1 = Math.min(x1, (f.w - cx) * z - 16);
    y1 = Math.min(y1, (f.h - cy) * z - 16);
  }
  return [x0, y0, Math.max(x0 + 80, x1), Math.max(y0 + 80, y1)];
}

type Path = { ms: number; at: (u: number) => [number, number] };

/** Handwriting: loops a letter high, marching along lines; every sixth
 * stroke is a long sweep across the page (an underline, an arrow). */
function strokePath(k: number, [x0, y0, x1, y1]: [number, number, number, number]): Path {
  const w = x1 - x0, h = y1 - y0;
  const size = Math.max(36, Math.min(220, h * 0.09));
  if (k % 6 === 5) {
    const y = y0 + h * (0.15 + 0.7 * ((k * 0.37) % 1));
    return { ms: 900, at: (u) => [x0 + w * (0.06 + 0.88 * u), y + size * 0.4 * Math.sin(u * 9)] };
  }
  const perLine = Math.max(3, Math.floor((w * 0.86) / (size * 1.3)));
  const lines = Math.max(1, Math.floor((h - size) / (size * 1.8)));
  const n = k - Math.floor(k / 6);
  const cx = x0 + w * 0.06 + (n % perLine) * size * 1.3 + size * 0.5;
  const cy = y0 + size + (Math.floor(n / perLine) % lines) * size * 1.8;
  return {
    ms: 320,
    at: (u) => [cx + size * 0.5 * Math.sin(u * Math.PI * (2 + (k % 3))) + u * size * 0.6, cy + size * 0.5 * Math.cos(u * Math.PI * (1.5 + (k % 2)))],
  };
}

/** Write for `ms` with the scripted pen and return the frame numbers. */
function scripted(ms: number): Promise<WritingStats> {
  const b = controller.board;
  const id = 9001;
  const area = writingArea();
  const step = 1000 / PEN_HZ;
  return new Promise((resolve) => {
    let end = 0, down = false, k = 0, path: Path | null = null, t0 = 0, last = 0, gapUntil = 0;
    startRun();
    controller.beforeFrame = (now) => {
      end ||= now + ms;
      if (!down) {
        if (now >= end) {
          controller.beforeFrame = null;
          resolve(endRun(gov.refreshMs));
          return;
        }
        if (now < gapUntil) return;
        path = strokePath(k++, area);
        t0 = last = now;
        const [x, y] = path.at(0);
        b.pointer_down(id, x, y, 0.5, now, "pen", 0);
        notePointerDown();
        notePen(true);
        down = true;
        return;
      }
      const flat: number[] = [], stamps: { timeStamp: number }[] = [];
      for (let t = last + step; t <= now; t += step) {
        const u = Math.min(1, (t - t0) / path!.ms);
        const [x, y] = path!.at(u);
        flat.push(x, y, 0.4 + 0.3 * Math.sin(u * Math.PI), t);
        stamps.push({ timeStamp: t });
        last = t;
      }
      if (stamps.length) {
        b.pointer_move(id, new Float64Array(flat), new Float64Array(0));
        noteSamples({ pointerType: "pen" }, stamps);
      }
      if (now - t0 >= path!.ms) {
        b.pointer_up(id);
        notePointerUp();
        notePen(false);
        down = false;
        // The gap between the strokes of a word.
        gapUntil = now + 90;
      }
    };
  });
}

/** Record the teacher's own writing: 10 s from the first touch. Null when
 * nobody wrote (or "Skip"). */
async function byHand(ms: number): Promise<WritingStats | null> {
  const waitUntil = performance.now() + 30_000;
  while (!penIsDown()) {
    if (skipHand || performance.now() > waitUntil) return null;
    await sleep(30);
  }
  startRun();
  const end = performance.now() + ms;
  while (performance.now() < end && !skipHand) await sleep(50);
  const s = endRun(gov.refreshMs);
  return s.frames > 10 ? s : null;
}

// ------------------------------------------------------------------ the run

async function applyMode(m: Mode) {
  setPrefs(m.set);
  controller.applyPerf();
  // The canvas is resized and, for the native layer, its window made.
  await sleep(m.set.nativeWetInk ? 1500 : 700);
}

interface Resume {
  stage: "second" | "report";
  original: Prefs["renderer"];
}

function readResume(): Resume | null {
  try {
    const r = JSON.parse(localStorage.getItem(RESUME) || "null") as Resume | null;
    localStorage.removeItem(RESUME);
    return r && (r.stage === "second" || r.stage === "report") ? r : null;
  } catch {
    return null;
  }
}

/** After the page loaded: carry on a test that reloaded to change renderer. */
export function resumeSpeedTest() {
  const r = readResume();
  if (!r) return;
  if (r.stage === "second") void startSpeedTest("second", r.original);
  else void showReport();
}

let running = false;

/** Run the test. "compare": every mode, the teacher's writing, then the
 * other renderer. "second": the scripted modes again after the reload. */
export async function startSpeedTest(stage: "compare" | "second", original: Prefs["renderer"] = prefs().renderer) {
  if (running) return;
  running = true;
  skipHand = false;
  const b = controller.board;
  const before = { renderScale: prefs().renderScale, wetLayer: prefs().wetLayer, nativeWetInk: prefs().nativeWetInk } satisfies PerfPrefs;
  const smart = prefs().smartShapes;
  const wasTool = tool();
  let lesson: Uint8Array | null = null;
  const page = b.page_index();
  const cam = Array.from(b.camera());
  const rows = stage === "compare" ? [] : loadRows();
  try {
    lesson = b.save();
    controller.setTool("pen");
    // Loops must stay loops: no shape snapping during the test.
    b.set_smart_ink("off", false);
    const renderer = backend();
    let i = 0;
    for (const m of MODES) {
      setView({ kind: "running", step: `Speed test · ${renderer} · ${++i} of ${MODES.length}`, detail: `${m.label}. The board is writing by itself: please do not touch it.`, hand: false });
      await applyMode(m);
      const s = await scripted(RUN_MS);
      rows.push({ at: Date.now(), renderer, input: "scripted", mode: m.label, canvas: canvasNote(), s });
      saveRows(rows);
      await sleep(400);
    }
    if (stage === "compare") {
      const hand = [...MODES.filter((m) => HAND.includes(m.id)), ...(nativeInkAvailable ? [NATIVE] : [])];
      i = 0;
      for (const m of hand) {
        if (skipHand) break;
        await applyMode(m);
        if (m.set.nativeWetInk && nativeInkState() !== "ready") {
          rows.push({ at: Date.now(), renderer, input: "hand", mode: m.label, canvas: canvasNote(), note: `did not start: ${nativeInkFailure() || nativeInkState()}`, s: emptyStats() });
          saveRows(rows);
          continue;
        }
        setView({ kind: "running", step: `Your turn · ${++i} of ${hand.length}`, detail: `${m.label}. Write on the board now, as in class, for 10 seconds. It starts when you touch.`, hand: true });
        const s = await byHand(RUN_MS);
        if (!s) break;
        rows.push({ at: Date.now(), renderer, input: "hand", mode: m.label, canvas: canvasNote(), s });
        saveRows(rows);
        setView({ kind: "running", step: "Lift the pen", detail: "Next mode in a moment…", hand: false });
        await sleep(900);
      }
    }
  } catch (e: any) {
    showToast(`Speed test stopped: ${e?.message ?? e}`, 6000);
  } finally {
    controller.beforeFrame = null;
    setPrefs(before);
    controller.applyPerf();
    try {
      if (lesson) {
        b.load(lesson);
        b.goto_page(page);
        b.set_camera(cam[0], cam[1], cam[2]);
      }
    } catch (e) {
      console.warn("speed test: the lesson could not be put back", e);
    }
    b.set_smart_ink(smart, prefs().scratchOut);
    controller.applyPrefs();
    controller.setTool(wasTool);
    controller.sync(true);
    running = false;
  }
  // The other renderer needs a reload: the scripted part runs again after
  // it, then the first renderer comes back and the report is shown.
  const other: Prefs["renderer"] | null = stage !== "compare" ? null : backend() === "WebGPU" ? "webgl" : webgpuTested() ? "webgpu" : null;
  const next: Resume | null = other ? { stage: "second", original } : stage === "second" ? { stage: "report", original } : null;
  if (!next) return showReport();
  setView({ kind: "running", step: "Speed test", detail: other ? "Switching renderer: the board reloads and carries on by itself." : "Almost done: the board reloads once more.", hand: false });
  try {
    localStorage.setItem(RESUME, JSON.stringify(next));
  } catch {
    return showReport();
  }
  setPrefs({ renderer: other ?? original });
  await controller.autosave().catch(() => {});
  location.reload();
}

const emptyStats = (): WritingStats => ({
  frames: 0, fpsMedian: NaN, fpsP5: NaN, fpsMin: NaN, histogram: [0, 0, 0, 0, 0], latencyP50: NaN, latencyP95: NaN, mainFrames: 0, tileFrames: 0, inputHz: NaN, seconds: 0,
});

// --------------------------------------------------------------- the report

const n = (v: number, d = 0) => (Number.isFinite(v) ? v.toFixed(d) : "—");
const pad = (s: string, w: number) => (s.length >= w ? s : s + " ".repeat(w - s.length));
const padL = (s: string, w: number) => (s.length >= w ? s : " ".repeat(w - s.length) + s);

function table(rows: Row[]): string[] {
  const head = ["Renderer", "Mode", "Canvas (+ wet layer)", "fps med", "p5", "min", "on time", "late 1", "late 2+", "pen→frame p50", "p95", "small/full frames", "input Hz"];
  const body = rows.map((r) => {
    if (r.note) return [r.renderer, r.mode, r.canvas, r.note];
    const h = r.s.histogram, all = Math.max(1, h.reduce((a, c) => a + c, 0));
    const pc = (v: number) => `${Math.round((100 * v) / all)} %`;
    return [
      r.renderer, r.mode, r.canvas, n(r.s.fpsMedian), n(r.s.fpsP5), n(r.s.fpsMin), pc(h[0]), pc(h[1]), pc(h[2] + h[3] + h[4]),
      `${n(r.s.latencyP50)} ms`, `${n(r.s.latencyP95)} ms`, `${r.s.tileFrames}/${r.s.mainFrames}`, n(r.s.inputHz),
    ];
  });
  const widths = head.map((h, i) => Math.max(h.length, ...body.map((b) => (b.length === head.length ? b[i].length : 0))));
  const line = (cells: string[]) => cells.map((c, i) => (i < 3 ? pad(c, widths[i]) : padL(c, widths[i]))).join("  ").trimEnd();
  return [line(head), ...body.map((b) => (b.length === head.length ? line(b) : `${pad(b[0], widths[0])}  ${pad(b[1], widths[1])}  ${pad(b[2], widths[2])}  ${b[3]}`))];
}

async function system(): Promise<string[]> {
  const ua = navigator.userAgent;
  const edge = ua.match(/Edg\/([\d.]+)/), chrome = ua.match(/Chrome\/([\d.]+)/), safari = ua.match(/Version\/([\d.]+).*Safari/), webkit = ua.match(/AppleWebKit\/([\d.]+)/);
  const browser = edge ? `${isDesktop ? "WebView2" : "Edge"} ${edge[1]}` : chrome ? `Chrome ${chrome[1]}` : safari ? `Safari ${safari[1]}` : webkit ? `WebKit ${webkit[1]}` : ua;
  let os = /Windows/.test(ua) ? "Windows" : /Mac OS X/.test(ua) ? "macOS" : /Android/.test(ua) ? "Android" : /Linux/.test(ua) ? "Linux" : "?";
  try {
    const d = await (navigator as any).userAgentData?.getHighEntropyValues?.(["platformVersion", "architecture"]);
    if (d?.platformVersion) {
      // Windows 11 reports platform version 13 and up.
      const major = Number(String(d.platformVersion).split(".")[0]);
      os = os === "Windows" ? `Windows ${major >= 13 ? 11 : 10} (${d.platformVersion}, ${d.architecture ?? ""})` : `${os} ${d.platformVersion}`;
    }
  } catch { /* not in this browser */ }
  const dpr = window.devicePixelRatio || 1;
  const a = graphicsAdapter();
  const r = controller.root.getBoundingClientRect();
  let strokes = 0;
  try {
    strokes = Number(JSON.parse(controller.board.stats_json()).strokes);
  } catch { /* not drawn yet */ }
  const { VERSION } = await import("./components/settings/About");
  return [
    `Rupok ${VERSION} (${BUILD})${isDesktop ? " desktop app" : " in the browser"} · ${os} · ${browser}`,
    `Screen ${Math.round(screen.width * dpr)}×${Math.round(screen.height * dpr)} px, scaling ${Math.round(dpr * 100)} % · refresh ${refreshHz()} Hz (measured)`,
    `Graphics: ${a.software ? "SOFTWARE RENDERING, no GPU: " : ""}${a.name}`,
    `Board ${Math.round(r.width * dpr)}×${Math.round(r.height * dpr)} px · ${strokes} strokes on the page`,
    `OS ink trail: ${trailState()} · native wet ink: ${nativeInkState()}${nativeInkFailure() ? ` (${nativeInkFailure()})` : ""}`,
    `Settings now: renderer ${prefs().renderer}, ink sharpness ${prefs().renderScale}, wet layer ${prefs().wetLayer}, flat panels ${prefs().flatPanels ? "on" : "off"}, smoothing ${prefs().smoothing}`,
  ];
}

async function reportText(rows: Row[]): Promise<string> {
  const when = new Date();
  const out = [`Rupok board speed test · ${when.toLocaleString()}`, ...(await system()), ""];
  const scriptedRows = rows.filter((r) => r.input === "scripted"), hand = rows.filter((r) => r.input === "hand");
  if (scriptedRows.length) out.push(`Scripted pen (${PEN_HZ} Hz, ${RUN_MS / 1000} s per mode)`, ...table(scriptedRows), "");
  if (hand.length) out.push(`Your own writing (${RUN_MS / 1000} s per mode)`, ...table(hand), "");
  if (!rows.length) out.push("No runs were recorded.", "");
  out.push(
    "fps med / p5 / min: frames per second while the pen is down (median, the slowest 5 %, the slowest).",
    "on time / late: share of frames that came at the display's rate, one refresh late, two or more late.",
    "pen→frame: from a pen sample reaching the browser to the frame that shows it being handed to the GPU.",
    "small/full frames: frames that drew only the small wet-ink layer / the whole board canvas.",
    "",
    "Please send this file (or paste this text) to the Rupok team.",
  );
  return out.join("\n");
}

async function showReport() {
  const text = await reportText(loadRows());
  let path: string | null = null;
  if (isDesktop) {
    try {
      const d = new Date();
      const two = (v: number) => String(v).padStart(2, "0");
      const stamp = `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}-${two(d.getHours())}${two(d.getMinutes())}`;
      path = await desktopInvoke<string>("save_speed_report", { stamp, text });
    } catch (e) {
      console.warn("speed test: the report could not be saved", e);
    }
  }
  setView({ kind: "report", text, path });
}

/** "Save as file": a download in the browser, a second copy in the app. */
export function downloadReport(text: string) {
  const a = document.createElement("a");
  a.href = URL.createObjectURL(new Blob([text], { type: "text/plain" }));
  a.download = `rupok-speed-test-${new Date().toISOString().slice(0, 16).replace(/[:T]/g, "-")}.txt`;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 2000);
}
