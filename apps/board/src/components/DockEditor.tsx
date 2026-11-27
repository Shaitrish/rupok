// "Customize dock": the teacher arranges the Smart Dock by hand. Buttons
// move between the tray of available buttons and the dock's groups (drag,
// or pick up with Enter and move with the arrow keys), groups get a
// separator between them, a second dock can sit on another edge, and every
// button (or the whole dock) can be bigger. Changes apply at once: the real
// dock behind the sheet is the live preview. Named layouts (built-in and
// the teacher's own) switch everything in one tap.
//
// The model is modes.ts → DockConfig (groups, split, size, sizes, large);
// agents change the same thing with set_dock_layout (agentControl.ts).

import { For, Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import {
  ALLOWED_TOOLS, CORE_TOOLS, DEFAULT_MODES, DOCK_PRESETS, SHAPE_TOOLS, SHAPES_PIN, TOOLBAR_ITEMS, type DockConfig, type DockGroup, type DockPin,
  type DockPosition, type DockPreset, type DockSize, type Mode, type ToolbarItem, applyDockPreset, dockWarnings, layoutPins, sanitizeMode,
  CLASSIC_ITEMS,
} from "../modes";
import { dockEditor, resetDock, setDockEditor } from "../dock";
import { mode, modeConfig, prefs, setPrefs, showToast } from "../state";
import { dockLayout } from "./Dock";
import { type Layout, layout } from "../layout";
import { Icon } from "./Icon";
import type { IconName } from "../icons.generated";
import "./DockEditor.css";

type EditMode = Exclude<Mode, "view">;
const META = new Map<string, { icon: IconName; label: string }>(TOOLBAR_ITEMS.map((t) => [t.id, { icon: t.icon, label: t.label }]));
META.set("shapes", { icon: SHAPES_PIN.icon, label: SHAPES_PIN.label });
const labelOf = (p: DockPin) => META.get(p)?.label ?? p;
const iconOf = (p: DockPin) => META.get(p)?.icon ?? "pen";
const isCore = (p: DockPin) => CORE_TOOLS.includes(p as ToolbarItem);

/** Where a button sits: `main` or `split` dock, group index, position. */
interface Slot { dock: "main" | "split"; g: number; i: number }

/** Everything a mode's dock can hold (shape tools fold into one Shapes button). */
export function placeable(m: Mode): DockPin[] {
  const out: DockPin[] = [];
  for (const t of ALLOWED_TOOLS[m]) {
    const pin: DockPin = SHAPE_TOOLS.includes(t) ? "shapes" : t;
    if (!out.includes(pin)) out.push(pin);
  }
  return out;
}

/** The dock as groups: the teacher's, or the automatic grouping turned into groups. */
export function groupsOf(m: EditMode): { main: DockGroup[]; split: DockGroup[] } {
  const c = modeConfig(m);
  if (c.dock.groups?.length || c.dock.split) return { main: structuredClone(c.dock.groups ?? []), split: structuredClone(c.dock.split?.groups ?? []) };
  const bar = dockLayout(c, true, false).bar.filter((e) => e.kind !== "colors" && e.kind !== "context" && e.kind !== "more");
  const main: DockGroup[] = [];
  for (const e of bar) {
    const pin: DockPin = e.kind === "shapes" ? "shapes" : (e.id as DockPin);
    const last = main[main.length - 1];
    if (last && last.id === e.group) last.items.push(pin);
    else main.push({ id: e.group, items: [pin] });
  }
  return { main: main.map((g, i) => ({ ...g, id: `g${i + 1}` })), split: [] };
}

/** Save a mode's dock (validated like an imported config). */
export function saveDock(m: EditMode, patch: Partial<DockConfig>) {
  const cur = modeConfig(m);
  const next = sanitizeMode(m, { chrome: cur.chrome, tools: cur.tools, dock: { ...cur.dock, ...patch } });
  setPrefs({ modes: { ...prefs().modes, [m]: next } });
  resetDock(m);
  controller.updateInsets(mode() === "teach");
}

/** The teacher's saved layouts (prefs → dockPresets). */
export const savedPresets = (): DockPreset[] => prefs().dockPresets ?? [];

export function applyPreset(p: DockPreset) {
  const toIslands = Object.values(p.modes).some((d) => d?.style === "islands");
  // Going Classic: keep the Modern docks, so the Layout switch brings them back.
  const keep = toIslands && layout() === "modern"
    ? { modernDocks: { prepare: structuredClone(modeConfig("prepare").dock), teach: structuredClone(modeConfig("teach").dock) } }
    : {};
  setPrefs({ modes: applyDockPreset(prefs().modes, p), ...(p.hand ? { hand: p.hand } : {}), ...keep });
  resetDock("prepare");
  resetDock("teach");
  controller.updateInsets(mode() === "teach");
}

/** Switch between the Modern and the Classic layout (layout.ts). Classic
 * applies the "Classic" preset; Modern brings back the teacher's own
 * Modern docks (else the default dock). One prefs change, undoable. */
export function setLayout(to: Layout) {
  setPrefs({ layoutChosen: true });
  if (to === layout() && to === "modern") return;
  if (to === "classic") return applyPreset(DOCK_PRESETS.find((p) => p.name === "Classic")!);
  const kept = prefs().modernDocks;
  const modes = { ...prefs().modes };
  for (const m of ["prepare", "teach"] as const) {
    const cur = modeConfig(m);
    const dock = kept?.[m] ?? DEFAULT_MODES[m].dock;
    // The islands' own buttons leave with them, unless the Modern dock has them too.
    const pins = new Set<string>([...dock.pinned, ...layoutPins(dock)]);
    const tools = cur.tools.filter((t) => !CLASSIC_ITEMS.includes(t) || pins.has(t));
    modes[m] = sanitizeMode(m, { chrome: cur.chrome, tools, dock: { ...dock, style: "dock" } });
  }
  setPrefs({ modes });
  resetDock("prepare");
  resetDock("teach");
  controller.updateInsets(true);
}

/** Save the current docks as a named layout (replacing one of that name). */
export function savePreset(name: string, modes: EditMode[] = ["prepare", "teach"]): DockPreset {
  const clean = name.trim().slice(0, 32) || "My layout";
  const p: DockPreset = { name: clean, hint: "Saved by you", modes: {} };
  for (const m of modes) p.modes[m] = structuredClone(modeConfig(m).dock);
  const rest = savedPresets().filter((x) => x.name.toLowerCase() !== clean.toLowerCase());
  setPrefs({ dockPresets: [...rest, p].slice(-12) });
  return p;
}

const POSITIONS: { v: DockPosition; label: string }[] = [
  { v: "auto", label: "Auto" }, { v: "left", label: "Left" }, { v: "right", label: "Right" }, { v: "top", label: "Top" }, { v: "bottom", label: "Bottom" },
];
const SPLIT_POS: { v: DockPosition; label: string }[] = [
  { v: "left", label: "Left" }, { v: "right", label: "Right" }, { v: "top", label: "Top" }, { v: "bottom", label: "Bottom" },
];
const SIZES: { v: DockSize; label: string }[] = [{ v: "s", label: "Small" }, { v: "m", label: "Medium" }, { v: "l", label: "Large" }];

export function DockEditor() {
  return (
    <Show when={dockEditor()}>
      {(m) => <Editor mode={m() as EditMode} />}
    </Show>
  );
}

function Editor(props: { mode: EditMode }) {
  const [m, setM] = createSignal<EditMode>(props.mode);
  const cfg = () => modeConfig(m());
  const isles = () => cfg().dock.style === "islands";
  const groups = createMemo(() => groupsOf(m()));
  const [picked, setPicked] = createSignal<Slot | null>(null);
  const [drag, setDrag] = createSignal<{ pin: DockPin; x: number; y: number; from: Slot | null } | null>(null);
  const [presetName, setPresetName] = createSignal("");
  let sheet!: HTMLDivElement;

  const close = () => setDockEditor(null);
  onMount(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !picked()) {
        e.preventDefault();
        close();
      }
    };
    window.addEventListener("keydown", key);
    onCleanup(() => window.removeEventListener("keydown", key));
    queueMicrotask(() => sheet?.querySelector<HTMLElement>("[data-autofocus]")?.focus());
  });

  const write = (main: DockGroup[], split: DockGroup[]) => {
    const s = cfg().dock.split;
    saveDock(m(), {
      groups: main.filter((g) => g.items.length || main.length === 1),
      split: split.some((g) => g.items.length) ? { position: s?.position ?? "right", orientation: s?.orientation ?? "auto", groups: split.filter((g) => g.items.length) } : null,
    });
  };
  const listOf = (d: "main" | "split", gs = groups()) => (d === "main" ? gs.main : gs.split);
  const tray = () => {
    const on = new Set<string>(layoutPins({ ...cfg().dock, groups: groups().main, split: { position: "right", orientation: "auto", groups: groups().split } }));
    return placeable(m()).filter((p) => !on.has(p));
  };

  /** Move `pin` (from `from`, or the tray) to `to` (or the tray when null). */
  const place = (pin: DockPin, from: Slot | null, to: Slot | null) => {
    const gs = structuredClone(groups());
    if (from) listOf(from.dock, gs)[from.g]?.items.splice(from.i, 1);
    if (!to) {
      if (isCore(pin)) {
        showToast(`${labelOf(pin)} always stays on a dock (pen, highlighter, eraser, laser and undo)`);
        return;
      }
    } else {
      const list = listOf(to.dock, gs);
      while (list.length <= to.g) list.push({ id: `${to.dock === "main" ? "g" : "s"}${Date.now().toString(36)}${list.length}`, items: [] });
      const items = list[to.g].items;
      items.splice(Math.min(Math.max(0, to.i), items.length), 0, pin);
    }
    write(gs.main, gs.split);
  };
  const addGroup = (d: "main" | "split") => {
    const gs = structuredClone(groups());
    listOf(d, gs).push({ id: `${d === "main" ? "g" : "s"}${Date.now().toString(36)}`, items: [] });
    // An empty group is kept only until it gets a button: show it now.
    setEmpty({ dock: d, g: listOf(d, gs).length - 1 });
  };
  const [empty, setEmpty] = createSignal<{ dock: "main" | "split"; g: number } | null>(null);
  const groupsFor = (d: "main" | "split") => {
    const list = listOf(d);
    const e = empty();
    return e && e.dock === d && e.g >= list.length ? [...list, { id: "new", items: [] as DockPin[] }] : list.length ? list : [{ id: "new", items: [] as DockPin[] }];
  };

  // ------------------------------------------------- keyboard pick-up
  const onChipKey = (e: KeyboardEvent, pin: DockPin, at: Slot) => {
    const p = picked();
    const same = p && p.dock === at.dock && p.g === at.g && p.i === at.i;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      setPicked(same ? null : at);
      return;
    }
    if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      setPicked(null);
      place(pin, at, null);
      return;
    }
    if (!same) return;
    const list = groupsFor(at.dock);
    let to: Slot | null = null;
    if (e.key === "ArrowLeft" || e.key === "ArrowUp" && e.shiftKey) to = { ...at, i: at.i - 1 };
    else if (e.key === "ArrowRight") to = { ...at, i: at.i + 1 };
    else if (e.key === "ArrowUp") to = at.g > 0 ? { dock: at.dock, g: at.g - 1, i: list[at.g - 1].items.length } : at.dock === "split" ? { dock: "main", g: Math.max(0, groups().main.length - 1), i: 99 } : null;
    else if (e.key === "ArrowDown") to = at.g < list.length - 1 ? { dock: at.dock, g: at.g + 1, i: 0 } : at.dock === "main" && cfg().dock.split ? { dock: "split", g: 0, i: 0 } : null;
    else if (e.key === "Escape") {
      e.preventDefault();
      setPicked(null);
      return;
    }
    if (!to) return;
    e.preventDefault();
    if (to.i < 0) {
      if (to.g === 0) return;
      to = { dock: to.dock, g: to.g - 1, i: list[to.g - 1].items.length };
    }
    if (to.i > list[to.g].items.length - (to.g === at.g ? 1 : 0)) {
      if (to.g >= list.length - 1) return;
      to = { dock: to.dock, g: to.g + 1, i: 0 };
    }
    place(pin, at, to);
    const nt = to;
    setPicked(nt);
    queueMicrotask(() => sheet.querySelector<HTMLElement>(`[data-slot="${nt.dock}-${nt.g}-${nt.i}"]`)?.focus());
  };

  // ------------------------------------------------------ pointer drag
  const startDrag = (e: PointerEvent, pin: DockPin, from: Slot | null) => {
    if (e.button !== 0) return;
    const [x0, y0] = [e.clientX, e.clientY];
    let moving = false;
    const move = (ev: PointerEvent) => {
      if (!moving && Math.hypot(ev.clientX - x0, ev.clientY - y0) < 6) return;
      moving = true;
      ev.preventDefault();
      setDrag({ pin, x: ev.clientX, y: ev.clientY, from });
      // Near the top or bottom of the sheet, scroll it so every group is reachable.
      const body = sheet.querySelector<HTMLElement>(".de-body");
      if (body) {
        const r = body.getBoundingClientRect();
        if (ev.clientY < r.top + 48) body.scrollTop -= 16;
        else if (ev.clientY > r.bottom - 48) body.scrollTop += 16;
      }
    };
    const up = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      if (!moving) return;
      setDrag(null);
      const target = dropTarget(ev.clientX, ev.clientY);
      if (target === "tray") place(pin, from, null);
      else if (target) {
        // Moving later in the same group: the gap left behind shifts it.
        const adj = from && from.dock === target.dock && from.g === target.g && from.i < target.i ? target.i - 1 : target.i;
        place(pin, from, { ...target, i: adj });
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
  };
  const dropTarget = (x: number, y: number): Slot | "tray" | null => {
    const el = document.elementFromPoint(x, y) as HTMLElement | null;
    if (el?.closest("[data-drop-tray]")) return "tray";
    const zone = el?.closest<HTMLElement>("[data-drop-group]");
    if (!zone) return null;
    const [dock, g] = zone.dataset.dropGroup!.split("-");
    const chips = [...zone.querySelectorAll<HTMLElement>("[data-chip]")];
    let i = chips.length;
    for (const [k, c] of chips.entries()) {
      const r = c.getBoundingClientRect();
      if (x < r.left + r.width / 2 && y < r.bottom) {
        i = k;
        break;
      }
    }
    return { dock: dock as "main" | "split", g: Number(g), i };
  };

  // ---------------------------------------------------------- settings
  const setSize = (v: DockSize) => saveDock(m(), { size: v === "m" ? undefined : v });
  const setButtonSize = (pin: DockPin, v: DockSize) => {
    const sizes = { ...(cfg().dock.sizes ?? {}) };
    if (v === "m") delete sizes[pin];
    else sizes[pin] = v;
    saveDock(m(), { sizes });
  };
  const pickedPin = (): DockPin | null => {
    const p = picked();
    return p ? listOf(p.dock)[p.g]?.items[p.i] ?? null : null;
  };
  const splitOn = () => !!cfg().dock.split;
  const toggleSplit = (on: boolean) => {
    const gs = structuredClone(groups());
    if (on) {
      // Start the second dock with undo/redo and the pager, as most teachers want.
      const want: DockPin[] = ["undo", "redo", "pager"].filter((p) => placeable(m()).includes(p as DockPin)) as DockPin[];
      for (const g of gs.main) g.items = g.items.filter((x) => !want.includes(x));
      saveDock(m(), {
        groups: gs.main.filter((g) => g.items.length),
        split: { position: cfg().dock.position === "left" ? "right" : "left", orientation: "auto", groups: [{ id: "s1", items: want.length ? want : ["undo"] }] },
      });
    } else {
      gs.main.push(...gs.split.map((g) => ({ ...g, id: `g-${g.id}` })));
      saveDock(m(), { groups: gs.main, split: null });
    }
  };
  const reset = () => {
    const cur = modeConfig(m());
    setPrefs({ modes: { ...prefs().modes, [m()]: sanitizeMode(m(), { chrome: cur.chrome, tools: DEFAULT_MODES[m()].tools, dock: DEFAULT_MODES[m()].dock }) } });
    resetDock(m());
    controller.updateInsets(mode() === "teach");
    setPicked(null);
    showToast(`${m() === "teach" ? "Teach" : "Prepare"} dock reset to default`);
  };
  const warnings = () => dockWarnings(m(), cfg());

  const Chip = (p: { pin: DockPin; at: Slot | null }) => {
    const on = () => {
      const k = picked();
      return !!(k && p.at && k.dock === p.at.dock && k.g === p.at.g && k.i === p.at.i);
    };
    const size = () => cfg().dock.sizes?.[p.pin];
    return (
      <button
        class="de-chip"
        classList={{ picked: on(), core: isCore(p.pin), big: size() === "l", small: size() === "s" }}
        data-chip={p.pin}
        data-slot={p.at ? `${p.at.dock}-${p.at.g}-${p.at.i}` : undefined}
        aria-pressed={p.at ? on() : undefined}
        aria-label={
          p.at
            ? `${labelOf(p.pin)}${isCore(p.pin) ? ", always on a dock" : ""}. Enter to pick up and move with the arrow keys; Delete to take it off the dock`
            : `${labelOf(p.pin)}: add to the dock`
        }
        title={p.at ? `${labelOf(p.pin)}  ·  drag to move, Delete to remove` : `Add ${labelOf(p.pin)} to the dock`}
        onPointerDown={(e) => startDrag(e, p.pin, p.at)}
        onKeyDown={(e) => (p.at ? onChipKey(e, p.pin, p.at) : (e.key === "Enter" || e.key === " ") && (e.preventDefault(), place(p.pin, null, { dock: "main", g: Math.max(0, groups().main.length - 1), i: 99 })))}
        onClick={() => !p.at && !drag() && place(p.pin, null, { dock: "main", g: Math.max(0, groups().main.length - 1), i: 99 })}
      >
        <Icon name={iconOf(p.pin)} size={20} />
        <span>{labelOf(p.pin)}</span>
        <Show when={isCore(p.pin) && p.at}>
          <span class="de-lock" aria-hidden="true" title="Always on a dock">•</span>
        </Show>
      </button>
    );
  };

  const Groups = (p: { dock: "main" | "split" }) => (
    <div class="de-groups" classList={{ split: p.dock === "split" }}>
      <For each={groupsFor(p.dock)}>
        {(g, gi) => (
          <>
            <Show when={gi() > 0}>
              <div class="de-sep" aria-hidden="true" />
            </Show>
            <div class="de-group" data-drop-group={`${p.dock}-${gi()}`} role="group" aria-label={`${p.dock === "main" ? "Dock" : "Second dock"} group ${gi() + 1}`}>
              <For each={g.items}>{(pin, i) => <Chip pin={pin} at={{ dock: p.dock, g: gi(), i: i() }} />}</For>
              <Show when={!g.items.length}>
                <span class="de-empty">Drop buttons here</span>
              </Show>
            </div>
          </>
        )}
      </For>
      <button class="de-add-group" onClick={() => addGroup(p.dock)} title="A new group, separated from the others">
        <Icon name="plus" size={18} /><span>Separator</span>
      </button>
    </div>
  );

  return (
    <div class="de-backdrop" onPointerDown={(e) => e.target === e.currentTarget && close()}>
      <div class="de-sheet" ref={sheet} role="dialog" aria-modal="true" aria-labelledby="de-title">
        <header class="de-head">
          <h2 id="de-title">{isles() ? "Customize islands" : "Customize dock"}</h2>
          <div class="seg" role="radiogroup" aria-label="Mode">
            <For each={[["prepare", "Prepare"], ["teach", "Teach"]] as const}>
              {([v, l]) => (
                <button role="radio" aria-checked={m() === v} classList={{ on: m() === v }} onClick={() => (setM(v), setPicked(null))} data-autofocus={m() === v ? "" : undefined}>
                  {l}
                </button>
              )}
            </For>
          </div>
          <div class="grow" />
          <button class="btn icon de-close" onClick={close} aria-label="Close"><Icon name="close" /></button>
        </header>

        <div class="de-body">
          <section class="de-sec" aria-labelledby="de-buttons">
            <h3 id="de-buttons">Buttons</h3>
            <p class="de-hint">
              Drag buttons to arrange them, or press Enter on one and move it with the arrow keys. A separator starts a new group
              <Show when={isles()}>: each group is an island, the first at the left, the last at the right, the others in the middle</Show>.
            </p>
            <div class="de-dock-label">{isles() ? "Islands" : "Dock"}</div>
            <Groups dock="main" />
            <Show when={splitOn()}>
              <div class="de-dock-label">Second dock · {SPLIT_POS.find((p) => p.v === cfg().dock.split?.position)?.label ?? "Right"}</div>
              <Groups dock="split" />
            </Show>
            <Show when={pickedPin()}>
              {(pin) => (
                <div class="de-row de-picked">
                  <span class="de-label">{labelOf(pin())}</span>
                  <div class="seg" role="radiogroup" aria-label={`Size of ${labelOf(pin())}`}>
                    <For each={SIZES}>
                      {(s) => (
                        <button
                          role="radio"
                          aria-checked={(cfg().dock.sizes?.[pin()] ?? "m") === s.v}
                          classList={{ on: (cfg().dock.sizes?.[pin()] ?? "m") === s.v }}
                          onClick={() => setButtonSize(pin(), s.v)}
                        >
                          {s.label}
                        </button>
                      )}
                    </For>
                  </div>
                </div>
              )}
            </Show>
            <div class="de-dock-label">Available</div>
            <div class="de-tray" data-drop-tray="" role="group" aria-label="Available buttons">
              <For each={tray()}>{(pin) => <Chip pin={pin} at={null} />}</For>
              <Show when={!tray().length}>
                <span class="de-empty">Everything is on the dock</span>
              </Show>
            </div>
            <p class="de-hint">Pen, highlighter, eraser, laser and undo always stay on a dock. What is not on a dock waits in its ⋯ menu.</p>
          </section>

          <section class="de-sec" aria-labelledby="de-layouts">
            <h3 id="de-layouts">Layouts</h3>
            <div class="de-presets">
              <For each={[...DOCK_PRESETS, ...savedPresets()]}>
                {(p) => (
                  <span class="de-preset-wrap" classList={{ saved: !DOCK_PRESETS.includes(p) }}>
                    <button class="de-preset" title={p.hint} onClick={() => (applyPreset(p), setPicked(null), showToast(`Dock layout: ${p.name}`))}>
                      {p.name}
                    </button>
                    <Show when={!DOCK_PRESETS.includes(p)}>
                      <button
                        class="de-preset-x"
                        aria-label={`Delete the layout ${p.name}`}
                        onClick={() => setPrefs({ dockPresets: savedPresets().filter((x) => x !== p) })}
                      >
                        <Icon name="close" size={14} />
                      </button>
                    </Show>
                  </span>
                )}
              </For>
            </div>
            <form
              class="de-save"
              onSubmit={(e) => {
                e.preventDefault();
                if (!presetName().trim()) return;
                const p = savePreset(presetName());
                setPresetName("");
                showToast(`Saved the layout “${p.name}”`);
              }}
            >
              <input class="input" placeholder="Name this layout, e.g. Science lab" maxLength={32} value={presetName()} onInput={(e) => setPresetName(e.currentTarget.value)} aria-label="Layout name" />
              <button class="btn" type="submit" disabled={!presetName().trim()}>Save layout</button>
            </form>
          </section>

          <section class="de-sec" aria-labelledby="de-where">
            <h3 id="de-where">Where</h3>
            <div class="de-row">
              <span class="de-label">Look</span>
              <div class="seg" role="radiogroup" aria-label="Dock style">
                <For each={[["dock", "One dock"], ["islands", "Islands (Classic)"]] as const}>
                  {([v, l]) => (
                    <button
                      role="radio"
                      aria-checked={(cfg().dock.style ?? "dock") === v}
                      classList={{ on: (cfg().dock.style ?? "dock") === v }}
                      onClick={() => saveDock(m(), v === "islands" ? { style: "islands", position: "bottom", split: null, groups: cfg().dock.groups ?? groupsOf(m()).main } : { style: "dock" })}
                    >
                      {l}
                    </button>
                  )}
                </For>
              </div>
            </div>
            <div class="de-row">
              <span class="de-label">{isles() ? "Islands" : "Dock"}</span>
              <div class="seg" role="radiogroup" aria-label="Dock position">
                <For each={isles() ? POSITIONS.filter((p) => p.v === "bottom" || p.v === "top") : POSITIONS}>
                  {(p) => (
                    <button role="radio" aria-checked={cfg().dock.position === p.v} classList={{ on: cfg().dock.position === p.v }} onClick={() => saveDock(m(), { position: p.v })}>
                      {p.label}
                    </button>
                  )}
                </For>
              </div>
            </div>
            <div class="de-row" hidden={isles()}>
              <label class="check">
                <input type="checkbox" checked={splitOn()} onChange={(e) => toggleSplit(e.currentTarget.checked)} />
                <span>Second dock</span>
              </label>
              <Show when={splitOn()}>
                <div class="seg" role="radiogroup" aria-label="Second dock position">
                  <For each={SPLIT_POS}>
                    {(p) => (
                      <button
                        role="radio"
                        aria-checked={cfg().dock.split?.position === p.v}
                        classList={{ on: cfg().dock.split?.position === p.v }}
                        onClick={() => saveDock(m(), { split: { ...cfg().dock.split!, position: p.v } })}
                      >
                        {p.label}
                      </button>
                    )}
                  </For>
                </div>
              </Show>
            </div>
            <div class="de-row">
              <span class="de-label">Buttons</span>
              <div class="seg" role="radiogroup" aria-label="Button size">
                <For each={SIZES}>
                  {(s) => (
                    <button role="radio" aria-checked={(cfg().dock.size ?? "m") === s.v} classList={{ on: (cfg().dock.size ?? "m") === s.v }} onClick={() => setSize(s.v)}>
                      {s.label}
                    </button>
                  )}
                </For>
              </div>
              <label class="check">
                <input type="checkbox" checked={!!cfg().dock.large} onChange={(e) => saveDock(m(), { large: e.currentTarget.checked || undefined })} />
                <span>Large icons</span>
              </label>
            </div>
          </section>

          <Show when={warnings().length}>
            <div class="de-warn" role="status">
              <For each={warnings()}>{(w) => <p><Icon name="info" size={16} />{w}</p>}</For>
            </div>
          </Show>
        </div>

        <footer class="de-foot">
          <button class="btn" onClick={reset}><Icon name="restart" /><span>Reset to default</span></button>
          <div class="grow" />
          <button class="btn solid" onClick={close}>Done</button>
        </footer>
      </div>
      <Show when={drag()}>
        {(d) => (
          <div class="de-ghost" style={{ left: `${d().x}px`, top: `${d().y}px` }} aria-hidden="true">
            <Icon name={iconOf(d().pin)} size={20} /><span>{labelOf(d().pin)}</span>
          </div>
        )}
      </Show>
    </div>
  );
}

// For the console and e2e tests: rupokLayout.set("classic").
if (typeof window !== "undefined") (window as any).rupokLayout = { set: setLayout, get: layout };
