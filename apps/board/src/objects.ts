// Host side of the Smart Object bridge (spec/smart-object.md, protocol v1).
// Each object runs in an opaque-origin sandboxed iframe; we inject the SDK and
// a strict CSP, then talk to it with postMessage.

import sdkSource from "../../../packages/object-sdk/sdk.js?raw";
import type { Controller } from "./controller";
import { type DomItem, setActiveObject, showToast, setTool } from "./state";

/** Theme for objects, read from the app's live design tokens so objects follow
 * the teacher's accent and stay consistent with the UI. */
function theme(): Record<string, string> {
  const css = getComputedStyle(document.documentElement);
  const v = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
  const font = 'Figtree Variable, system-ui, "Segoe UI", Roboto, "Noto Sans Bengali Variable", sans-serif';
  return {
    "color.bg": "#ffffff",
    "color.fg": "#1c2024",
    "color.accent": v("--accent-9", "#3e63dd"),
    "color.accent2": v("--accent-11", "#3a5bc7"),
    "color.muted": "#60646c",
    "color.success": "#30a46c",
    "color.danger": "#e5484d",
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

  constructor(private c: Controller) {
    window.addEventListener("message", (e) => this.onMessage(e));
  }

  /** Build the iframe document for a bundle: CSP + SDK + author HTML. */
  srcdoc(bundle: string): string {
    const cached = this.srcCache.get(bundle);
    if (cached) return cached;
    const html = this.c.board.object_html(bundle) ?? "<p>Missing object</p>";
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
      `<script>${sdkSource.replaceAll("</script", "<\\/script")}</script>`;
    // Put our head first so the CSP and SDK apply before any author code.
    const doc = /<head[^>]*>/i.test(body)
      ? body.replace(/<head[^>]*>/i, (m) => m + head)
      : /<html[^>]*>/i.test(body)
        ? body.replace(/<html[^>]*>/i, (m) => `${m}<head>${head}</head>`)
        : `<!doctype html><html><head>${head}</head><body>${body}</body></html>`;
    this.srcCache.set(bundle, doc);
    return doc;
  }

  register(id: string, iframe: HTMLIFrameElement, item: DomItem) {
    this.frames.set(id, { iframe, item, ready: false });
  }

  unregister(id: string) {
    this.frames.delete(id);
  }

  syncItems(list: DomItem[]) {
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
        if (typeof msg.count === "number") f.item = { ...f.item, steps: Math.max(0, Math.min(500, msg.count | 0)) };
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
    const max = f?.item.steps ?? 500;
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
