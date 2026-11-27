// The Classic layout's panels (layout.ts): what opens from the islands.
// Each one sits in the dock's popover (DockShell → DockPopover) and calls
// the board's existing actions; nothing here re-implements a feature.
//
//   ☰ MainMenu      big labelled rows with a submenu pane (File, Import, …)
//   InkPanel        a titled panel: thickness with a live dot, illustrated
//                   pen tiles (icons/drawn/), 15 colours and an eyedropper
//   ShapesPanel     the same frame around the Shapes picker
//   ToolsBox        large tiles: text, highlighter, laser, media, …
//   ModeMenu, ZoomPanel, RecognizePanel, PageGrid

import { type JSX, For, Show, createSignal } from "solid-js";
import { controller } from "../../controller";
import { type BrushId, BRUSHES, TOOL_BRUSHES, brushInfo, brushOf, setBrushOf } from "../../brushes";
import { DRAWN, type DrawnName } from "../../icons.generated";
import type { IconName } from "../../icons.generated";
import { MODES, TOOLBAR_ITEMS, type ToolbarItem } from "../../modes";
import * as platform from "../../platform";
import {
  camera, canRedo, canUndo, hex, isView, mode, pageIndex, pages, prefs, selectionIds, selectionMeta, setPrefs, studentEntry, tool,
} from "../../state";
import { FILE_EXT } from "../../brand";
import { Icon } from "../Icon";
import { MarkerOpacity } from "../InkOptions";
import { ShapesPicker } from "../ShapesPicker";
import { currentShape, shapeLabel, openShapesPicker } from "../../shapes";
import { ThumbCanvas } from "../PageThumb";
import { addPageLike } from "../Chrome";
import { openExport } from "../ExportDialog";
import { openSettings } from "../../settings/open";
import { startTour } from "../../tour/state";
import { openLibrary } from "../../library/session";
import { pickImage, pickPdf, shareLesson } from "../Header";
import { insertVideo, insertWebEmbed, pickMedia, pickModel } from "../MediaDialog";
import { openPageDesign } from "../PageDesign";
import { openObjectDialog } from "./requests";
import { ToTextButton } from "../ToText";
import { SolveButton } from "../../solve/SolveUI";
import { celebrate } from "../../stamps";
import { openDockEditor } from "../../dock";
import { setLayout } from "../DockEditor";
import { openPagesPanel } from "./edge";
import { flipIslandsNow, foldIslandsNow, islandsOf } from "../../layout";
import { islandsAtTop } from "../../islands";

const allFolded = () => !!islandsOf()?.groups?.every((g) => g.collapsed);
const atTop = () => {
  const d = islandsOf();
  return !!d && islandsAtTop(d, prefs().islandSwap);
};
import { toggleAi, aiConnected } from "../AiStatus";

/** 15 ink colours, 3 × 5, chosen to read from the back of a classroom on
 * white, paper and dark boards. */
export const CLASSIC_COLORS = [
  0x1c2024ff, 0x60646cff, 0xffffffff,
  0xe5484dff, 0xf76b15ff, 0xffc53dff,
  0x30a46cff, 0x1f6f43ff, 0x12a594ff,
  0x0090ffff, 0x3e63ddff, 0x1e3a8aff,
  0x8e4ec6ff, 0xd6409fff, 0xa35829ff,
];

const same = (a: number, b: number) => a >>> 8 === b >>> 8;

/** One of our drawings (icons/drawn/), its ink in `color`. */
export function Drawing(props: { name: DrawnName; color: number; size?: number }) {
  const d = () => DRAWN[props.name];
  const vb = () => d().viewBox.split(" ").map(Number);
  const h = () => props.size ?? 72;
  return (
    <svg
      class="cl-art"
      viewBox={d().viewBox}
      width={(h() * vb()[2]) / vb()[3]}
      height={h()}
      aria-hidden="true"
      style={{ "--art-ink": hex(props.color) }}
      innerHTML={d().body}
    />
  );
}

/** The titled frame every Classic panel shares. */
export function Panel(props: { title: string; onClose: () => void; class?: string; children: JSX.Element }) {
  return (
    <div class={`cl-panel ${props.class ?? ""}`}>
      <div class="cl-title">
        <span>{props.title}</span>
        <button class="cl-x" title="Close  ·  Esc" aria-label="Close" onClick={props.onClose}>
          <Icon name="close" size={24} />
        </button>
      </div>
      <div class="cl-body">{props.children}</div>
    </div>
  );
}

