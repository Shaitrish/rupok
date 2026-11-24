// The "Support Rupok" card: a small corner card in the desktop app, on first
// launch and once more about two weeks later. Never a modal, never in Teach.
// The schedule and rules live in donate.ts.

import { Show } from "solid-js";
import { donateCardOpen, donateNever, donateNotNow, donateNow } from "../donate";
import { Icon } from "./Icon";
import "./DonateCard.css";

export default function DonateCard() {
  return (
    <Show when={donateCardOpen()}>
      <aside class="donate-card" aria-labelledby="donate-card-title" data-testid="donate-card">
        <button class="donate-x" type="button" aria-label="Not now" title="Not now" onClick={donateNotNow}>
          <Icon name="close" size={20} />
        </button>
        <span class="donate-ico" aria-hidden="true"><Icon name="heart" size={22} variant="bold" /></span>
        <p class="donate-title" id="donate-card-title">Rupok is free and open source. Support it ❤️</p>
        <p class="donate-sub">Donations pay for signing certificates, test devices and development time.</p>
        <div class="donate-acts">
          <button class="donate-go" type="button" onClick={donateNow}>Donate</button>
          <button class="donate-later" type="button" onClick={donateNotNow}>Not now</button>
        </div>
        <button class="donate-never" type="button" onClick={donateNever}>Don't show again</button>
      </aside>
    </Show>
  );
}
