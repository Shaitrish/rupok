// The "Presets" tab of the Smart Object dialog: ready-made lessons from
// examples/objects/ (presets.ts), filtered by subject. One click puts the
// preset on the current page, scaled to fill the slide frame, and selects it.

import { For, createSignal } from "solid-js";
import { controller } from "../controller";
import { PRESETS, presetHtml, type Preset, type PresetSubject } from "../presets";
import { pageIndex, pages, showToast } from "../state";
import "./PresetGallery.css";

/** Static thumbnails made by scripts/preset-thumbs.mjs. */
const THUMBS = `${import.meta.env.BASE_URL}presets/`;

const SUBJECTS: readonly PresetSubject[] = [...new Set(PRESETS.map((p) => p.subject))];

/** Objects follow the page colour (objects.ts), so the thumbnail does too:
 * a chalkboard or blackboard page shows the dark variant. */
function pageScheme(): "light" | "dark" {
  const rgba = pages()[pageIndex()]?.bg_color ?? 0xffffffff;
  const [r, g, b] = [rgba >>> 24, (rgba >>> 16) & 255, (rgba >>> 8) & 255].map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b < 0.25 ? "dark" : "light";
}

/** Place a preset on the current page, filling the slide frame (keeping its
 * aspect ratio), selected and ready to use. One undo step. */
export async function insertPreset(p: Preset): Promise<boolean> {
  const html = await presetHtml(p.id);
  const frame = pages()[pageIndex()]?.frame;
  if (!frame) return controller.addObject(html, p.title, p.w, p.h, p.steps) !== null;
  const s = Math.min(frame.w / p.w, frame.h / p.h);
  const [w, h] = [Math.round(p.w * s), Math.round(p.h * s)];
  // New objects land on the centre of the view. Centre the view on the frame
  // for the insert, then put the camera back: the object lands on the frame
  // in a single document change.
  const [x, y, zoom] = controller.board.camera();
  controller.board.fit_page();
  const id = controller.addObject(html, p.title, w, h, p.steps);
  controller.board.set_camera(x, y, zoom);
  return id !== null;
}

export function PresetGallery(props: { onInserted: () => void }) {
  const [subject, setSubject] = createSignal<PresetSubject | null>(null);
  const [busy, setBusy] = createSignal(false);
  const shown = () => (subject() ? PRESETS.filter((p) => p.subject === subject()) : PRESETS);
  const add = async (p: Preset) => {
    if (busy()) return;
    setBusy(true);
    try {
      if (await insertPreset(p)) props.onInserted();
    } catch (e: any) {
      showToast(`Could not add “${p.title}”: ${e?.message ?? e}`, 5000);
    } finally {
      setBusy(false);
    }
  };
  const chip = (value: PresetSubject | null, label: string) => (
    <button class="preset-chip" aria-pressed={subject() === value} onClick={() => setSubject(value)}>
      {label}
    </button>
  );
  return (
    <div class="preset-gallery">
      <div class="preset-chips" role="group" aria-label="Filter by subject">
        {chip(null, "All")}
        <For each={SUBJECTS}>{(s) => chip(s, s)}</For>
      </div>
      <div class="preset-grid" aria-busy={busy()}>
        <For each={shown()}>
          {(p) => (
            <button class="preset-card" title={p.description} disabled={busy()} onClick={() => add(p)}>
              <span class="preset-thumb">
                <img
                  src={`${THUMBS}${p.id}-${pageScheme()}.webp`}
                  alt=""
                  width="640"
                  height="360"
                  loading="lazy"
                  decoding="async"
                  onError={(e) => (e.currentTarget.style.visibility = "hidden")}
                />
              </span>
              <span class="preset-title">{p.title}</span>
              <span class="preset-summary">{p.summary}</span>
              <span class="preset-meta">
                {p.subject} · {p.steps} {p.steps === 1 ? "step" : "steps"}
              </span>
            </button>
          )}
        </For>
      </div>
    </div>
  );
}