/** Size: a labelled slider with its value, and a live sample of the line. */
function Thickness(props: { value: number; min: number; max: number; color: number; onChange: (w: number) => void; alpha?: number }) {
  const w = () => Math.max(1, Math.min(36, props.value));
  return (
    <div class="cl-thick">
      <div class="cl-thick-head">
        <span class="cl-label">Size</span>
        <output class="cl-value">{Math.round(props.value)} px</output>
      </div>
      <div class="cl-thick-row">
        <input
          class="slider cl-slider"
          type="range"
          min={props.min}
          max={props.max}
          step="1"
          value={props.value}
          aria-label="Size"
          aria-valuetext={`${Math.round(props.value)} pixels`}
          style={{ "--p": `${((props.value - props.min) / (props.max - props.min)) * 100}%` }}
          onInput={(e) => props.onChange(Number(e.currentTarget.value))}
        />
        <svg class="cl-sample" viewBox="0 0 104 52" width="104" height="52" aria-hidden="true">
          <line x1={w() / 2 + 14} y1="26" x2={90 - w() / 2} y2="26" stroke={hex(props.color)} stroke-opacity={props.alpha ?? 1} stroke-width={w()} stroke-linecap="round" />
        </svg>
      </div>
    </div>
  );
}

/** The colour grid and the eyedropper (the browser's own, where there is one). */
function ColourGrid(props: { value: number; onPick: (c: number) => void }) {
  let input!: HTMLInputElement;
  const drop = async () => {
    const ED = (window as any).EyeDropper;
    if (ED) {
      try {
        const r = await new ED().open();
        props.onPick(((parseInt(String(r.sRGBHex).slice(1, 7), 16) << 8) | 0xff) >>> 0);
      } catch { /* cancelled */ }
    } else input.click();
  };
  return (
    <div class="cl-colours" role="group" aria-label="Colours">
      <For each={CLASSIC_COLORS}>
        {(c) => (
          <button
            class="cl-swatch"
            classList={{ on: same(c, props.value), light: c === 0xffffffff }}
            style={{ background: hex(c) }}
            title={hex(c)}
            aria-label={`Colour ${hex(c)}`}
            aria-pressed={same(c, props.value)}
            onClick={() => props.onPick(c)}
          />
        )}
      </For>
      <button class="cl-swatch cl-drop" title="Pick a colour from the screen" aria-label="Eyedropper: pick a colour" onClick={() => void drop()}>
        <Icon name="pipette" size={24} />
      </button>
      <input
        ref={input}
        type="color"
        class="cl-hidden-color"
        tabIndex={-1}
        aria-hidden="true"
        value={hex(props.value)}
        onInput={(e) => props.onPick(((parseInt(e.currentTarget.value.slice(1), 16) << 8) | 0xff) >>> 0)}
      />
    </div>
  );
}

// ------------------------------------------------------------------ pen

