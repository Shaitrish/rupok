// The update card: top right under the bar, small, dismissible, never in
// Teach mode and never modal (updates.ts decides when there is something).

import { Show } from "solid-js";
import { mode } from "../state";
import { Icon, Mark } from "../components/Icon";
import { applyWebUpdate, dismissNotice, notice, skipVersion } from "./updates";
import "./updates.css";

export function UpdateNotice() {
  const shown = () => (mode() === "teach" ? null : notice());
  const desktop = () => {
    const n = shown();
    return n?.kind === "desktop" ? n : null;
  };
  return (
    <Show when={shown()}>
      {(n) => (
        <div class="update-card surface" role="status" aria-live="polite" data-update={n().kind}>
          <Mark size={28} />
          <div class="update-body">
            <Show
              when={desktop()}
              fallback={
                <>
                  <p class="update-title">A new version of Rupok is ready</p>
                  <div class="update-actions">
                    <button class="btn solid" onClick={() => void applyWebUpdate()}>
                      <Icon name="restart" size={20} /><span class="label">Reload</span>
                    </button>
                  </div>
                </>
              }
            >
              {(d) => (
                <>
                  <p class="update-title">Rupok {d().version} is available</p>
                  <div class="update-actions">
                    <a class="btn" href={d().notes} target="_blank" rel="noreferrer">What’s new</a>
                    <a class="btn solid" href={d().download} target="_blank" rel="noreferrer" onClick={() => dismissNotice()}>
                      <Icon name="download" size={20} /><span class="label">Download</span>
                    </a>
                  </div>
                  <button class="update-skip" onClick={() => skipVersion(d().version)}>Skip this version</button>
                </>
              )}
            </Show>
          </div>
          <button class="btn icon update-close" onClick={dismissNotice} title="Not now" aria-label="Dismiss update notice">
            <Icon name="close" size={20} />
          </button>
        </div>
      )}
    </Show>
  );
}
