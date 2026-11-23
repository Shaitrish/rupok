// Settings: a sidebar of categories, a search box over every setting, and
// the selected category on the right. Everything shown comes from the
// settings registry (settings/registry.ts); this file never lists a setting.
// Open it with `openSettings(target?)` (settings/open.ts).

import { For, Show, createEffect, createMemo, createSignal, on, onCleanup, onMount } from "solid-js";
import "../../settings"; // registers the built-in settings
import {
  type SettingDef, categories, categoryById, groupsOf, resetCategory, resettable, resolveTarget, searchSettings,
} from "../../settings/registry";
import { type SettingsRequest } from "../../settings/open";
import { Icon } from "../Icon";
import { MicButton } from "../MicButton";
import { SettingRow, domId } from "./Controls";
import "./settings.css";

let lastCategory = "general";

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function SearchIcon() {
  return (
    <svg class="icon" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round">
      <circle cx="11" cy="11" r="6.5" />
      <path d="M16 16l4 4" />
    </svg>
  );
}

export function SettingsDialog(props: { request: SettingsRequest; onClose: () => void }) {
  const [cat, setCat] = createSignal(categoryById(lastCategory) ? lastCategory : (categories()[0]?.id ?? "general"));
  const [query, setQuery] = createSignal("");
  const [active, setActive] = createSignal(0);
  const [hl, setHl] = createSignal<string | null>(null);
  const [armed, setArmed] = createSignal(false);
  let dialog!: HTMLDivElement;
  let pane!: HTMLDivElement;
  let search!: HTMLInputElement;
  let hlTimer: number | undefined;
  let armTimer: number | undefined;
  onCleanup(() => { clearTimeout(hlTimer); clearTimeout(armTimer); });

  createEffect(() => { lastCategory = cat(); });

  const hits = createMemo(() => searchSettings(query()));
  const searching = () => query().trim() !== "";
  const counts = createMemo(() => {
    const m = new Map<string, number>();
    for (const h of hits()) m.set(h.setting.category, (m.get(h.setting.category) ?? 0) + 1);
    return m;
  });
  const current = () => categoryById(cat());
  const groups = createMemo(() => groupsOf(cat()));
  const canReset = () => groups().some((g) => g.items.some(resettable));

  /** Show a setting: its category, scrolled into view, highlighted, focused. */
  const go = (id: string, focus = true) => {
    const r = resolveTarget(id);
    if (!r) return;
    setQuery("");
    setCat(r.category);
    setArmed(false);
    if (!r.setting) {
      pane && (pane.scrollTop = 0);
      return;
    }
    const setting = r.setting;
    // Two frames: the pane re-renders, then lays out.
    requestAnimationFrame(() => requestAnimationFrame(() => {
      const row = dialog?.querySelector<HTMLElement>(`#${domId(setting)}`);
      if (!row) return;
      row.scrollIntoView({ block: "center", behavior: "auto" });
      setHl(setting);
      clearTimeout(hlTimer);
      hlTimer = window.setTimeout(() => setHl(null), 2400);
      if (focus) row.querySelector<HTMLElement>(`.st-control ${FOCUSABLE}, ${FOCUSABLE}`)?.focus({ preventScroll: true });
    }));
  };

  // Deep links: every openSettings() call, including while already open.
  createEffect(on(() => props.request.seq, () => {
    const t = props.request.target;
    if (t) go(t);
  }));

  onMount(() => {
    // Keyboard users land in search; on touch boards that would pop the
    // on-screen keyboard, so focus the dialog instead.
    if (!props.request.target) {
      if (matchMedia("(pointer: fine)").matches) search.focus();
      else dialog.focus();
    }
  });

  const pickCategory = (id: string) => {
    setQuery("");
    setCat(id);
    setArmed(false);
    pane.scrollTop = 0;
  };

  const reset = () => {
    if (!armed()) {
      setArmed(true);
      clearTimeout(armTimer);
      armTimer = window.setTimeout(() => setArmed(false), 3500);
      return;
    }
    setArmed(false);
    resetCategory(cat());
  };

  const onKey = (e: KeyboardEvent) => {
    // Keys typed in Settings never reach the board (tools, pages, undo).
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      if (searching()) { setQuery(""); search.focus(); }
      else props.onClose();
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
      e.preventDefault();
      search.focus();
      search.select();
      return;
    }
    if (e.key === "Tab") {
      const els = [...dialog.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.offsetParent !== null);
      if (!els.length) return;
      const first = els[0], last = els[els.length - 1];
      if (e.shiftKey && (document.activeElement === first || document.activeElement === dialog)) { e.preventDefault(); last.focus(); }
      else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
    }
  };

  const onSearchKey = (e: KeyboardEvent) => {
    const n = hits().length;
    if (e.key === "ArrowDown" && n) { e.preventDefault(); setActive((active() + 1) % n); }
    else if (e.key === "ArrowUp" && n) { e.preventDefault(); setActive((active() - 1 + n) % n); }
    else if (e.key === "Enter" && n) { e.preventDefault(); go(hits()[Math.min(active(), n - 1)].setting.id); }
  };
  createEffect(on(query, () => setActive(0)));
  createEffect(on(active, (i) => {
    if (searching()) dialog?.querySelector(`#st-hit-${i}`)?.scrollIntoView({ block: "nearest" });
  }));

  const onNavKey = (e: KeyboardEvent, i: number) => {
    const list = categories();
    let j = -1;
    if (e.key === "ArrowDown" || e.key === "ArrowRight") j = (i + 1) % list.length;
    else if (e.key === "ArrowUp" || e.key === "ArrowLeft") j = (i - 1 + list.length) % list.length;
    else if (e.key === "Home") j = 0;
    else if (e.key === "End") j = list.length - 1;
    if (j < 0) return;
    e.preventDefault();
    pickCategory(list[j].id);
    dialog.querySelector<HTMLElement>(`#st-tab-${list[j].id}`)?.focus();
  };

  const path = (s: SettingDef) => [categoryById(s.category)?.label, s.group].filter(Boolean).join(" › ");

  return (
    <div class="scrim st-scrim" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <div class="st-dialog" ref={dialog} role="dialog" aria-modal="true" aria-labelledby="st-title" tabIndex={-1} onKeyDown={onKey}>
        <aside class="st-side">
          <div class="st-side-head">
            <span class="st-side-title">Settings</span>
          </div>
          <label class="st-search">
            <SearchIcon />
            <input
              ref={search}
              type="search"
              placeholder="Search settings"
              aria-label="Search settings"
              aria-controls="st-results"
              aria-activedescendant={searching() && hits().length ? `st-hit-${active()}` : undefined}
              autocomplete="off"
              spellcheck={false}
              value={query()}
              onInput={(e) => setQuery(e.currentTarget.value)}
              onKeyDown={onSearchKey}
            />
            <MicButton target={() => search} size={32} class="search-mic" label="Search settings by voice" />
            <Show when={searching()}>
              <button type="button" class="st-search-clear" aria-label="Clear search" onClick={() => { setQuery(""); search.focus(); }}>
                <Icon name="close" size={16} />
              </button>
            </Show>
          </label>
          <nav class="st-nav" role="tablist" aria-label="Settings categories" aria-orientation="vertical">
            <For each={categories()}>
              {(c, i) => {
                const on = () => !searching() && cat() === c.id;
                return (
                  <button
                    type="button"
                    id={`st-tab-${c.id}`}
                    class="st-nav-item"
                    classList={{ on: on(), dim: searching() && !counts().get(c.id) }}
                    role="tab"
                    aria-selected={on()}
                    aria-controls="st-pane"
                    tabIndex={(searching() ? i() === 0 : cat() === c.id) ? 0 : -1}
                    onClick={() => pickCategory(c.id)}
                    onKeyDown={(e) => onNavKey(e, i())}
                  >
                    <Icon name={c.icon} size={20} active={on()} />
                    <span class="st-nav-label">{c.label}</span>
                    <Show when={searching() && counts().get(c.id)}>
                      {(n) => <span class="st-count" aria-label={`${n()} matches`}>{n()}</span>}
                    </Show>
                  </button>
                );
              }}
            </For>
          </nav>
        </aside>

        <section class="st-main">
          <header class="st-head">
            <div class="st-head-text">
              <h2 id="st-title">{searching() ? "Search" : current()?.label ?? "Settings"}</h2>
              <p class="st-head-sub">
                {searching()
                  ? hits().length ? `${hits().length} setting${hits().length === 1 ? "" : "s"} match “${query().trim()}”` : `Nothing matches “${query().trim()}”`
                  : current()?.description}
              </p>
            </div>
            <button type="button" class="st-close" onClick={props.onClose} aria-label="Close settings">
              <Icon name="close" size={20} />
            </button>
          </header>

          <div class="st-pane" id="st-pane" ref={pane} role={searching() ? undefined : "tabpanel"} aria-labelledby={searching() ? undefined : `st-tab-${cat()}`}>
            <Show
              when={!searching()}
              fallback={
                <Show
                  when={hits().length}
                  fallback={<div class="st-empty">Try another word, for example “pen”, “dark”, “palm” or “বাংলা”.</div>}
                >
                  <ul class="st-results" id="st-results" role="listbox" aria-label="Matching settings">
                    <For each={hits()}>
                      {(h, i) => (
                        <li
                          id={`st-hit-${i()}`}
                          role="option"
                          aria-selected={active() === i()}
                          class="st-hit"
                          classList={{ on: active() === i() }}
                          data-hit={h.setting.id}
                          onClick={() => go(h.setting.id)}
                          onPointerMove={() => setActive(i())}
                        >
                          <span class="st-hit-icon"><Icon name={categoryById(h.setting.category)?.icon ?? "settings"} size={18} /></span>
                          <span class="st-hit-text">
                            <span class="st-hit-label">{h.setting.label}</span>
                            <span class="st-hit-path">{path(h.setting)}</span>
                            <Show when={h.setting.description}><span class="st-hit-desc">{h.setting.description}</span></Show>
                          </span>
                          <Icon name="next" size={16} />
                        </li>
                      )}
                    </For>
                  </ul>
                </Show>
              }
            >
              <For each={groups()}>
                {(g) => (
                  <section class="st-group" aria-label={g.name ?? current()?.label}>
                    <Show when={g.name}><h3 class="st-group-title">{g.name}</h3></Show>
                    <div class="st-card">
                      <For each={g.items}>{(s) => <SettingRow def={s} highlight={hl() === s.id} />}</For>
                    </div>
                  </section>
                )}
              </For>
              <Show when={canReset()}>
                <div class="st-foot">
                  <button type="button" class="st-btn ghost" classList={{ armed: armed() }} onClick={reset}>
                    <Icon name="restart" size={18} />
                    <span>{armed() ? `Click again to reset ${current()?.label ?? "this section"}` : "Reset this section"}</span>
                  </button>
                </div>
              </Show>
            </Show>
          </div>
        </section>
      </div>
    </div>
  );
}