export function InkPanel(props: {
  kind: "pen" | "highlighter";
  color: number;
  size: number;
  onSize: (w: number) => void;
  onColor: (c: number) => void;
  onTool: (t: "pen" | "highlighter") => void;
  onClose: () => void;
}) {
  const brush = () => brushOf(props.kind);
  const title = () => (props.kind === "highlighter" ? "Highlighter" : brush() === "pen" ? "Pen" : `${brushInfo(brush()).label} pen`);
  const pickBrush = (id: BrushId) => {
    if (props.kind !== "pen") props.onTool("pen");
    setBrushOf("pen", id);
    controller.applyPrefs();
  };
  return (
    <Panel title={title()} onClose={props.onClose} class="cl-ink">
      <div class="cl-main">
        <Thickness
          value={props.size}
          min={props.kind === "highlighter" ? 8 : 1}
          max={props.kind === "highlighter" ? 60 : 40}
          color={props.color}
          alpha={props.kind === "highlighter" ? 0.45 : 1}
          onChange={props.onSize}
        />
        <div class="cl-tray" role="radiogroup" aria-label="Pen type">
          <For each={BRUSHES.map((b) => b.id)}>
            {(id) => {
              const on = () => props.kind === "pen" && brush() === id;
              return (
                <button
                  class="cl-brush"
                  classList={{ on: on() }}
                  role="radio"
                  aria-checked={on()}
                  title={`${brushInfo(id).label}: ${brushInfo(id).hint}`}
                  data-brush={id}
                  onClick={() => pickBrush(id)}
                >
                  <Drawing name={`brush-${id}` as DrawnName} color={props.color} />
                  <span>{brushInfo(id).label}</span>
                </button>
              );
            }}
          </For>
          <button
            class="cl-brush"
            classList={{ on: props.kind === "highlighter" }}
            role="radio"
            aria-checked={props.kind === "highlighter"}
            title="Highlighter: see-through, ink stays readable underneath  ·  H"
            data-brush="highlighter"
            onClick={() => props.onTool("highlighter")}
          >
            <Drawing name="brush-highlighter" color={props.kind === "highlighter" ? props.color : 0xffe629ff} />
            <span>Highlighter</span>
          </button>
        </div>
        <Show when={props.kind === "highlighter"}>
          <Segmented
            label="Highlighter style"
            value={brushOf("highlighter")}
            options={TOOL_BRUSHES.highlighter.map((id) => [id, brushInfo(id).label] as const)}
            onPick={(id) => {
              setBrushOf("highlighter", id as BrushId);
              controller.applyPrefs();
            }}
          />
        </Show>
        <MarkerOpacity />
      </div>
      <ColourGrid value={props.color} onPick={props.onColor} />
    </Panel>
  );
}

// --------------------------------------------------------------- shapes

export function ShapesPanel(props: { color: number; size: number; onSize: (w: number) => void; onColor: (c: number) => void; onClose: () => void; focus?: "table" }) {
  return (
    <Panel title={tool() === "shape" ? shapeLabel(currentShape()) : "Shapes"} onClose={props.onClose} class="cl-shapes">
      <div class="cl-main">
        <Thickness value={props.size} min={1} max={40} color={props.color} onChange={props.onSize} />
        <ShapesPicker onDone={props.onClose} focus={props.focus} />
      </div>
      <ColourGrid value={props.color} onPick={props.onColor} />
    </Panel>
  );
}

// ------------------------------------------------------------ ☰ menu

interface Row { id: string; label: string; icon: IconName; key?: string; run?: () => void; sub?: Tile[]; hidden?: boolean }
interface Tile { id: string; label: string; icon: IconName; key?: string; run: () => void; on?: boolean; hidden?: boolean; disabled?: boolean }

