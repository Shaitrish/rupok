// Maths live preview (Smart ink → Maths): while the teacher writes, the
// handwriting around the last stroke is read offline (mathInk.ts) and, when
// it looks like maths, its typeset reading floats above it. One tap converts
// the ink to a typeset maths item (one undo step brings the ink back); the
// arrow shows up to three readings. The board never converts on its own.

import { For, Show, createEffect, createSignal, on, onCleanup } from "solid-js";
import { controller } from "../controller";
import { renderTex } from "../katex";
import { PREVIEW_CONFIDENCE, looksLikeMath, mathPlacement, recognize, type MathCandidate } from "../mathInk";
import { camera, editingText, isView, prefs, replay, revision, showToast, tool } from "../state";
import { Icon } from "./Icon";
import { penBusy } from "../wetInk";

interface Cluster {
  ids: string[];
  bounds: [number, number, number, number];
  xy: number[];
  counts: number[];
  color: string;
}

function Tex(props: { latex: string; class?: string }) {
  let el!: HTMLSpanElement;
  createEffect(() => renderTex(el, props.latex));
  return <span ref={el} class={props.class} />;
}

/** Convert the cluster's ink to typeset maths. One undo step. */
export function convertToMath(ids: string[], bounds: number[], latex: string): boolean {
  controller.board.select_json(JSON.stringify(ids));
  const at = mathPlacement(bounds, latex);
  const id = controller.board.replace_selection_with_latex(latex, at.x, at.y, at.w, at.size);
  controller.board.select_json("[]");
  controller.sync(true);
  if (id) showToast("Converted to maths. Undo brings the ink back.");
  return !!id;
}

export function MathPreview() {
  const [cluster, setCluster] = createSignal<(Cluster & { cands: MathCandidate[] }) | null>(null);
  const [more, setMore] = createSignal(false);
  let timer = 0;
  let lastKey = "";
  let dismissed = "";
  const enabled = () =>
    prefs().smartMath && prefs().smartShapes !== "off" && tool() === "pen" && !isView() && !replay() && !editingText();

  const update = () => {
    // Recognition waits for the pen to rest: never mid-stroke, nor in the
    // first frames of the next stroke (wetInk.ts, docs/performance.md).
    if (penBusy(120)) {
      clearTimeout(timer);
      timer = window.setTimeout(update, 120);
      return;
    }
    if (!enabled()) {
      setCluster(null);
      return;
    }
    let c: Cluster | null = null;
    try {
      c = JSON.parse(controller.board.math_cluster_json());
    } catch {
      c = null;
    }
    if (!c || c.counts.length < 2 || c.counts.length > 40) {
      lastKey = "";
      setCluster(null);
      return;
    }
    const key = c.ids.join(",");
    if (key === lastKey) return;
    lastKey = key;
    setMore(false);
    const cands = recognize(c.xy, c.counts);
    const top = cands[0];
    if (key === dismissed || !looksLikeMath(top) || top.confidence < PREVIEW_CONFIDENCE) {
      setCluster(null);
      return;
    }
    setCluster({ ...c, cands: cands.filter((k) => k.confidence >= PREVIEW_CONFIDENCE * 0.6).slice(0, 3) });
  };

  createEffect(on([revision, enabled], () => {
    clearTimeout(timer);
    timer = window.setTimeout(update, 160);
  }));
  onCleanup(() => clearTimeout(timer));

  const pos = () => {
    const c = cluster();
    if (!c) return null;
    const [cx, cy, z] = camera();
    const [x0, y0] = c.bounds;
    const left = Math.max(8, Math.min(window.innerWidth - 360, (x0 - cx) * z));
    const top = (y0 - cy) * z - 12;
    return { left, top: Math.max(72, top) };
  };

  const pick = (latex: string) => {
    const c = cluster();
    if (!c) return;
    convertToMath(c.ids, c.bounds, latex);
    lastKey = "";
    setCluster(null);
  };

  return (
    <Show when={cluster() && pos()}>
      {(p) => (
        <div class="math-preview" style={{ left: `${p().left}px`, top: `${p().top}px` }} role="group" aria-label="Maths preview">
          <div class="math-preview-row surface">
            <button class="math-preview-main" title="Convert to typeset maths" onClick={() => pick(cluster()!.cands[0].latex)}>
              <Tex latex={cluster()!.cands[0].latex} />
            </button>
            <Show when={cluster()!.cands.length > 1}>
              <button class="btn icon math-preview-more" title="Other readings" aria-label="Other readings" aria-expanded={more()} onClick={() => setMore(!more())}>
                <Icon name={more() ? "up" : "down"} size={16} />
              </button>
            </Show>
            <button
              class="btn icon math-preview-more"
              title="Not maths"
              aria-label="Dismiss"
              onClick={() => {
                dismissed = cluster()!.ids.join(",");
                setCluster(null);
              }}
            >
              <Icon name="close" size={16} />
            </button>
          </div>
          <Show when={more()}>
            <div class="math-preview-alts surface">
              <For each={cluster()!.cands.slice(1)}>
                {(k) => (
                  <button class="math-preview-alt" onClick={() => pick(k.latex)} title={`${Math.round(k.confidence * 100)}%`}>
                    <Tex latex={k.latex} />
                  </button>
                )}
              </For>
            </div>
          </Show>
        </div>
      )}
    </Show>
  );
}
