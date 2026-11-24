// Host side of the Smart Object bridge (spec/smart-object.md, protocol v1).
// Each object runs in an opaque-origin sandboxed iframe; we inject the SDK and
// a strict CSP, then talk to it with postMessage.

import sdkSource from "../../../packages/object-sdk/sdk.js?raw";
import type { Controller } from "./controller";
import { objectFontStyle } from "./objectFonts";
import { type DomItem, pageIndex, pages, setActiveObject, showToast, setTool } from "./state";

/** Theme for objects. Objects sit on the slide, so they follow the *page*
 * colour: a white page gets a light object, a chalkboard or blackboard page a
 * dark one. The accent follows the teacher's chosen accent. */
function pageColor(): string {
  const p = pages()[pageIndex()];
  return "#" + ((p?.bg_color ?? 0xffffffff) >>> 8).toString(16).padStart(6, "0");
}

function isDark(hex: string): boolean {
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b < 0.25;
}

/** Accent colours for objects, chosen by the *object's* scheme (the page),
 * not the app UI theme — a white page under a dark UI must still get the
 * light-scheme accent, or bright mint lands on white. Values mirror
 * styles.css. accent: fills/strokes (≥3:1 on the page); text: accent-coloured
 * text (≥4.5:1); on: text on an accent fill; soft: highlight background. */
type AccentSet = { accent: string; text: string; on: string; soft: string };
const ACCENTS: Record<string, { light: AccentSet; dark: AccentSet }> = {
  mint: { light: { accent: "#0ca678", text: "#087f5b", on: "#04241b", soft: "#e6fcf5" }, dark: { accent: "#3ee6b0", text: "#63e6be", on: "#04241b", soft: "#0b2b23" } },
  graphite: { light: { accent: "#111418", text: "#111418", on: "#ffffff", soft: "#f1f3f5" }, dark: { accent: "#eef0f2", text: "#eef0f2", on: "#111418", soft: "#25282d" } },
  blue: { light: { accent: "#1c7ed6", text: "#1864ab", on: "#ffffff", soft: "#e7f5ff" }, dark: { accent: "#4dabf7", text: "#74c0fc", on: "#061a2e", soft: "#0c2440" } },
  grass: { light: { accent: "#2f9e44", text: "#2b8a3e", on: "#ffffff", soft: "#ebfbee" }, dark: { accent: "#51cf66", text: "#69db7c", on: "#0a1f0f", soft: "#102a17" } },
  coral: { light: { accent: "#fa5a45", text: "#c92a2a", on: "#2b0b06", soft: "#fff4e6" }, dark: { accent: "#ff8a78", text: "#ffa294", on: "#2b0b06", soft: "#33150f" } },
  sun: { light: { accent: "#fab005", text: "#a06800", on: "#2b2200", soft: "#fff9db" }, dark: { accent: "#ffd43b", text: "#ffe066", on: "#2b2200", soft: "#2d2508" } },
};

function accentFor(dark: boolean): AccentSet {
  const name = document.documentElement.dataset.accent ?? "mint";
  const set = ACCENTS[name] ?? ACCENTS.mint;
  return dark ? set.dark : set.light;
}

/** The theme for objects on a page of colour `bg` (`#rrggbb`); the current
 * page by default. PDF export passes the exported page's colour. */
export function theme(bg: string = pageColor()): Record<string, string> {
  const dark = isDark(bg);
  const accent = accentFor(dark);
  const font = '"Inter Variable", Inter, "Hind Siliguri", system-ui, "Segoe UI", Roboto, sans-serif';
  return {
    scheme: dark ? "dark" : "light",
    "color.bg": bg,
    "color.surface": dark ? "rgba(255, 255, 255, 0.06)" : "rgba(17, 20, 24, 0.04)",
    "color.line": dark ? "rgba(255, 255, 255, 0.14)" : "rgba(17, 20, 24, 0.12)",
    "color.fg": dark ? "#eef0f2" : "#111418",
    "color.muted": dark ? "#a6adb7" : "#5f6670",
    "color.accent": accent.accent,
    "color.accent-text": accent.text,
    "color.on-accent": accent.on,
    "color.accent-soft": accent.soft,
    "color.accent2": dark ? "#74c0fc" : "#1c7ed6",
    "color.success": dark ? "#69db7c" : "#2b8a3e",
    "color.danger": dark ? "#ff8787" : "#e03131",
    "font.body": font,
    "font.heading": font,
    "font.mono": 'ui-monospace, "Cascadia Mono", Menlo, Consolas, monospace',
    radius: "12px",
  };
}

interface Frame {
  iframe: HTMLIFrameElement;
  item: DomItem;
  ready: boolean;
}

export interface QuizAnswer {
  objectId: string;
  title: string;
  payload: unknown;
  at: number;
}

export class ObjectHost {
  frames = new Map<string, Frame>();
  srcCache = new Map<string, string>();
  quizLog: QuizAnswer[] = [];
  /** Step counts objects declared at run time (`steps`), e.g. the 3D
   * viewer's saved views; they outlive item syncs. */
  runtimeSteps = new Map<string, number>();

  /** An object's step count: what it declared at run time, else its manifest's. */
  stepsOf(it: DomItem): number {
    return this.runtimeSteps.get(it.id) ?? it.steps ?? 0;
  }

  constructor(private c: Controller) {
    window.addEventListener("message", (e) => this.onMessage(e));
  }

