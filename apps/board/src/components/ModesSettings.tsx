// Settings → Teaching modes: per-mode chrome and toolbar, plus import/export of the
// whole UI config as JSON (format: docs/ui-config.md), so a teacher can use a
// config that Claude or any AI wrote for them.

import { For, Show, createSignal } from "solid-js";
import { controller } from "../controller";
import { layout } from "../layout";
import { setLayout } from "./DockEditor";
import {
  ALLOWED_CHROME, ALLOWED_TOOLS, CHROME_ITEMS, CORE_TOOLS, DEFAULT_MODES, DOCK_POSITIONS, MODES, SHAPE_TOOLS, TOOLBAR_ITEMS, UI_CONFIG_FORMAT,
  UI_CONFIG_VERSION, type ChromeKey, type DockConfig, type DockPin, type Mode, type ModeConfig, type ToolbarItem, type UiConfigFile,
  resolveMode, sanitizeDockPresets, sanitizeModes,
} from "../modes";
import { openDockEditor } from "../dock";
import { ACCENTS, type Prefs, mode, modeConfig, prefs, setPrefs, showToast } from "../state";
import { exportGestures, importGestures } from "../gesturePrefs";
import { Icon } from "./Icon";

const TOOL_META = new Map(TOOLBAR_ITEMS.map((t) => [t.id, t]));
const APPEARANCE: Record<string, readonly string[]> = {
  theme: ["system", "light", "dark"],
  iconStyle: ["linear", "line-duotone", "broken", "bold"],
  toolbar: ["bottom", "left", "right"],
  density: ["compact", "comfortable", "large"],
  hand: ["right", "left"],
};
const POSITION_LABEL: Record<(typeof DOCK_POSITIONS)[number], string> = {
  auto: "Auto (Appearance → Dock side)", bottom: "Bottom", top: "Top", left: "Left", right: "Right",
  "bottom-left": "Bottom left", "bottom-right": "Bottom right", "top-left": "Top left", "top-right": "Top right",
};

/** The current UI config as a file anyone (or any AI) can edit and import. */
export function exportUiConfig(): UiConfigFile {
  const p = prefs();
  return {
    format: UI_CONFIG_FORMAT,
    version: UI_CONFIG_VERSION,
    modes: Object.fromEntries(MODES.map((m) => [m.id, resolveMode(p.modes, m.id)])) as UiConfigFile["modes"],
    appearance: { theme: p.theme, accent: p.accent, iconStyle: p.iconStyle, toolbar: p.toolbar, density: p.density, hand: p.hand, layout: layout(), islandSwap: p.islandSwap },
    gestures: exportGestures(),
    dockPresets: p.dockPresets ?? [],
  };
}

/** Apply an imported UI config. Unknown keys, tools and values are ignored. */
export function importUiConfig(raw: unknown): string {
  if (!raw || typeof raw !== "object") throw new Error("not a JSON object");
  const f = raw as Partial<UiConfigFile>;
  if (f.format !== UI_CONFIG_FORMAT) throw new Error(`"format" must be "${UI_CONFIG_FORMAT}"`);
  if (typeof f.version !== "number" || f.version > UI_CONFIG_VERSION) throw new Error(`unsupported version ${f.version}`);
  const next: Partial<Prefs> = {};
  const modes = sanitizeModes(f.modes);
  if (Object.keys(modes).length) next.modes = { ...prefs().modes, ...modes };
  const a = (f.appearance ?? {}) as Record<string, unknown>;
  for (const [k, allowed] of Object.entries(APPEARANCE)) {
    if (typeof a[k] === "string" && allowed.includes(a[k] as string)) (next as any)[k] = a[k];
  }
  if (typeof a.accent === "string" && ACCENTS.some((x) => x.id === a.accent)) next.accent = a.accent as Prefs["accent"];
  if (f.dockPresets !== undefined) next.dockPresets = sanitizeDockPresets(f.dockPresets);
  setPrefs(next);
  // A layout without its own modes: Classic applies the Classic preset (a file's modes win).
  if (typeof a.islandSwap === "boolean") next.islandSwap = a.islandSwap;
  if ((a.layout === "classic" || a.layout === "modern") && !f.modes?.prepare && !f.modes?.teach) setLayout(a.layout);
  controller.applyPrefs();
  controller.updateInsets(true);
  const parts = [...Object.keys(modes).map((m) => MODES.find((x) => x.id === m)!.label)];
  if (Object.keys(next).some((k) => k !== "modes")) parts.push("appearance");
  if (f.gestures !== undefined && importGestures(f.gestures)) parts.push("gestures");
  return parts.length ? `Imported UI config: ${parts.join(", ")}` : "The file had nothing to import";
}

