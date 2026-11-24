// A small accessible menu for the top bar: the Insert ▾ menu and the ⋯
// overflow menu. Menu-button pattern (WAI-ARIA APG): Enter, Space or ↓ opens
// on the first item, ↑ on the last; ↑ ↓ Home End move; Esc closes and returns
// focus to the button; Tab and a click outside close. The popup is portalled
// to <body> and placed with `position: fixed`, so it is never clipped by the
// bar (the bar's `.surface` has a backdrop filter, a containing block for
// fixed children) and never counts toward the bar's width.

import { For, Match, Show, Switch, createSignal, createUniqueId, onCleanup } from "solid-js";
import { Portal } from "solid-js/web";
import type { IconName } from "../icons.generated";
import { Icon } from "./Icon";
import "./Header.css";

export type MenuEntry =
  | {
      kind?: "item";
      /** Stable id, also `data-id` on the element (tests, agents). */
      id: string;
      label: string;
      icon: IconName;
      /** Shortcut shown on the right, e.g. "Ctrl O". */
      key?: string;
      /** Secondary text on the right instead of a shortcut (e.g. "On"). */
      state?: string;
      disabled?: boolean;
      /** Radio-like current choice (mode items). */
      checked?: boolean;
      /** A small status dot before the label (AI connected). */
      dot?: boolean;
      run: () => void;
    }
  | { kind: "sep" }
  | { kind: "label"; label: string };

