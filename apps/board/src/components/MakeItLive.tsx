// "Make it live": a sketched graph (Smart Ink axes and a fitted line,
// parabola or sine curve) becomes the interactive Function grapher preset,
// opened on the fitted equation. One undo brings the sketch back.

import { PRESETS } from "../presets";
import { showToast } from "../state";
import { controller } from "../controller";
import { insertPreset } from "./PresetGallery";

interface Live {
  fam: "quad" | "trig";
  params: Record<string, number>;
  step: number;
  ids: string[];
}

export async function makeItLive(): Promise<boolean> {
  let live: Live | null = null;
  try {
    live = JSON.parse(controller.board.graph_live_json());
  } catch {
    live = null;
  }
  const preset = PRESETS.find((p) => p.id === "math-graph");
  if (!live || !preset) return false;
  // The grapher restores { fam, trig, quad } (examples/objects/math-graph.html).
  const state = { fam: live.fam, [live.fam]: live.params };
  try {
    return await insertPreset(preset, { replace: live.ids, state, step: live.step });
  } catch (e: any) {
    showToast(`Could not make it live: ${e?.message ?? e}`, 5000);
    return false;
  }
}