export function ModesSettings() {
  const [editing, setEditing] = createSignal<Mode>(mode());
  const cfg = () => modeConfig(editing());
  const save = (next: ModeConfig) => {
    setPrefs({ modes: { ...prefs().modes, [editing()]: next } });
    controller.updateInsets(mode() === "teach");
  };
  const setChrome = (k: ChromeKey, on: boolean) => save({ ...cfg(), chrome: { ...cfg().chrome, [k]: on } });
  const setTool = (id: ToolbarItem, on: boolean) =>
    save({ ...cfg(), tools: on ? [...cfg().tools, id] : cfg().tools.filter((t) => t !== id) });
  /** Core tools are always on the dock in Prepare and Teach. */
  const isCore = (id: ToolbarItem) => editing() !== "view" && CORE_TOOLS.includes(id);
  const pinOf = (id: ToolbarItem): DockPin => (SHAPE_TOOLS.includes(id) ? "shapes" : id);
  const pinned = (id: ToolbarItem) => cfg().dock.pinned.includes(pinOf(id));
  const setDock = (d: Partial<DockConfig>) => save({ ...cfg(), dock: { ...cfg().dock, ...d } });
  const setPinned = (id: ToolbarItem, on: boolean) => {
    const pin = pinOf(id);
    const rest = cfg().dock.pinned.filter((x) => x !== pin);
    setDock({ pinned: on ? [...rest, pin] : rest });
  };
  const move = (id: ToolbarItem, dir: -1 | 1) => {
    const tools = [...cfg().tools];
    const i = tools.indexOf(id), j = i + dir;
    if (i < 0 || j < 0 || j >= tools.length) return;
    [tools[i], tools[j]] = [tools[j], tools[i]];
    save({ ...cfg(), tools });
  };
  /** Enabled tools in toolbar order, then the rest in catalogue order. */
  const rows = () => {
    const on = cfg().tools;
    return [...on, ...ALLOWED_TOOLS[editing()].filter((t) => !on.includes(t))];
  };
  const reset = () => {
    const rest = { ...prefs().modes };
    delete rest[editing()];
    setPrefs({ modes: rest });
    controller.updateInsets(mode() === "teach");
    showToast(`${MODES.find((m) => m.id === editing())!.label} mode reset to default`);
  };
  const download = () => {
    const blob = new Blob([JSON.stringify(exportUiConfig(), null, 2)], { type: "application/json" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = "rupok-ui-config.json";
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 2000);
  };
  const upload = () => {
    const i = document.createElement("input");
    i.type = "file";
    i.accept = ".json,application/json";
    i.onchange = async () => {
      const f = i.files?.[0];
      if (!f) return;
      try {
        showToast(importUiConfig(JSON.parse(await f.text())), 4000);
      } catch (e: any) {
        showToast(`Could not import ${f.name}: ${e?.message ?? e}`, 6000);
      }
    };
    i.click();
  };
  const isDefault = () => JSON.stringify(cfg()) === JSON.stringify(DEFAULT_MODES[editing()]);

  return (
    <div class="modes-settings">
      <div class="seg" role="radiogroup" aria-label="Mode to configure">
        <For each={MODES}>
          {(m) => (
            <button role="radio" aria-checked={editing() === m.id} classList={{ on: editing() === m.id }} onClick={() => setEditing(m.id)}>
              <Icon name={m.icon} size={16} active={editing() === m.id} />{m.label}
            </button>
          )}
        </For>
      </div>
      <div class="modes-grid">
        <div class="field">
          <label>Show</label>
          <div class="check-grid">
            <For each={CHROME_ITEMS.filter((c) => ALLOWED_CHROME[editing()].includes(c.id))}>
              {(c) => (
                <label class="check">
                  <input type="checkbox" checked={cfg().chrome[c.id]} onChange={(e) => setChrome(c.id, e.currentTarget.checked)} />
                  <span>{c.label}</span>
                </label>
              )}
            </For>
          </div>
        </div>
        <div class="field">
          <label>Toolbar, in order</label>
          <ol class="tool-list">
            <For each={rows()}>
              {(id) => {
                const on = () => cfg().tools.includes(id);
                const at = () => cfg().tools.indexOf(id);
                return (
                  <li classList={{ off: !on() }}>
                    <label class="check">
                      <input type="checkbox" checked={on()} disabled={isCore(id)} onChange={(e) => setTool(id, e.currentTarget.checked)} />
                      <Icon name={TOOL_META.get(id)!.icon} size={16} />
                      <span>{TOOL_META.get(id)!.label}</span>
                    </label>
                    <Show when={on()}>
                      <Show
                        when={!isCore(id) && !(id === "smartink" && cfg().dock.pinned.includes("shapes"))}
                        fallback={<span class="pin-toggle fixed">{isCore(id) ? "Always" : "In Shapes"}</span>}
                      >
                        <button
                          class="pin-toggle"
                          classList={{ on: pinned(id) }}
                          aria-pressed={pinned(id)}
                          title={pinned(id) ? "On the dock. Click to move it to the ⋯ menu" : "In the ⋯ menu. Click to pin it to the dock"}
                          onClick={() => setPinned(id, !pinned(id))}
                        >
                          {pinned(id) ? "Dock" : "⋯ menu"}
                        </button>
                      </Show>
                      <button class="btn icon" disabled={at() === 0} onClick={() => move(id, -1)} aria-label={`Move ${TOOL_META.get(id)!.label} up`}>
                        <Icon name="up" size={16} />
                      </button>
                      <button class="btn icon" disabled={at() === cfg().tools.length - 1} onClick={() => move(id, 1)} aria-label={`Move ${TOOL_META.get(id)!.label} down`}>
                        <Icon name="down" size={16} />
                      </button>
                    </Show>
                  </li>
                );
              }}
            </For>
          </ol>
        </div>
      </div>
      <div class="dock-settings">
        <div class="field">
          <label for="dock-pos">Dock starts at</label>
          <select
            id="dock-pos"
            value={typeof cfg().dock.position === "string" ? (cfg().dock.position as string) : "float"}
            onChange={(e) => setDock({ position: e.currentTarget.value as DockConfig["position"] })}
          >
            <For each={DOCK_POSITIONS}>{(p) => <option value={p}>{POSITION_LABEL[p]}</option>}</For>
            <Show when={typeof cfg().dock.position === "object"}>
              <option value="float" disabled>Floating (from a UI config)</option>
            </Show>
          </select>
        </div>
        <div class="field">
          <label>Orientation</label>
          <div class="seg" role="radiogroup" aria-label="Dock orientation">
            <For each={[["auto", "Auto"], ["horizontal", "Across"], ["vertical", "Down"]] as const}>
              {([v, l]) => (
                <button role="radio" aria-checked={cfg().dock.orientation === v} classList={{ on: cfg().dock.orientation === v }} onClick={() => setDock({ orientation: v })}>
                  {l}
                </button>
              )}
            </For>
          </div>
        </div>
        <label class="check">
          <input type="checkbox" checked={cfg().dock.minimized} onChange={(e) => setDock({ minimized: e.currentTarget.checked })} />
          <span>Start minimised (a small round button)</span>
        </label>
      </div>
      <Show when={editing() !== "view"}>
        <button class="btn solid" onClick={() => openDockEditor(editing())}>
          <Icon name="settings" /><span class="label">Customize dock…</span>
        </button>
      </Show>
      <p class="hint">Pen, highlighter, eraser, laser and undo are always on the dock in Prepare and Teach. Tools set to “⋯ menu” wait in the dock's ⋯ button. Drag the dock by its grip; it remembers where you leave it on each screen size.</p>
      <div class="actions">
        <button class="btn" onClick={upload}><Icon name="import" /><span class="label">Import UI config</span></button>
        <button class="btn" onClick={download}><Icon name="export" /><span class="label">Export UI config</span></button>
        <div class="grow" />
        <button class="btn" disabled={isDefault()} onClick={reset}><Icon name="restart" /><span class="label">Reset to default</span></button>
      </div>
      <p class="hint">A UI config is a small JSON file. Ask Claude or any AI to write one for your class (the format is in docs/ui-config.md), then import it here.</p>
    </div>
  );
}