export function MenuButton(props: {
  /** Accessible name of the button. */
  label: string;
  /** Tooltip (with the shortcut, if any). */
  title: string;
  icon: IconName;
  /** Visible text next to the icon (hidden in the compact bar). */
  text?: string;
  caret?: boolean;
  /** A dot on the button (e.g. the AI is connected while it sits in ⋯). */
  dot?: boolean;
  /** Which edge of the button the menu lines up with. */
  align?: "left" | "right";
  class?: string;
  id?: string;
  entries: () => MenuEntry[];
}) {
  const [open, setOpen] = createSignal(false);
  const [pos, setPos] = createSignal<{ top: number; left?: number; right?: number; maxH: number }>({ top: 0, maxH: 400 });
  const menuId = createUniqueId();
  let btn!: HTMLButtonElement;
  let menu: HTMLDivElement | undefined;

  const items = () => (menu ? Array.from(menu.querySelectorAll<HTMLButtonElement>("[role^=menuitem]:not(:disabled)")) : []);
  const focusAt = (i: number) => {
    const all = items();
    if (!all.length) return;
    all[(i + all.length) % all.length].focus();
  };

  const place = () => {
    const r = btn.getBoundingClientRect();
    const top = Math.round(r.bottom + 6);
    const maxH = Math.max(160, window.innerHeight - top - 8);
    if ((props.align ?? "right") === "right") setPos({ top, right: Math.max(8, Math.round(window.innerWidth - r.right)), maxH });
    else setPos({ top, left: Math.max(8, Math.round(r.left)), maxH });
  };

  const away = (e: PointerEvent) => {
    const t = e.target as Node;
    if (!btn.contains(t) && !menu?.contains(t)) close(false);
  };
  // Phones fire resize when the address bar or keyboard moves: follow the
  // button instead of closing. Escape anywhere closes (focus may be elsewhere
  // after a touch).
  const onResize = () => place();
  const onEsc = (e: KeyboardEvent) => e.key === "Escape" && close(true);
  const show = (focus: "first" | "last" = "first") => {
    place();
    setOpen(true);
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("resize", onResize);
    window.addEventListener("keydown", onEsc);
    queueMicrotask(() => focusAt(focus === "first" ? 0 : -1));
  };
  function close(refocus: boolean) {
    if (!open()) return;
    setOpen(false);
    window.removeEventListener("pointerdown", away, true);
    window.removeEventListener("resize", onResize);
    window.removeEventListener("keydown", onEsc);
    if (refocus) btn.focus();
  }
  onCleanup(() => close(false));

  const onBtnKey = (e: KeyboardEvent) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      show(e.key === "ArrowDown" ? "first" : "last");
    }
  };
  const onMenuKey = (e: KeyboardEvent) => {
    const all = items();
    const i = all.indexOf(document.activeElement as HTMLButtonElement);
    // Keep board shortcuts (arrows page, letters pick tools) out of the menu.
    e.stopPropagation();
    switch (e.key) {
      case "ArrowDown": e.preventDefault(); focusAt(i + 1); break;
      case "ArrowUp": e.preventDefault(); focusAt(i < 0 ? -1 : i - 1); break;
      case "Home": e.preventDefault(); focusAt(0); break;
      case "End": e.preventDefault(); focusAt(-1); break;
      case "Escape": e.preventDefault(); close(true); break;
      case "Tab": close(false); break;
      default:
        // Type-ahead: the first letter jumps to the next matching item.
        if (e.key.length === 1 && /\S/.test(e.key) && !e.ctrlKey && !e.metaKey && !e.altKey) {
          const k = e.key.toLowerCase();
          const n = all.length;
          for (let d = 1; d <= n; d++) {
            const el = all[(i + d + n) % n];
            if (el.textContent?.trim().toLowerCase().startsWith(k)) { el.focus(); break; }
          }
        }
    }
  };
  const pick = (run: () => void) => {
    close(true);
    run();
  };

  return (
    <>
      <button
        ref={btn}
        id={props.id}
        data-tour={props.id}
        class={`btn hdr-menu-btn ${props.class ?? ""}`}
        classList={{ icon: !props.text, open: open() }}
        aria-label={props.label}
        title={props.title}
        aria-haspopup="menu"
        aria-expanded={open()}
        aria-controls={open() ? menuId : undefined}
        onClick={() => (open() ? close(false) : show())}
        onKeyDown={onBtnKey}
      >
        <Icon name={props.icon} />
        <Show when={props.text}>
          <span class="label">{props.text}</span>
        </Show>
        <Show when={props.caret}>
          <svg class="hdr-caret" viewBox="0 0 12 12" width="12" height="12" aria-hidden="true"><path d="M3 4.5 6 7.5 9 4.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </Show>
        <Show when={props.dot}>
          <span class="hdr-btn-dot" aria-hidden="true" />
        </Show>
      </button>
      <Show when={open()}>
        <Portal>
          <div
            ref={menu}
            id={menuId}
            class="hmenu"
            role="menu"
            aria-label={props.label}
            style={{
              top: `${pos().top}px`,
              left: pos().left !== undefined ? `${pos().left}px` : undefined,
              right: pos().right !== undefined ? `${pos().right}px` : undefined,
              "max-height": `${pos().maxH}px`,
            }}
            onKeyDown={onMenuKey}
          >
            <For each={props.entries()}>
              {(e) => (
                <Switch>
                  <Match when={e.kind === "sep"}>
                    <div class="hmenu-sep" role="separator" />
                  </Match>
                  <Match when={e.kind === "label" && e}>
                    {(l) => <div class="hmenu-label" role="presentation">{(l() as { label: string }).label}</div>}
                  </Match>
                  <Match when={(e.kind ?? "item") === "item" && (e as Extract<MenuEntry, { id: string }>)}>
                    {(it) => (
                      <button
                        class="hmenu-item"
                        role={it().checked !== undefined ? "menuitemradio" : "menuitem"}
                        aria-checked={it().checked !== undefined ? it().checked : undefined}
                        classList={{ on: !!it().checked }}
                        data-id={it().id}
                        tabIndex={-1}
                        disabled={it().disabled}
                        title={it().key ? `${it().label}  ·  ${it().key}` : undefined}
                        onClick={() => pick(it().run)}
                      >
                        <Icon name={it().icon} size={20} active={!!it().checked} />
                        <span class="hmenu-text">{it().label}</span>
                        <Show when={it().dot}><span class="hmenu-dot" aria-hidden="true" /></Show>
                        <Show when={it().state}><span class="hmenu-state">{it().state}</span></Show>
                        <Show when={it().key}><kbd>{it().key}</kbd></Show>
                      </button>
                    )}
                  </Match>
                </Switch>
              )}
            </For>
          </div>
        </Portal>
      </Show>
    </>
  );
}