export function MainMenu(props: { onClose: () => void }) {
  const view = isView;
  const go = (f: () => void) => () => {
    props.onClose();
    f();
  };
  const rows = (): Row[] => [
    {
      id: "file", label: "File", icon: "file", sub: [
        { id: "new", label: "New lesson", icon: "plus", run: go(() => openSettings("files.newLesson")), hidden: view() },
        { id: "open", label: "Open", icon: "open", key: "Ctrl O", run: go(() => controller.openFile()) },
        { id: "save", label: "Save", icon: "save", key: "Ctrl S", run: go(() => void controller.saveFile()), hidden: view() },
        { id: "save-as", label: "Save as…", icon: "download", run: go(() => { controller.fileHandle = null; void controller.saveFile(); }), hidden: view() },
        { id: "share-file", label: "Share", icon: "share", run: go(() => void shareLesson()), hidden: view() },
        { id: "library", label: "Library", icon: "library", run: go(() => void openLibrary()), hidden: studentEntry },
      ],
    },
    {
      id: "import", label: "Import", icon: "import", hidden: view(), sub: [
        { id: "image", label: "Image", icon: "image", run: go(() => void pickImage()) },
        { id: "pdf", label: "PDF", icon: "pdf", run: go(() => void pickPdf()) },
        { id: "object", label: "Smart Object", icon: "object", run: go(openObjectDialog) },
        { id: "video", label: "Video / YouTube", icon: "video", run: go(insertVideo) },
        { id: "audio", label: "Audio", icon: "audio", run: go(() => void pickMedia("audio")) },
        { id: "model", label: "3D model", icon: "cube", run: go(() => void pickModel()) },
        { id: "web", label: "Web page", icon: "globe", run: go(insertWebEmbed) },
        { id: "lesson", label: `Lesson (.${FILE_EXT})`, icon: "open", run: go(() => controller.openFile()) },
      ],
    },
    {
      id: "export", label: "Export", icon: "export", sub: [
        { id: "pdf-out", label: "PDF", icon: "pdf", key: "Ctrl Shift E", run: go(openExport) },
        { id: "rpk-out", label: `Lesson file (.${FILE_EXT})`, icon: "download", run: go(() => { controller.fileHandle = null; void controller.saveFile(); }), hidden: view() },
        { id: "share", label: "Share", icon: "share", run: go(() => void shareLesson()), hidden: view() },
      ],
    },
    { id: "print", label: "Print", icon: "print", run: go(openExport) },
    {
      id: "zoom", label: "Zoom", icon: "search", sub: [
        { id: "zoom-in", label: "Zoom in", icon: "zoomIn", key: "+", run: () => controller.zoomBy(1.25) },
        { id: "zoom-out", label: "Zoom out", icon: "zoomOut", key: "−", run: () => controller.zoomBy(0.8) },
        { id: "fit", label: "Fit page", icon: "grow", key: "0", run: go(() => controller.board.fit_page()) },
        { id: "fullscreen-z", label: "Fullscreen", icon: "fullscreen", key: "F", run: go(() => controller.toggleFullscreen()) },
      ],
    },
    {
      id: "toolbar", label: atTop() ? "Toolbar at bottom" : "Toolbar at top", icon: atTop() ? "down" : "up",
      run: go(flipIslandsNow), hidden: view(),
    },
    {
      id: "hide-toolbar", label: allFolded() ? "Show toolbar" : "Hide toolbar", icon: allFolded() ? "eye" : "down",
      run: go(() => foldIslandsNow(!allFolded())), hidden: view(),
    },
    { id: "settings", label: "Settings", icon: "settings", run: go(() => openSettings()), hidden: view() },
    {
      id: "help", label: "Help", icon: "help", sub: [
        { id: "tour", label: "Quick tour", icon: "play", run: go(startTour), hidden: view() },
        { id: "keys", label: "Shortcuts", icon: "keyboard", run: go(() => openSettings("keyboard")) },
        { id: "about", label: "Help & about", icon: "info", run: go(() => openSettings("about")) },
        { id: "ai", label: aiConnected() ? "AI assistant" : "Connect AI", icon: "smartink", run: go(toggleAi), hidden: view() },
      ],
    },
    {
      id: "mode", label: "Mode", icon: "monitor", sub: [
        ...MODES.map((m) => ({ id: `mode-${m.id}`, label: m.label, icon: m.icon, key: m.key, on: mode() === m.id, run: go(() => controller.setMode(m.id)) })),
        { id: "overlay", label: "Draw over apps", icon: "overlay", run: go(() => void platform.openOverlay?.()), hidden: !platform.isDesktop },
        { id: "fullscreen", label: "Fullscreen", icon: "fullscreen", key: "F", run: go(() => controller.toggleFullscreen()) },
        { id: "theme", label: dark() ? "Light theme" : "Dark theme", icon: dark() ? "sun" : "moon", run: () => setPrefs({ theme: dark() ? "light" : "dark" }) },
        { id: "modern", label: "Modern layout", icon: "sidebar", run: go(() => setLayout("modern")), hidden: view() },
      ],
    },
    { id: "exit", label: mode() === "teach" ? "Exit Teach" : "Library", icon: "exit", key: mode() === "teach" ? "Esc" : undefined, run: go(() => (mode() === "teach" ? controller.exitMode() : void openLibrary())), hidden: studentEntry && mode() !== "teach" },
  ];
  const shown = () => rows().filter((r) => !r.hidden);
  const [open, setOpen] = createSignal<string>("file");
  const sub = () => shown().find((r) => r.id === open())?.sub?.filter((t) => !t.hidden) ?? [];
  return (
    <div class="cl-menu" role="menu" aria-label="Main menu">
      <div class="cl-menu-rows">
        <For each={shown()}>
          {(r) => (
            <button
              class="cl-row"
              classList={{ on: !!r.sub && open() === r.id }}
              role="menuitem"
              aria-haspopup={r.sub ? "menu" : undefined}
              aria-expanded={r.sub ? open() === r.id : undefined}
              data-row={r.id}
              title={r.key ? `${r.label}  ·  ${r.key}` : r.label}
              onPointerEnter={(e) => e.pointerType === "mouse" && r.sub && setOpen(r.id)}
              onClick={() => (r.sub ? setOpen(r.id) : r.run?.())}
            >
              <Icon name={r.icon} size={28} active={!!r.sub && open() === r.id} />
              <span class="grow">{r.label}</span>
              <Show when={r.sub}><Icon name="next" size={20} /></Show>
            </button>
          )}
        </For>
      </div>
      <div class="cl-menu-sub" role="menu" aria-label={shown().find((r) => r.id === open())?.label}>
        <For each={sub()}>
          {(t) => (
            <button class="cl-tile" classList={{ on: !!t.on }} role="menuitem" data-tile={t.id} title={t.key ? `${t.label}  ·  ${t.key}` : t.label} onClick={t.run}>
              <Icon name={t.icon} size={32} active={!!t.on} />
              <span>{t.label}</span>
            </button>
          )}
        </For>
      </div>
    </div>
  );
}

