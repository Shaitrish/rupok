import { For, Show, createEffect, createMemo, createSignal, on, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import {
  BOARD_COLORS, camera, pageIndex, pages, prefs, replay, setPrefs, showToast, isView, modeConfig,
} from "../state";
import { brandVars, brandingFor, lessonBranding, styleOf, usesPlaceholders } from "../design";
import { openPageDesign } from "./PageDesign";
import { hasTopBar } from "../modes";
import LatencyOverlay from "./LatencyOverlay";
import { Icon } from "./Icon";
import { SelectionOverlay } from "./Selection";
import { MathPreview } from "./MathPreview";
import { ThumbCanvas } from "./PageThumb";
import { ExportHost } from "./ExportDialog";
import { FloatingAiStatus, Header } from "./Header";
import { livePhase } from "../live";
import { isPhone, pagesDrawer, setPagesDrawer } from "../viewport";

// The top bar itself lives in Header.tsx (with HeaderMenu.tsx for Insert ▾
// and ⋯, and AiStatus.tsx for the AI). Add top-bar actions there: an item on
// the bar, or an entry in the ⋯ menu (overflowEntries).
export function TopBar(props: { onObject: () => void; onSettings: () => void }) {
  const aiAround = () => livePhase() !== "idle";
  return (
    <>
      <Show when={hasTopBar(modeConfig())}>
        <Header onObject={props.onObject} onSettings={props.onSettings} />
      </Show>
      <Show when={!hasTopBar(modeConfig()) && !isView() && aiAround()}>
        <FloatingAiStatus />
      </Show>
      {/* Mounted here because the top bar component is always rendered; the
          overlay itself is a full-screen layer. */}
      <SelectionOverlay />
      <MathPreview />
      <ExportHost />
    </>
  );
}

const sameIds = (a: string[], b: string[]) => a.length === b.length && a.every((x, i) => x === b[i]);

/** Add a page that looks like the current one: right after it (what every
 * "+" does: on slide 3 it makes a new slide 4), or at the end. */
export function addPageLike(atEnd: boolean) {
  controller.board.add_page_like(atEnd);
}

/** Give every page the current page's look (colour, pattern, background image). */
export function useLookForAllPages() {
  const n = pages().length;
  controller.board.set_page_style_json(JSON.stringify(styleOf(pages()[pageIndex()])), true);
  showToast(`This look is now on all ${n} pages. Undo with Ctrl+Z.`);
}

/**
 * The pages panel. Two forms:
 * - docked: the thumbnail column at the side in Prepare (Settings →
 *   Appearance → Pages panel); the board keeps room for it;
 * - drawer: an overlay with large thumbnails that never refits the slide
 *   ("Show pages" in the dock's ⋯, the page counter, G; the only form on
 *   phones). A tap jumps and closes it unless it is pinned.
 */
export function PagesPanel() {
  const drawer = () => pagesDrawer() || isPhone();
  const editable = () => !isView();
  const current = () => pages()[pageIndex()];
  const thumbBg = (c: number) => "#" + (c >>> 8).toString(16).padStart(6, "0");
  // Keyed by page id, so thumbnails survive edits, reorders and inserts.
  const ids = createMemo(() => pages().map((p) => p.id), [], { equals: sameIds });
  let list!: HTMLDivElement;
  const close = () => setPagesDrawer(false);
  /** Pinned: the drawer stays open after a jump (not on phones, where it covers the page). */
  const pinned = () => drawer() && prefs().pagesPinned && !isPhone();
  const jump = (i: number) => {
    controller.board.goto_page(i);
    if (drawer() && !pinned()) close();
  };

  // ------------------------------------------------- drag to reorder
  // Mouse: press and move. Pen and touch: press and hold, then move (a
  // plain swipe keeps scrolling the list); hold and lift without moving
  // opens the page menu.
  const [drag, setDrag] = createSignal<{ from: number; gap: number; dy: number; dx: number } | null>(null);
  let suppressClick = false;

  const thumbEls = () => list.querySelectorAll<HTMLElement>(".page-thumb");
  /** The gap (0..n) a point falls in: before the thumbnail it is over, by reading order. */
  const gapAt = (clientX: number, clientY: number) => {
    const thumbs = thumbEls();
    for (let k = 0; k < thumbs.length; k++) {
      const r = thumbs[k].getBoundingClientRect();
      if (clientY < r.top - 4) return k;
      if (clientY <= r.bottom + 4 && clientX < r.left + r.width / 2) return k;
    }
    return thumbs.length;
  };

  const startDrag = (e: PointerEvent, from: number) => {
    if (!editable() || e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
    const el = e.currentTarget as HTMLElement;
    const pid = e.pointerId;
    const [x0, y0, scroll0] = [e.clientX, e.clientY, list.scrollTop];
    const mouse = e.pointerType === "mouse";
    let active = false;
    let moved = false;
    const begin = () => {
      active = true;
      setDrag({ from, gap: from, dy: 0, dx: 0 });
      try { el.setPointerCapture(pid); } catch { /* pointer already gone */ }
    };
    const timer = mouse ? 0 : window.setTimeout(begin, 320);
    const move = (ev: PointerEvent) => {
      if (ev.pointerId !== pid) return;
      const d = Math.hypot(ev.clientX - x0, ev.clientY - y0);
      if (!active) {
        if (mouse && d > 5) begin();
        else if (!mouse && d > 8) return end(false);
        if (!active) return;
      }
      if (d > 6) moved = true;
      ev.preventDefault();
      const r = list.getBoundingClientRect();
      if (ev.clientY < r.top + 32) list.scrollTop -= 12;
      else if (ev.clientY > r.bottom - 32) list.scrollTop += 12;
      setDrag({ from, gap: gapAt(ev.clientX, ev.clientY), dy: ev.clientY - y0 + list.scrollTop - scroll0, dx: drawer() ? ev.clientX - x0 : 0 });
    };
    const end = (commit: boolean) => {
      clearTimeout(timer);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", cancel);
      const d = drag();
      setDrag(null);
      if (!active) return;
      suppressClick = true;
      setTimeout(() => (suppressClick = false), 0);
      // Touch or pen: held without moving is a long-press. It starts
      // selecting pages (and opens the page menu); while selecting, it marks.
      if (!mouse && !moved && commit) {
        if (selectMode()) return toggleSel(from);
        toggleSel(from);
        const r = el.getBoundingClientRect();
        openMenu(from, r.left + r.width / 2, r.top + r.height / 2);
        return;
      }
      if (commit && d) {
        const id = pages()[d.from]?.id;
        // Dragging a marked page moves every marked page.
        if (id && isSel(id) && selected().length > 1) {
          controller.board.move_pages(Uint32Array.from(selIndexes()), d.gap);
          return;
        }
        const to = d.gap > d.from ? d.gap - 1 : d.gap;
        if (to !== d.from) controller.board.move_page(d.from, to);
      }
    };
    const up = (ev: PointerEvent) => ev.pointerId === pid && end(true);
    const cancel = (ev: PointerEvent) => ev.pointerId === pid && end(false);
    window.addEventListener("pointermove", move, { passive: false });
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
  };

  /** Where the dragged page would land: a bar in the gap between thumbnails. */
  const dropLine = createMemo(() => {
    const d = drag();
    if (!d || d.gap === d.from || d.gap === d.from + 1) return null;
    const thumbs = thumbEls();
    if (!thumbs.length) return null;
    const lr = list.getBoundingClientRect();
    const at = (el: HTMLElement, after: boolean) => {
      const r = el.getBoundingClientRect();
      const top = r.top - lr.top + list.scrollTop;
      // Docked: a line across the column. Drawer (a grid): a bar beside the thumbnail.
      if (!drawer()) return { top: after ? top + r.height + 3 : top - 5, left: 2, width: lr.width - 4, height: 2 };
      return { top, left: (after ? r.right + 3 : r.left - 5) - lr.left, width: 2, height: r.height };
    };
    return d.gap < thumbs.length ? at(thumbs[d.gap], false) : at(thumbs[thumbs.length - 1], true);
  });

  // ------------------------------------------------ several pages
  // Long-press (touch), Ctrl/⌘-click or Shift-click enters selection mode;
  // then a tap marks or unmarks a page. Delete, duplicate, or drag to move
  // them all: each one undo step.
  const [selectMode, setSelectMode] = createSignal(false);
  const [selected, setSelected] = createSignal<string[]>([]);
  let anchor = -1;
  const isSel = (id: string) => selected().includes(id);
  const selIndexes = () => pages().map((p, i) => (isSel(p.id) ? i : -1)).filter((i) => i >= 0);
  const toggleSel = (i: number) => {
    const id = pages()[i]?.id;
    if (!id) return;
    setSelectMode(true);
    setSelected((s) => (s.includes(id) ? s.filter((x) => x !== id) : [...s, id]));
    anchor = i;
  };
  const selectRange = (i: number) => {
    const from = anchor >= 0 ? anchor : pageIndex();
    const [a, b] = from <= i ? [from, i] : [i, from];
    setSelectMode(true);
    setSelected(pages().slice(a, b + 1).map((p) => p.id));
  };
  const endSelect = () => {
    setSelectMode(false);
    setSelected([]);
    setArmDeleteMany(false);
    anchor = -1;
  };
  // Pages that are gone (undo, another device) drop out of the selection.
  createEffect(on(pages, (ps) => setSelected((s) => s.filter((id) => ps.some((p) => p.id === id))), { defer: true }));
  const [armDeleteMany, setArmDeleteMany] = createSignal(false);
  const deleteSelected = () => {
    const idx = selIndexes();
    if (!idx.length) return;
    if (idx.length >= pages().length) return showToast("A lesson keeps at least one page");
    if (!armDeleteMany()) return setArmDeleteMany(true);
    const n = controller.board.delete_pages(Uint32Array.from(idx));
    endSelect();
    showToast(`Deleted ${n} page${n === 1 ? "" : "s"}. Undo with Ctrl+Z.`);
  };
  const duplicateSelected = () => {
    const idx = selIndexes();
    if (!idx.length) return;
    controller.board.duplicate_pages(Uint32Array.from(idx));
    showToast(`Duplicated ${idx.length} page${idx.length === 1 ? "" : "s"}`);
    endSelect();
  };
  onMount(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape" && selectMode()) { e.stopPropagation(); endSelect(); }
    };
    window.addEventListener("keydown", key, true);
    onCleanup(() => window.removeEventListener("keydown", key, true));
  });

  // ------------------------------------------------------- page menu
  const [menu, setMenu] = createSignal<{ index: number; x: number; y: number } | null>(null);
  const [armDelete, setArmDelete] = createSignal(false);
  const openMenu = (index: number, x: number, y: number) => {
    if (!editable()) return;
    setArmDelete(false);
    setMenu({ index, x, y });
  };
  const menuAction = (fn: (i: number) => void) => () => {
    const m = menu();
    setMenu(null);
    if (m) fn(m.index);
  };

  // The list element is re-created when a mode hides and shows the panel.
  const attachList = (el: HTMLDivElement) => {
    list = el;
    // Once a touch drag has begun, stop the list from scrolling under it.
    el.addEventListener("touchmove", (e) => drag() && e.preventDefault(), { passive: false });
    el.addEventListener("contextmenu", (e) => e.preventDefault());
  };

  // Keep the current page in view: on opening (centred) and as it changes.
  const reveal = (block: ScrollLogicalPosition) =>
    requestAnimationFrame(() => list?.querySelector<HTMLElement>(".page-thumb.on")?.scrollIntoView({ block, inline: "nearest" }));
  onMount(() => reveal("center"));
  createEffect(on(pageIndex, () => reveal("nearest"), { defer: true }));

  const [goTo, setGoTo] = createSignal("");

  return (
    <Show when={drawer() || modeConfig().chrome.pages}>
    {/* The drawer floats over the board; tapping outside closes it (unless pinned). */}
    <Show when={drawer() && !pinned()}>
      <div class="pages-scrim" onClick={close} />
    </Show>
    <aside
      class="pages surface"
      classList={{ viewing: isView(), top: !hasTopBar(modeConfig()), drawer: drawer(), pinned: pinned() }}
      aria-label="Pages"
      role={drawer() ? "dialog" : undefined}
    >
      <Show when={selectMode()}>
        <div class="pages-head pages-select" role="toolbar" aria-label="Selected pages">
          <span class="sel-count">{selected().length} selected</span>
          <span class="grow" />
          <button class="btn icon" title="Duplicate the selected pages" aria-label="Duplicate selected pages" disabled={!selected().length} onClick={duplicateSelected}>
            <Icon name="duplicate" size={20} />
          </button>
          <button
            class="btn icon thumb-del"
            classList={{ armed: armDeleteMany() }}
            title={armDeleteMany() ? "Tap again to delete" : "Delete the selected pages"}
            aria-label={armDeleteMany() ? `Confirm: delete ${selected().length} pages` : "Delete selected pages"}
            disabled={!selected().length}
            onClick={deleteSelected}
          >
            <Icon name="trash" size={20} />
          </button>
          <button class="btn done" title="Done  ·  Esc" onClick={endSelect}>Done</button>
        </div>
      </Show>
      <div class="pages-head" classList={{ hidden: selectMode() }}>
        <span>Pages · {pages().length}</span>
        <Show when={drawer() && pages().length > 9}>
          <form
            class="page-goto"
            onSubmit={(e) => {
              e.preventDefault();
              const n = Math.round(Number(goTo()));
              if (n >= 1 && n <= pages().length) jump(n - 1);
              setGoTo("");
            }}
          >
            <input
              class="input"
              inputmode="numeric"
              enterkeyhint="go"
              placeholder="Go to"
              aria-label={`Go to page (1 to ${pages().length})`}
              value={goTo()}
              onInput={(e) => setGoTo(e.currentTarget.value.replace(/\D/g, "").slice(0, 4))}
              onKeyDown={(e) => e.key === "Escape" && close()}
            />
          </form>
        </Show>
        <span class="grow" />
        <Show when={editable()}>
          <button class="btn icon" title="Add page after this one (same look)" aria-label="Add page" onClick={() => addPageLike(false)}>
            <Icon name="plus" size={20} />
          </button>
        </Show>
        <Show when={drawer() && !isPhone()}>
          <button
            class="btn icon"
            classList={{ on: prefs().pagesPinned }}
            title={prefs().pagesPinned ? "Unpin: close after picking a page" : "Pin: keep open after picking a page"}
            aria-label="Keep the pages open"
            aria-pressed={prefs().pagesPinned}
            onClick={() => setPrefs({ pagesPinned: !prefs().pagesPinned })}
          >
            <Icon name="pin" size={20} active={prefs().pagesPinned} />
          </button>
        </Show>
        <Show when={drawer()}>
          <button class="btn icon" title="Close  ·  Esc" aria-label="Close pages" onClick={close}>
            <Icon name="close" size={20} />
          </button>
        </Show>
      </div>
      <div class="pages-list" classList={{ sorting: !!drag() }} ref={attachList}>
        <For each={ids()}>
          {(id, i) => {
            const p = () => pages()[i()];
            const lifted = () => drag()?.from === i();
            const design = () => {
              const b = brandingFor(p(), lessonBranding());
              // A line with placeholders changes with the title, page count and names.
              return JSON.stringify([p()?.background, b, usesPlaceholders(b) ? [pages().length, brandVars()] : 0]);
            };
            return (
              <div
                class="page-thumb"
                classList={{ on: i() === pageIndex(), lifted: lifted(), marked: isSel(id), selecting: selectMode() }}
                data-page={id}
                role="button"
                tabindex="0"
                aria-label={`Page ${i() + 1}${p()?.title ? `: ${p()!.title}` : ""}`}
                aria-current={i() === pageIndex() ? "page" : undefined}
                aria-pressed={selectMode() ? isSel(id) : undefined}
                style={{
                  background: thumbBg(p()?.bg_color ?? 0xffffffff),
                  transform: lifted() ? `translate(${drag()!.dx}px, ${drag()!.dy}px)` : undefined,
                }}
                onClick={(e) => {
                  if (suppressClick) return;
                  if (editable() && (e.ctrlKey || e.metaKey)) return toggleSel(i());
                  if (editable() && e.shiftKey) return selectRange(i());
                  if (selectMode()) return toggleSel(i());
                  anchor = i();
                  jump(i());
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") { e.preventDefault(); e.stopPropagation(); jump(i()); }
                  else if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
                    e.preventDefault();
                    const r = e.currentTarget.getBoundingClientRect();
                    openMenu(i(), r.left + r.width / 2, r.top + r.height / 2);
                  }
                }}
                onPointerDown={(e) => startDrag(e, i())}
                onContextMenu={(e) => {
                  // Mouse right-click; a long touch opens the menu on release instead.
                  if ((e as PointerEvent).pointerType && (e as PointerEvent).pointerType !== "mouse") return;
                  e.preventDefault();
                  openMenu(i(), e.clientX, e.clientY);
                }}
                title={p()?.title || `Page ${i() + 1}`}
              >
                <ThumbCanvas index={i()} current={i() === pageIndex()} itemCount={p()?.item_count ?? 0} design={design()} />
                <span class="num">{i() + 1}</span>
                <Show when={selectMode()}>
                  <span class="thumb-check" classList={{ on: isSel(id) }} aria-hidden="true" />
                </Show>
                <Show when={editable() && !selectMode()}>
                  <button
                    class="gap-add"
                    title={`Insert a page after page ${i() + 1}`}
                    aria-label={`Insert page after page ${i() + 1}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      controller.board.insert_page_like(i(), false);
                    }}
                  >
                    <Icon name="plus" size={16} />
                  </button>
                  <div class="thumb-actions">
                    <button
                      class="btn icon"
                      title="Duplicate page"
                      aria-label="Duplicate page"
                      onClick={(e) => {
                        e.stopPropagation();
                        controller.board.duplicate_page(i());
                      }}
                    >
                      <Icon name="duplicate" size={16} />
                    </button>
                    <Show when={pages().length > 1}>
                      <button
                        class="btn icon thumb-del"
                        title="Delete page (click twice)"
                        aria-label="Delete page"
                        onClick={(e) => {
                          e.stopPropagation();
                          controller.board.goto_page(i());
                          if (confirmTwice()) controller.board.delete_page();
                        }}
                      >
                        <Icon name="trash" size={16} />
                      </button>
                    </Show>
                    <button
                      class="btn icon"
                      title="More page actions (or right-click, or press and hold)"
                      aria-label="Page actions"
                      aria-haspopup="menu"
                      onClick={(e) => {
                        e.stopPropagation();
                        const r = e.currentTarget.getBoundingClientRect();
                        openMenu(i(), r.left, r.bottom + 4);
                      }}
                    >
                      <Icon name="more" size={16} />
                    </button>
                  </div>
                </Show>
              </div>
            );
          }}
        </For>
        {/* The end of the list: a teacher who scrolled down adds a page here. */}
        <Show when={editable()}>
          <button class="page-add-tile" title={`New page after page ${pageIndex() + 1} (same look)`} aria-label="New page after this one" onClick={() => addPageLike(false)}>
            <Icon name="plus" size={20} />
            <span>New page <small>after {pageIndex() + 1}</small></span>
          </button>
        </Show>
        <Show when={dropLine()}>
          {(l) => <div class="drop-line" style={{ top: `${l().top}px`, left: `${l().left}px`, width: `${l().width}px`, height: `${l().height}px` }} />}
        </Show>
      </div>
      <Show when={editable()}>
        <div class="page-style-wrap">
          <div class="page-style" role="group" aria-label="Page pattern">
            <For each={["plain", "grid", "dots", "lines"] as const}>
              {(pat) => (
                <button
                  classList={{ on: current()?.pattern === pat }}
                  title={pat[0].toUpperCase() + pat.slice(1)}
                  aria-label={`${pat[0].toUpperCase() + pat.slice(1)} pattern`}
                  aria-pressed={current()?.pattern === pat}
                  onClick={() => controller.board.set_background(pat, current()?.bg_color ?? 0xffffffff)}
                >
                  <span class={`pat-ico ${pat}`} />
                </button>
              )}
            </For>
          </div>
          <div class="page-style" role="group" aria-label="Board colour">
            <For each={BOARD_COLORS}>
              {(b) => (
                <button
                  classList={{ on: (current()?.bg_color ?? 0xffffffff) === b.rgba }}
                  title={b.label}
                  aria-label={`${b.label} board`}
                  aria-pressed={(current()?.bg_color ?? 0xffffffff) === b.rgba}
                  onClick={() => controller.setBoardColor(b.rgba)}
                >
                  <span class="bg-dot" style={{ background: thumbBg(b.rgba) }} />
                </button>
              )}
            </For>
          </div>
          <div class="page-style-actions">
            <button class="btn" title="Give every page this page's colour, pattern and background" onClick={useLookForAllPages}>
              <Icon name="layers" size={16} /><span>All pages</span>
            </button>
            <button class="btn" title="Page design: background image, logo, school name, looks" aria-label="Page design" onClick={() => openPageDesign()}>
              <Icon name="palette" size={16} /><span>Design…</span>
            </button>
          </div>
        </div>
      </Show>
    </aside>
    <Show when={menu()}>
      {(m) => (
        <PageMenu
          x={m().x}
          y={m().y}
          index={m().index}
          count={pages().length}
          armDelete={armDelete()}
          onClose={() => setMenu(null)}
          onInsertBefore={menuAction((i) => { endSelect(); controller.board.insert_page_like(i, true); })}
          onInsertAfter={menuAction((i) => { endSelect(); controller.board.insert_page_like(i, false); })}
          onSelect={menuAction((i) => { if (!isSel(pages()[i]?.id ?? "")) toggleSel(i); })}
          selecting={selectMode()}
          onAddAtEnd={menuAction((i) => { endSelect(); controller.board.goto_page(i); addPageLike(true); })}
          onDuplicate={menuAction((i) => { endSelect(); controller.board.duplicate_page(i); })}
          onMove={(d) => menuAction((i) => { endSelect(); controller.board.move_page(i, i + d); })()}
          onDelete={() => {
            if (!armDelete()) return setArmDelete(true);
            menuAction((i) => { endSelect(); controller.board.goto_page(i); controller.board.delete_page(); })();
          }}
        />
      )}
    </Show>
    </Show>
  );
}

/** The page menu (right-click, long-press or ⋯ on a thumbnail). */
function PageMenu(props: {
  x: number; y: number; index: number; count: number; armDelete: boolean; selecting: boolean;
  onClose: () => void; onInsertBefore: () => void; onInsertAfter: () => void; onAddAtEnd: () => void; onDuplicate: () => void;
  onSelect: () => void;
  onMove: (d: -1 | 1) => void; onDelete: () => void;
}) {
  let el!: HTMLDivElement;
  const [pos, setPos] = createSignal({ left: props.x, top: props.y });
  onMount(() => {
    // Keep it on screen.
    const r = el.getBoundingClientRect();
    setPos({
      left: Math.max(8, Math.min(props.x, window.innerWidth - r.width - 8)),
      top: Math.max(8, Math.min(props.y, window.innerHeight - r.height - 8)),
    });
    el.querySelector<HTMLButtonElement>("button")?.focus();
    const away = (e: PointerEvent) => !el.contains(e.target as Node) && props.onClose();
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") { e.stopPropagation(); props.onClose(); }
    };
    setTimeout(() => window.addEventListener("pointerdown", away, true), 0);
    window.addEventListener("keydown", key, true);
    onCleanup(() => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", key, true);
    });
  });
  return (
    <div class="page-menu dock-menu" role="menu" aria-label={`Page ${props.index + 1}`} ref={el} style={{ left: `${pos().left}px`, top: `${pos().top}px` }}>
      <button class="dock-row" role="menuitem" onClick={props.onInsertBefore}><Icon name="plus" size={20} /><span class="grow">Insert page before</span></button>
      <button class="dock-row" role="menuitem" onClick={props.onInsertAfter}><Icon name="plus" size={20} /><span class="grow">Insert page after</span></button>
      <button class="dock-row" role="menuitem" onClick={props.onAddAtEnd}><Icon name="plus" size={20} /><span class="grow">New page at the end</span></button>
      <button class="dock-row" role="menuitem" onClick={props.onDuplicate}><Icon name="duplicate" size={20} /><span class="grow">Duplicate</span></button>
      <button class="dock-row" role="menuitem" onClick={props.onSelect}><Icon name="select" size={20} /><span class="grow">{props.selecting ? "Selecting: tap more pages" : "Select several pages"}</span></button>
      <div class="dock-hr" />
      <button class="dock-row" role="menuitem" disabled={props.index === 0} onClick={() => props.onMove(-1)}><Icon name="up" size={20} /><span class="grow">Move up</span></button>
      <button class="dock-row" role="menuitem" disabled={props.index >= props.count - 1} onClick={() => props.onMove(1)}><Icon name="down" size={20} /><span class="grow">Move down</span></button>
      <Show when={props.count > 1}>
        <div class="dock-hr" />
        <button class="dock-row danger" classList={{ armed: props.armDelete }} role="menuitem" onClick={props.onDelete}>
          <Icon name="trash" size={20} /><span class="grow">{props.armDelete ? "Tap again to delete" : "Delete page"}</span>
        </button>
      </Show>
    </div>
  );
}

let lastDelete = 0;
function confirmTwice(): boolean {
  const now = Date.now();
  if (now - lastDelete < 2000) return true;
  lastDelete = now;
  showToast("Click delete again to remove this page");
  return false;
}

export function ReplayBar() {
  const [speed, setSpeed] = createSignal(2);
  const fmt = (ms: number) => {
    const s = Math.floor(ms / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  };
  return (
    <Show when={replay()}>
      {(r) => (
        <div class="replaybar surface" role="group" aria-label="Replay">
          <button class="btn icon soft" onClick={() => controller.board.replay_play(!r().playing, speed())} aria-label={r().playing ? "Pause" : "Play"}>
            <Icon name={r().playing ? "pause" : "play"} active />
          </button>
          <input
            class="range"
            type="range"
            min="0"
            max={r().d}
            step="10"
            value={r().t}
            style={{ "--p": `${r().d ? (r().t / r().d) * 100 : 0}%` }}
            onInput={(e) => controller.board.replay_seek(Number(e.currentTarget.value))}
            aria-label="Replay position"
          />
          <span class="time">{fmt(r().t)} / {fmt(r().d)}</span>
          <div class="seg">
            <For each={[1, 2, 4, 8]}>
              {(s) => (
                <button classList={{ on: speed() === s }} onClick={() => { setSpeed(s); controller.board.replay_play(r().playing, s); }}>
                  {s}×
                </button>
              )}
            </For>
          </div>
          <button class="btn icon" title="Close replay  ·  Esc" aria-label="Close replay" onClick={() => controller.board.replay_stop()}>
            <Icon name="close" />
          </button>
        </div>
      )}
    </Show>
  );
}

export function StatusChips() {
  const c = () => modeConfig().chrome;
  return (
    <div class="statusbar">
      <Show when={c().zoom}>
      <div class="zoom surface">
        <button class="btn icon" onClick={() => controller.zoomBy(0.8)} title="Zoom out  ·  −" aria-label="Zoom out"><Icon name="zoomOut" size={20} /></button>
        <button class="btn pct" onClick={() => controller.board.fit_page()} title="Fit page  ·  0">{Math.round(camera()[2] * 100)}%</button>
        <button class="btn icon" onClick={() => controller.zoomBy(1.25)} title="Zoom in  ·  +" aria-label="Zoom in"><Icon name="zoomIn" size={20} /></button>
      </div>
      </Show>
      {/* Performance and pen latency overlay (Settings → About → Troubleshooting). */}
      <Show when={c().stats && prefs().showStats}>
        <LatencyOverlay />
      </Show>
    </div>
  );
}

export function downloadQuiz() {
  const rows = controller.objects.quizLog.map((q) => ({ time: new Date(q.at).toISOString(), object: q.title, ...(q.payload as object) }));
  const blob = new Blob([JSON.stringify(rows, null, 2)], { type: "application/json" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = "quiz-answers.json";
  a.click();
}