  /** Build the iframe document for a bundle: CSP + SDK + author HTML. */
  srcdoc(bundle: string): string {
    const cached = this.srcCache.get(bundle);
    if (cached) return cached;
    const doc = this.wrap(this.c.board.object_html(bundle) ?? "<p>Missing object</p>");
    this.srcCache.set(bundle, doc);
    return doc;
  }

  /** The iframe document for raw object HTML (an agent's live preview, a
   * student's own layer): the same CSP and SDK as a placed object. */
  wrap(html: string): string {
    const lib = `${location.origin}/rupoklib/`;
    const body = html.replaceAll("rupoklib://", lib);
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
      // Theme fonts: the frame can't see the app's @fontsource fonts.
      objectFontStyle(lib) +
      // Tells the SDK where rupoklib is, so built-in LaTeX can load KaTeX.
      `<meta name="rupok-lib" content="${lib}">` +
      `<script>${sdkSource.replaceAll("</script", "<\\/script")}</script>`;
    // Put our head first so the CSP and SDK apply before any author code.
    const doc = /<head[^>]*>/i.test(body)
      ? body.replace(/<head[^>]*>/i, (m) => m + head)
      : /<html[^>]*>/i.test(body)
        ? body.replace(/<html[^>]*>/i, (m) => `${m}<head>${head}</head>`)
        : `<!doctype html><html><head>${head}</head><body>${body}</body></html>`;
    return doc;
  }

  register(id: string, iframe: HTMLIFrameElement, item: DomItem) {
    this.frames.set(id, { iframe, item, ready: false });
  }

  unregister(id: string) {
    this.frames.delete(id);
  }

  private lastPageBg = "";

  syncItems(list: DomItem[]) {
    // Objects follow the page colour; re-theme them when it changes.
    const bg = pageColor();
    if (bg !== this.lastPageBg) {
      this.lastPageBg = bg;
      document.documentElement.style.setProperty("--page-bg", bg);
      this.broadcastTheme();
    }
    for (const it of list) {
      const f = this.frames.get(it.id);
      if (!f) continue;
      const prevStep = f.item.step;
      f.item = it;
      if (f.ready && prevStep !== it.step) this.post(f, { type: "step", step: it.step ?? 0 });
    }
  }

  private post(f: Frame, msg: Record<string, unknown>) {
    f.iframe.contentWindow?.postMessage({ rupok: 1, ...msg }, "*");
  }

  private onMessage(e: MessageEvent) {
    const msg = e.data;
    if (!msg || msg.rupok !== 1) return;
    let found: [string, Frame] | undefined;
    for (const entry of this.frames) {
      if (entry[1].iframe.contentWindow === e.source) {
        found = entry;
        break;
      }
    }
    if (!found) return;
    const [id, f] = found;
    switch (msg.type) {
      case "hello": {
        f.ready = true;
        let state: unknown;
        try {
          state = f.item.state ? JSON.parse(f.item.state) : undefined;
        } catch { state = undefined; }
        this.post(f, {
          type: "init", id, step: f.item.step ?? 0, steps: f.item.steps ?? 0, state, theme: theme(), mode: "present",
        });
        break;
      }
      case "state":
        try {
          const s = JSON.stringify(msg.state);
          if (s.length <= 256 * 1024) this.c.board.set_object_state(id, s);
        } catch { /* not serialisable */ }
        break;
      case "goto":
        if (typeof msg.step === "number") this.setStep(id, msg.step);
        break;
      case "steps":
        // Runtime step count: remembered for presenter navigation.
        if (typeof msg.count === "number") this.runtimeSteps.set(id, Math.max(0, Math.min(500, msg.count | 0)));
        break;
      case "resize":
        if (typeof msg.w === "number" && typeof msg.h === "number") {
          const w = Math.min(8000, Math.max(50, msg.w));
          const h = Math.min(8000, Math.max(50, msg.h));
          this.c.board.set_geom(id, f.item.x, f.item.y, w, h);
        }
        break;
      case "quiz":
        this.quizLog.push({ objectId: id, title: f.item.title ?? "", payload: msg.payload, at: Date.now() });
        showToast(`Quiz answer recorded (${this.quizLog.length} total)`);
        break;
      case "key":
        // Presenter keys the object didn't use (clicker after touching an object).
        if (msg.key === "PageDown" || msg.key === "ArrowRight" || msg.key === "ArrowDown") this.c.next();
        else if (msg.key === "PageUp" || msg.key === "ArrowLeft" || msg.key === "ArrowUp") this.c.prev();
        else if (msg.key === "Escape") this.c.setTool("pen");
        break;
      case "log":
        console.log(`[object ${f.item.title}]`, msg.level, msg.message);
        break;
    }
  }

  /** Push the current theme to every live object (after an accent change). */
  broadcastTheme() {
    const t = theme();
    for (const f of this.frames.values()) if (f.ready) this.post(f, { type: "theme", theme: t });
  }

  setStep(id: string, step: number) {
    const f = this.frames.get(id);
    const max = f ? this.stepsOf(f.item) : 500;
    const s = Math.max(0, Math.min(max, step | 0));
    this.c.board.set_object_step(id, s);
    if (f?.ready) this.post(f, { type: "step", step: s });
  }

  activate(id: string) {
    setActiveObject(id);
    setTool("interact");
    this.c.setTool("interact");
  }
}
