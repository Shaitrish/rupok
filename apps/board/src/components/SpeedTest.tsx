// The board speed test's two faces (speedTest.ts): a small card while it
// runs, and the report with Copy and Save at the end.

import { Match, Show, Switch, createSignal } from "solid-js";
import { closeSpeedTest, downloadReport, skipWriting, speedTestView } from "../speedTest";
import { Icon } from "./Icon";

export default function SpeedTestLayer() {
  const [copied, setCopied] = createSignal(false);
  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // No clipboard permission: select the text so Ctrl+C takes it.
      const pre = document.querySelector(".speed-report pre");
      if (pre) getSelection()?.selectAllChildren(pre);
      document.execCommand?.("copy");
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };
  return (
    <Switch>
      <Match when={speedTestView()?.kind === "running" ? (speedTestView() as { step: string; detail: string; hand: boolean }) : null}>
        {(v) => (
          // Nothing here animates or counts down: the test measures the board, not this card.
          <div class="speed-card" classList={{ hand: v().hand }} role="status">
            <b>{v().step}</b>
            <span>{v().detail}</span>
            <Show when={v().hand}>
              <button class="btn soft" onClick={skipWriting}>Skip the writing part</button>
            </Show>
          </div>
        )}
      </Match>
      <Match when={speedTestView()?.kind === "report" ? (speedTestView() as { text: string; path: string | null }) : null}>
        {(v) => (
          <div class="scrim" onPointerDown={(e) => e.target === e.currentTarget && closeSpeedTest()}>
            <div class="dialog speed-report" role="dialog" aria-label="Board speed test report">
              <div class="dialog-head">
                <div class="badge"><Icon name="play" size={22} /></div>
                <div>
                  <h2>Board speed test</h2>
                  <p class="sub">
                    <Show when={v().path} fallback="Copy the report or save it as a file, and send it to the Rupok team.">
                      Saved to <b>{v().path}</b>. Send that file, or copy the text below.
                    </Show>
                  </p>
                </div>
              </div>
              <pre tabindex="0">{v().text}</pre>
              <div class="actions">
                <button class="btn solid" onClick={() => copy(v().text)}>{copied() ? "Copied" : "Copy"}</button>
                <button class="btn soft" onClick={() => downloadReport(v().text)}>Save as file</button>
                <button class="btn" onClick={closeSpeedTest}>Close</button>
              </div>
            </div>
          </div>
        )}
      </Match>
    </Switch>
  );
}