function dark(): boolean {
  const t = prefs().theme;
  return t === "dark" || (t === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
}

// ---------------------------------------------------------------- mode

export function ModeMenu(props: { onClose: () => void }) {
  return (
    <Panel title="Mode" onClose={props.onClose} class="cl-list">
      <For each={MODES.filter((m) => !(studentEntry && m.id !== "view"))}>
        {(m) => (
          <button
            class="cl-row"
            classList={{ on: mode() === m.id }}
            role="menuitemradio"
            aria-checked={mode() === m.id}
            title={`${m.hint}  ·  ${m.key}`}
            onClick={() => {
              props.onClose();
              controller.setMode(m.id);
            }}
          >
            <Icon name={m.icon} size={28} active={mode() === m.id} />
            <span class="grow">{m.label}<small>{m.hint}</small></span>
            <kbd>{m.key}</kbd>
          </button>
        )}
      </For>
      <Show when={platform.isDesktop && !isView()}>
        <button class="cl-row" onClick={() => { props.onClose(); void platform.openOverlay?.(); }}>
          <Icon name="overlay" size={28} />
          <span class="grow">Draw over apps<small>Ink over any program on the screen</small></span>
        </button>
      </Show>
    </Panel>
  );
}

// ---------------------------------------------------------------- tools

const META = new Map(TOOLBAR_ITEMS.map((t) => [t.id, t]));

/** The tools box: what is not on the islands, as large labelled tiles. */
export function ToolsBox(props: { overflow: ToolbarItem[]; run: (t: ToolbarItem) => void; onClose: () => void }) {
  const go = (f: () => void) => () => {
    props.onClose();
    f();
  };
  const fixed = (): Tile[] => [
    { id: "text", label: "Text", icon: "text", key: "T", run: go(() => controller.setTool("text")), on: tool() === "text" },
    { id: "highlighter", label: "Highlighter", icon: "highlighter", key: "H", run: go(() => props.run("highlighter")), on: tool() === "highlighter" },
    { id: "laser", label: "Laser pointer", icon: "laser", key: "L", run: go(() => props.run("laser")), on: tool() === "laser" },
    { id: "table", label: "Table", icon: "layers", run: go(() => openShapesPicker("table")) },
    { id: "image", label: "Image", icon: "image", run: go(() => void pickImage()) },
    { id: "object", label: "Smart Object", icon: "object", run: go(openObjectDialog) },
    { id: "video", label: "Video", icon: "video", run: go(insertVideo) },
    { id: "replay", label: "Replay page", icon: "replay", run: go(() => controller.board.replay_start()) },
    { id: "celebrate", label: "Celebrate", icon: "celebrate", run: go(() => celebrate()) },
    { id: "design", label: "Page design", icon: "palette", run: go(openPageDesign) },
    { id: "customize", label: "Arrange islands", icon: "settings", run: go(() => openDockEditor(mode())) },
  ];
  const known = new Set(["text", "highlighter", "laser", "replay", "celebrate"]);
  const extra = (): Tile[] =>
    props.overflow
      .filter((t) => !known.has(t) && META.has(t) && !["rect", "ellipse", "line", "arrow", "shape", "pager"].includes(t))
      .map((t) => ({
        id: t, label: META.get(t)!.label, icon: META.get(t)!.icon, key: META.get(t)!.key, on: tool() === t,
        disabled: (t === "undo" && !canUndo()) || (t === "redo" && !canRedo()),
        run: go(() => props.run(t)),
      }));
  const tiles = () => (isView() ? extra() : [...fixed(), ...extra()]);
  return (
    <Panel title="Tools" onClose={props.onClose} class="cl-tools">
      <div class="cl-tiles" role="menu">
        <For each={tiles()}>
          {(t) => (
            <button class="cl-tile" classList={{ on: !!t.on }} role="menuitem" data-tile={t.id} disabled={t.disabled} title={t.key ? `${t.label}  ·  ${t.key}` : t.label} onClick={t.run}>
              <Icon name={t.icon} size={32} active={!!t.on} />
              <span>{t.label}</span>
            </button>
          )}
        </For>
      </div>
    </Panel>
  );
}

// ----------------------------------------------------------------- zoom

export function ZoomPanel(props: { onClose: () => void }) {
  return (
    <Panel title="Zoom" onClose={props.onClose} class="cl-zoom">
      <div class="cl-zoom-row">
        <button class="cl-big" title="Zoom out  ·  −" aria-label="Zoom out" onClick={() => controller.zoomBy(0.8)}><Icon name="zoomOut" size={32} /></button>
        <output class="cl-pct" aria-live="polite">{controller.zoomPct(camera()[2])}%</output>
        <button class="cl-big" title="Zoom in  ·  +" aria-label="Zoom in" onClick={() => controller.zoomBy(1.25)}><Icon name="zoomIn" size={32} /></button>
      </div>
      <button class="cl-row" onClick={() => controller.board.fit_page()} title="Fit page  ·  0">
        <Icon name="grow" size={28} /><span class="grow">Fit page</span><kbd>0</kbd>
      </button>
      <button class="cl-row" onClick={() => { props.onClose(); controller.toggleFullscreen(); }} title="Fullscreen  ·  F">
        <Icon name="fullscreen" size={28} /><span class="grow">Fullscreen</span><kbd>F</kbd>
      </button>
    </Panel>
  );
}

// ------------------------------------------------------------ recognise

export function RecognizePanel(props: { onClose: () => void }) {
  const smart = () => prefs().smartShapes;
  const on = () => smart() !== "off";
  const inkSelected = () => selectionIds().length > 0 && selectionMeta().strokes > 0;
  return (
    <Panel title="Recognise ink" onClose={props.onClose} class="cl-recog">
      <div class="cl-stack">
        <button
          class="cl-row"
          classList={{ on: on() }}
          role="menuitemcheckbox"
          aria-checked={on()}
          title="Smart shapes: rough circles, boxes, arrows and graphs become neat shapes  ·  M"
          onClick={() => controller.toggleSmartInk()}
        >
          <Icon name="smartink" size={28} active={on()} />
          <span class="grow">Smart shapes<small>Rough shapes become neat ones</small></span>
          <span class="dock-check" classList={{ on: on() }} />
        </button>
        {/* A child of Smart shapes: when they snap (greyed while they are off). */}
        <Segmented
          label="When shapes snap"
          disabled={!on()}
          value={on() ? smart() : prefs().smartLast}
          options={[["auto", "When I lift the pen"], ["hold", "When I hold still"]]}
          onPick={(v) => {
            setPrefs({ smartShapes: v as "auto" | "hold", smartLast: v as "auto" | "hold" });
            controller.applyPrefs();
          }}
        />
        <div class="cl-hr" />
        <Show
          when={inkSelected()}
          fallback={
            <button class="cl-row" onClick={() => { props.onClose(); controller.setTool("lasso"); }} title="Select handwriting first, then turn it into text">
              <Icon name="totext" size={28} />
              <span class="grow">To text<small>Select handwriting first: tap here, then circle it</small></span>
              <Icon name="next" size={20} />
            </button>
          }
        >
          <div class="cl-recog-actions">
            <ToTextButton />
            <SolveButton />
          </div>
        </Show>
      </div>
    </Panel>
  );
}

/** The Classic segmented control: full width, equal segments, one height. */
export function Segmented(props: { label: string; value: string; options: (readonly [string, string])[]; onPick: (v: string) => void; disabled?: boolean }) {
  return (
    <div class="cl-seg" classList={{ off: !!props.disabled }} role="radiogroup" aria-label={props.label} aria-disabled={props.disabled}>
      <For each={props.options}>
        {([v, l]) => (
          <button role="radio" aria-checked={props.value === v} classList={{ on: props.value === v }} disabled={props.disabled} onClick={() => props.onPick(v)}>
            {l}
          </button>
        )}
      </For>
    </div>
  );
}

/** The eraser: what it removes, its size as tiles, and Clear page. */
export function EraserPanel(props: {
  mode: "partial" | "stroke"; onMode: (m: "partial" | "stroke") => void;
  sizes: number[]; size: number; onSize: (w: number) => void;
  armed: boolean; onClear: () => void; canClear: boolean; onClose: () => void;
}) {
  return (
    <Panel title="Eraser" onClose={props.onClose} class="cl-eraser">
      <div class="cl-stack">
        <span class="cl-label">Removes</span>
        <Segmented label="What the eraser removes" value={props.mode} options={[["partial", "What it rubs"], ["stroke", "Whole strokes"]]} onPick={(v) => props.onMode(v as "partial")} />
        <span class="cl-label">Size</span>
        <div class="cl-tiles-row" role="radiogroup" aria-label="Eraser size">
          <For each={props.sizes}>
            {(w, i) => (
              <button class="cl-tile-s" classList={{ on: props.size === w }} role="radio" aria-checked={props.size === w} title={`Size ${w}`} aria-label={`Size ${w}`} onClick={() => props.onSize(w)}>
                <span class="cl-ring" style={{ width: `${10 + i() * 8}px`, height: `${10 + i() * 8}px` }} />
              </button>
            )}
          </For>
        </div>
        <Show when={props.canClear}>
          <div class="cl-hr" />
          <button class="cl-row danger" classList={{ armed: props.armed }} aria-live="polite" onClick={props.onClear}>
            <Icon name="broom" size={28} />
            <span class="grow">{props.armed ? "Tap again to clear the page" : "Clear page"}<small>Undo brings it back</small></span>
          </button>
        </Show>
      </div>
    </Panel>
  );
}

// ------------------------------------------------------------ page grid

/** The page button's grid: every slide as a numbered thumbnail. A tap goes
 * there; "+" adds one; the side panel has the page tools. */
export function PageGrid(props: { onClose: () => void }) {
  const go = (i: number) => {
    controller.board.goto_page(i);
    props.onClose();
  };
  return (
    <Panel title={`Pages · ${pages().length}`} onClose={props.onClose} class="cl-grid">
      <div class="cl-grid-list" role="listbox" aria-label="Pages">
        <For each={pages()}>
          {(p, i) => (
            <button
              class="cl-thumb"
              classList={{ on: i() === pageIndex() }}
              role="option"
              aria-selected={i() === pageIndex()}
              aria-label={`Page ${i() + 1}${p.title ? `: ${p.title}` : ""}`}
              style={{ background: hex(p.bg_color ?? 0xffffffff) }}
              onClick={() => go(i())}
            >
              <ThumbCanvas index={i()} current={i() === pageIndex()} itemCount={p.item_count ?? 0} />
              <span class="num">{i() + 1}</span>
            </button>
          )}
        </For>
        <Show when={!isView()}>
          <button class="cl-thumb cl-thumb-add" title="New page after this one (same look)" aria-label="New page" onClick={() => { addPageLike(false); }}>
            <Icon name="plus" size={32} />
          </button>
        </Show>
      </div>
      <div class="cl-grid-foot">
        <button class="btn" onClick={() => { props.onClose(); openPagesPanel("right"); }} title="The slide panel: reorder, duplicate, star, delete  ·  swipe in from the screen edge">
          <Icon name="organise" size={20} /><span>Organise pages…</span>
        </button>
      </div>
    </Panel>
  );
}

