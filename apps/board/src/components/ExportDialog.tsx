// Export as PDF: the dialog (pages, layout, print-friendly, ink), the
// Ctrl+Shift+E shortcut and progress. The exporter itself (and pdf-lib) is
// loaded on first use, so the board's first load does not grow.

import { For, Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import { isView, pageIndex, pages, showToast, title } from "../state";
import { savePdfBytes } from "../platform";
import { APP_SLUG } from "../brand";
import { Icon } from "./Icon";
import type { ExportProgress, Layout } from "../export-pdf";
import "./ExportDialog.css";

const [open, setOpen] = createSignal(false);

/** Show the export dialog (top bar buttons, Ctrl+Shift+E). */
export const openExport = () => setOpen(true);

const PREFS_KEY = `${APP_SLUG}.export`;
interface Saved {
  layout: Layout;
  printFriendly: boolean;
  includeInk: boolean;
}
function loadSaved(): Saved {
  const d: Saved = { layout: "slides", printFriendly: false, includeInk: true };
  try {
    const s = JSON.parse(localStorage.getItem(PREFS_KEY) || "{}");
    return {
      layout: ["slides", "handout3", "handout2"].includes(s.layout) ? s.layout : d.layout,
      printFriendly: typeof s.printFriendly === "boolean" ? s.printFriendly : d.printFriendly,
      includeInk: typeof s.includeInk === "boolean" ? s.includeInk : d.includeInk,
    };
  } catch {
    return d;
  }
}

/** "1-3, 5, 8-" → 0-based indexes in the order typed, or an error message. */
export function parseRange(text: string, count: number): number[] | string {
  const out: number[] = [];
  const seen = new Set<number>();
  const parts = text.split(/[,;\s]+/).filter(Boolean);
  if (!parts.length) return "Type the pages to export, for example 1-3, 5";
  for (const part of parts) {
    const m = part.match(/^(\d*)\s*[-–]\s*(\d*)$/) ?? part.match(/^(\d+)$/);
    if (!m) return `“${part}” is not a page or a range`;
    const single = m.length === 2;
    const a = single ? Number(m[1]) : m[1] ? Number(m[1]) : 1;
    const b = single ? a : m[2] ? Number(m[2]) : count;
    if (a < 1 || b < 1 || a > count || b > count) return `This lesson has pages 1 to ${count}`;
    const step = a <= b ? 1 : -1;
    for (let p = a; step > 0 ? p <= b : p >= b; p += step) {
      if (!seen.has(p)) {
        seen.add(p);
        out.push(p - 1);
      }
    }
  }
  return out;
}

const fileName = () => `${(title() || "lesson").replace(/[\\/:*?"<>|]+/g, "-").trim() || "lesson"}.pdf`;
const mb = (n: number) => (n < 1024 * 1024 ? `${Math.max(1, Math.round(n / 1024))} KB` : `${(n / 1024 / 1024).toFixed(1)} MB`);

function Seg<T extends string>(props: { value: T; options: readonly (readonly [T, string])[]; onChange: (v: T) => void; label: string }) {
  return (
    <div class="seg" role="radiogroup" aria-label={props.label}>
      <For each={props.options}>
        {([v, label]) => (
          <button role="radio" aria-checked={props.value === v} classList={{ on: props.value === v }} onClick={() => props.onChange(v)}>
            {label}
          </button>
        )}
      </For>
    </div>
  );
}

function ExportDialog(props: { onClose: () => void }) {
  const saved = loadSaved();
  const [which, setWhich] = createSignal<"all" | "current" | "range">("all");
  const [range, setRange] = createSignal(`1-${Math.max(1, pages().length)}`);
  const [layout, setLayout] = createSignal<Layout>(saved.layout);
  const [printFriendly, setPrintFriendly] = createSignal(saved.printFriendly);
  const [includeInk, setIncludeInk] = createSignal(saved.includeInk);
  const [busy, setBusy] = createSignal(false);
  const [progress, setProgress] = createSignal<ExportProgress | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const verb = () => (isView() ? "Download PDF" : "Export PDF");
  const darkPages = createMemo(() => pages().some((p) => (p.bg_color >>> 8) < 0x808080));
  let abort: AbortController | null = null;
  let go!: HTMLButtonElement;

  const selection = createMemo((): number[] | string => {
    const n = pages().length;
    if (which() === "all") return Array.from({ length: n }, (_, i) => i);
    if (which() === "current") return [Math.min(pageIndex(), n - 1)];
    return parseRange(range(), n);
  });

  const persist = () => {
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify({ layout: layout(), printFriendly: printFriendly(), includeInk: includeInk() }));
    } catch { /* storage unavailable */ }
  };

  const run = async () => {
    const sel = selection();
    if (typeof sel === "string" || !sel.length || busy()) return;
    persist();
    setBusy(true);
    setError(null);
    setProgress({ done: 0, total: sel.length, label: "Loading the exporter…" });
    const ac = new AbortController();
    abort = ac;
    try {
      const { exportPdf } = await import("../export-pdf");
      const t0 = performance.now();
      const bytes = await exportPdf(
        controller,
        { pages: sel, layout: layout(), printFriendly: printFriendly(), includeInk: includeInk(), title: title() },
        setProgress,
        ac.signal,
      );
      if (ac.signal.aborted) return;
      console.info(`rupok: exported ${sel.length} page(s) as PDF in ${Math.round(performance.now() - t0)} ms, ${bytes.length} bytes`);
      setProgress({ done: sel.length, total: sel.length, label: "Saving…" });
      const where = await savePdfBytes(fileName(), bytes);
      if (where) {
        showToast(`${isView() ? "Downloaded" : "Exported"} ${where.split(/[\\/]/).pop()} · ${mb(bytes.length)}`, 4000);
        props.onClose();
      }
    } catch (e: any) {
      if (!ac.signal.aborted) setError(`Could not export the PDF: ${e?.message ?? e}`);
    } finally {
      abort = null;
      setBusy(false);
      setProgress(null);
    }
  };

  const close = () => {
    abort?.abort();
    props.onClose();
  };

  onMount(() => go?.focus());
  onCleanup(() => abort?.abort());

  const pct = () => {
    const p = progress();
    return p && p.total ? Math.round((p.done / p.total) * 100) : 0;
  };

  return (
    <div class="scrim" onClick={(e) => e.target === e.currentTarget && !busy() && close()} onKeyDown={(e) => e.key === "Escape" && close()}>
      <div class="dialog export-dialog" role="dialog" aria-modal="true" aria-label={verb()}>
        <div class="dialog-head">
          <div class="badge"><Icon name={isView() ? "download" : "export"} size={24} active /></div>
          <div style={{ flex: 1 }}>
            <h2>{verb()}</h2>
            <p class="sub">
              {isView() ? "Keep the lesson as a PDF to read, print or write on." : "Share the lesson as a PDF, or print handouts for the class."}
            </p>
          </div>
          <button class="btn icon" onClick={close} aria-label="Close"><Icon name="close" /></button>
        </div>

        <div class="field">
          <label>Pages</label>
          <div class="export-row">
            <Seg
              label="Pages"
              value={which()}
              options={[["all", `All ${pages().length}`], ["current", `Current · ${pageIndex() + 1}`], ["range", "Range"]] as const}
              onChange={setWhich}
            />
            <Show when={which() === "range"}>
              <input
                class="input export-range"
                value={range()}
                onInput={(e) => setRange(e.currentTarget.value)}
                onKeyDown={(e) => e.key === "Enter" && run()}
                aria-label="Page range"
                placeholder="1-3, 5"
                spellcheck={false}
              />
            </Show>
          </div>
          <Show when={typeof selection() === "string"}>
            <span class="export-hint error">{selection() as string}</span>
          </Show>
        </div>

        <div class="field">
          <label>Layout</label>
          <div class="layout-cards" role="radiogroup" aria-label="Layout">
            <button class="layout-card" classList={{ on: layout() === "slides" }} role="radio" aria-checked={layout() === "slides"} onClick={() => setLayout("slides")}>
              <span class="mini" aria-hidden="true"><span class="mini-slide"><i /><i /></span></span>
              <span><b>Slides</b><span>One per page, shaped like the slide</span></span>
            </button>
            <button
              class="layout-card"
              classList={{ on: layout() !== "slides" }}
              role="radio"
              aria-checked={layout() !== "slides"}
              onClick={() => layout() === "slides" && setLayout("handout3")}
            >
              <span class="mini" aria-hidden="true">
                <span class="mini-sheet"><i><em /><s /></i><i><em /><s /></i><i><em /><s /></i></span>
              </span>
              <span><b>Handout</b><span>A4 with lines for notes, for students</span></span>
            </button>
          </div>
          <Show when={layout() !== "slides"}>
            <div class="export-row" style={{ "margin-top": "4px" }}>
              <Seg label="Slides per page" value={layout()} options={[["handout3", "3 per page"], ["handout2", "2 per page"]] as const} onChange={setLayout} />
            </div>
          </Show>
        </div>

        <div class="export-checks">
          <label class="check">
            <input type="checkbox" checked={includeInk()} onChange={(e) => setIncludeInk(e.currentTarget.checked)} />
            <span>Include ink<small>Pen and highlighter. Shapes are always kept.</small></span>
          </label>
          <label class="check">
            <input type="checkbox" checked={printFriendly()} onChange={(e) => setPrintFriendly(e.currentTarget.checked)} />
            <span>
              Print-friendly
              <small>{darkPages() ? "Chalkboard and dark pages become white paper; light ink turns dark." : "Dark pages become white paper (this lesson has none)."}</small>
            </span>
          </label>
        </div>

        <Show when={error()}>
          <div class="error-box">{error()}</div>
        </Show>

        <div class="actions export-actions">
          <Show when={progress()} fallback={<span class="export-hint">{fileName()}</span>}>
            {(p) => (
              <div class="export-progress" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct()} aria-label="Export progress">
                <div class="bar"><div style={{ width: `${pct()}%` }} /></div>
                <span>{p().label}</span>
              </div>
            )}
          </Show>
          <div class="grow" />
          <button class="btn" onClick={close}>Cancel</button>
          <button class="btn solid" ref={go} disabled={busy() || typeof selection() === "string"} onClick={run}>
            <Icon name={isView() ? "download" : "export"} />
            <span>{busy() ? "Exporting…" : verb()}</span>
          </button>
        </div>
      </div>
    </div>
  );
}

/** Mount once (in the top bar): the dialog and the Ctrl+Shift+E shortcut. */
export function ExportHost() {
  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || !e.shiftKey || e.altKey || e.key.toLowerCase() !== "e") return;
      const t = e.target as HTMLElement | null;
      if (t && (t.isContentEditable || t.tagName === "INPUT" || t.tagName === "TEXTAREA")) return;
      // Capture phase, before the board's own Ctrl+E (merge ink).
      e.preventDefault();
      e.stopImmediatePropagation();
      setOpen(true);
    };
    window.addEventListener("keydown", onKey, true);
    onCleanup(() => window.removeEventListener("keydown", onKey, true));
  });
  return (
    <Show when={open()}>
      <ExportDialog onClose={() => setOpen(false)} />
    </Show>
  );
}
