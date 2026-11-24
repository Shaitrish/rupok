// Settings → About → "Check for updates": the button and its answer inline.

import { Show, createSignal } from "solid-js";
import type { SettingProps } from "../settings/registry";
import { isDesktop } from "../platform";
import { applyWebUpdate, checkRelease, checkWeb } from "./updates";

export function UpdateCheck(props: SettingProps) {
  const [busy, setBusy] = createSignal(false);
  const [answer, setAnswer] = createSignal("");
  const [reload, setReload] = createSignal(false);
  const run = async () => {
    setBusy(true);
    setAnswer("Checking…");
    setReload(false);
    try {
      if (isDesktop) {
        const r = await checkRelease({ force: true });
        setAnswer(
          r.status === "newer" ? `Rupok ${r.latest} is available: see the card on the board.`
          : r.status === "latest" ? `You have the latest version (${r.current}).`
          : "Couldn’t reach GitHub. Check the internet connection and try again later.",
        );
      } else {
        const r = await checkWeb({ force: true });
        setReload(r === "newer");
        setAnswer(
          r === "newer" ? "A new version of Rupok is ready."
          : r === "latest" ? "You have the latest version. The board also works offline in this browser."
          : "Couldn’t check: you seem to be offline. The board keeps working.",
        );
      }
    } finally {
      setBusy(false);
    }
  };
  return (
    <div class="st-inline">
      <Show when={answer()}>
        <span class="st-status" role="status">{answer()}</span>
      </Show>
      <Show when={reload()}>
        <button class="st-btn solid" onClick={() => void applyWebUpdate()}>Reload</button>
      </Show>
      <button class="st-btn" aria-describedby={props.descId} disabled={busy()} onClick={() => void run()} data-update-check>
        Check for updates
      </button>
    </div>
  );
}
