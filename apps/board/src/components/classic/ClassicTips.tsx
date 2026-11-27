// Readable tooltips for the Classic islands: the name in a teacher's words
// and the shortcut as key chips (tips.ts), 15 px text in a panel-styled
// bubble above the button. They replace the browser's small native
// tooltip there (the button keeps its aria-label). Mouse: after a short
// rest; keyboard: on focus.

import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { type Tip, tipFor, tipLang } from "./tips";

const SCOPE = ".islands button, .dock-pop.classic .cl-x";
const DELAY_MS = 450;

export function ClassicTips() {
  const [tip, setTip] = createSignal<{ tip: Tip; x: number; y: number; below: boolean } | null>(null);
  let timer = 0;
  const show = (btn: HTMLElement) => {
    const label = btn.getAttribute("aria-label") ?? "";
    const t = tipFor(label) ?? (label ? { en: label, bn: label } : null);
    if (!t) return;
    // The native tooltip would show too: our bubble replaces it here.
    if (btn.title) {
      btn.dataset.title = btn.title;
      btn.removeAttribute("title");
    }
    const r = btn.getBoundingClientRect();
    const below = r.top < 120;
    setTip({ tip: t, x: r.left + r.width / 2, y: below ? r.bottom + 10 : r.top - 10, below });
  };
  const hide = () => {
    clearTimeout(timer);
    setTip(null);
  };
  onMount(() => {
    // Island buttons show only this tooltip: their `title` moves to
    // data-title as they appear (the aria-label stays for screen readers).
    const strip = () => {
      for (const b of Array.from(document.querySelectorAll<HTMLElement>(".islands [title]"))) {
        b.dataset.title = b.title;
        b.removeAttribute("title");
      }
    };
    strip();
    const islandsEl = document.querySelector(".islands");
    const mo = new MutationObserver(strip);
    if (islandsEl) mo.observe(islandsEl, { subtree: true, childList: true, attributes: true, attributeFilter: ["title"] });
    onCleanup(() => mo.disconnect());
    const over = (e: PointerEvent) => {
      const btn = (e.target as Element | null)?.closest?.(SCOPE) as HTMLElement | null;
      clearTimeout(timer);
      if (!btn || e.pointerType !== "mouse") return hide();
      timer = window.setTimeout(() => show(btn), DELAY_MS);
    };
    const focus = (e: FocusEvent) => {
      const btn = (e.target as Element | null)?.closest?.(SCOPE) as HTMLElement | null;
      if (btn?.matches(":focus-visible")) show(btn);
      else hide();
    };
    document.addEventListener("pointerover", over, true);
    document.addEventListener("pointerdown", hide, true);
    document.addEventListener("focusin", focus, true);
    document.addEventListener("focusout", hide, true);
    onCleanup(() => {
      hide();
      document.removeEventListener("pointerover", over, true);
      document.removeEventListener("pointerdown", hide, true);
      document.removeEventListener("focusin", focus, true);
      document.removeEventListener("focusout", hide, true);
    });
  });
  const lang = tipLang();
  return (
    <Show when={tip()}>
      {(t) => (
        <div
          class="cl-tip"
          classList={{ below: t().below }}
          role="tooltip"
          style={{ left: `${t().x}px`, top: `${t().y}px` }}
        >
          <span>{t().tip[lang]}</span>
          <Show when={t().tip.keys?.length}>
            <span class="cl-keys">
              <Show when={t().tip.hold}><span class="cl-hold">{lang === "bn" ? "চেপে রাখুন" : "Hold"}</span></Show>
              <For each={t().tip.keys}>{(k) => <kbd>{k}</kbd>}</For>
            </span>
          </Show>
        </div>
      )}
    </Show>
  );
}
