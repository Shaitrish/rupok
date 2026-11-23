// Settings → Touch & gestures → "Calibrate my board": what the board reports
// for a finger and a palm, and the button that measures it again.

import { Show, createSignal } from "solid-js";
import { gestures } from "../../gesturePrefs";
import type { SettingProps } from "../../settings/registry";
import { Calibration } from "../GestureSettings";
import { Icon } from "../Icon";

export function GestureCalibration(props: SettingProps) {
  const [open, setOpen] = createSignal(false);
  const cal = () => gestures().calibration;
  return (
    <div class="st-callout">
      <Icon name="hand" size={20} />
      <div class="st-callout-text" role="status" id={props.descId}>
        <b id={props.labelId}>Calibrate my board</b>
        <Show
          when={cal()}
          fallback={<span>Not calibrated yet. A touch much wider than a finger, or 3 or more touches close together, erases.</span>}
        >
          {(c) => (
            <Show
              when={c().reportsSize}
              fallback={<span>Your board does not report touch size, so Rupok uses hand-cluster detection{c().contacts >= 3 ? ` (your palm showed as ${c().contacts} touches)` : ""}.</span>}
            >
              <span>A finger is about {c().finger} px and your palm about {c().palm} px; touches wider than {Math.round(gestures().palmSize)} px erase.</span>
            </Show>
          )}
        </Show>
      </div>
      <button type="button" class="st-btn" onClick={() => setOpen(true)}>
        {cal() ? "Calibrate again" : "Calibrate"}
      </button>
      <Show when={open()}>
        <Calibration onClose={() => setOpen(false)} />
      </Show>
    </div>
  );
}
