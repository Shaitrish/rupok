import { For, Show, createSignal } from "solid-js";
import { controller } from "../controller";
import type { IconVariant } from "../icons.generated";
import { ACCENTS, prefs, setPrefs, type Prefs } from "../state";
import { autosaveClear } from "../storage";
import { downloadQuiz } from "./Chrome";
import { Icon } from "./Icon";
import { ModesSettings } from "./ModesSettings";
import { SmartInkSettings } from "./SmartInkSettings";
import { PresetGallery } from "./PresetGallery";
import { APP_NAME, APP_SLUG } from "../brand";

const SAMPLE = `<!doctype html>
<html>
<head>
<title>Binary place values</title>
<style>
  html, body { height: 100%; margin: 0; }
  body { display: grid; place-items: center; font-family: var(--rupok-font-body); background: var(--rupok-color-bg); color: var(--rupok-color-fg); }
  h1 { font-size: 4.2vw; text-align: center; margin: 0 0 4vw; letter-spacing: -0.02em; }
  .bits { display: flex; gap: 1.6vw; }
  .col { display: flex; flex-direction: column; align-items: center; gap: 1vw; cursor: pointer; }
  .bit { width: 9vw; height: 9vw; border-radius: 2vw; display: grid; place-items: center; font-size: 5vw; font-weight: 800;
         background: #f0f0f3; transition: transform .25s, background .25s, opacity .4s, color .25s; opacity: .12; }
  .bit.shown { opacity: 1; }
  .bit.on { background: var(--rupok-color-accent); color: #fff; transform: translateY(-.8vw); }
  .val { font-size: 2.4vw; color: var(--rupok-color-muted); font-variant-numeric: tabular-nums; }
  .total { text-align: center; font-size: 5vw; margin-top: 4vw; font-weight: 800; font-variant-numeric: tabular-nums; }
</style>
</head>
<body>
<div>
  <h1>Binary: every place doubles</h1>
  <div class="bits" id="bits"></div>
  <div class="total" id="total">= 0</div>
</div>
<script>
  const vals = [128, 64, 32, 16, 8, 4, 2, 1];
  const on = new Array(8).fill(false);
  const root = document.getElementById('bits');
  vals.forEach((v, i) => {
    const col = document.createElement('div');
    col.className = 'col';
    col.innerHTML = '<div class="bit" id="b' + i + '">0</div><div class="val">' + v + '</div>';
    col.onclick = () => { on[i] = !on[i]; render(); rupok.state.save(on); };
    root.appendChild(col);
  });
  function render() {
    const step = rupok.steps.current;
    vals.forEach((v, i) => {
      const el = document.getElementById('b' + i);
      el.classList.toggle('shown', 7 - i < step);
      el.classList.toggle('on', on[i]);
      el.textContent = on[i] ? '1' : '0';
    });
    document.getElementById('total').textContent = '= ' + vals.reduce((s, v, i) => s + (on[i] ? v : 0), 0);
  }
  rupok.ready.then(() => { const s = rupok.state.load(); if (Array.isArray(s)) s.forEach((b, i) => on[i] = !!b); render(); });
  rupok.steps.onChange(render);
  render();
</script>
</body>
</html>`;

function Dialog(props: { icon: any; title: string; sub: any; width?: number; onClose: () => void; children: any }) {
  return (
    <div class="scrim" onClick={(e) => e.target === e.currentTarget && props.onClose()} onKeyDown={(e) => e.key === "Escape" && props.onClose()}>
      <div class="dialog" role="dialog" aria-modal="true" aria-label={props.title} style={{ width: props.width ? `min(${props.width}px, 100%)` : undefined }}>
        <div class="dialog-head">
          <div class="badge"><Icon name={props.icon} size={24} active /></div>
          <div style={{ flex: 1 }}>
            <h2>{props.title}</h2>
            <p class="sub">{props.sub}</p>
          </div>
          <button class="btn icon" onClick={props.onClose} aria-label="Close"><Icon name="close" /></button>
        </div>
        {props.children}
      </div>
    </div>
  );
}

export function ObjectDialog(props: { onClose: () => void }) {
  const [tab, setTab] = createSignal<"presets" | "html">("presets");
  const [html, setHtml] = createSignal("");
  const [title, setTitle] = createSignal("");
  const [steps, setSteps] = createSignal(0);
  const [size, setSize] = createSignal("1280x720");
  const [error, setError] = createSignal<string | null>(null);
  const add = () => {
    const [w, h] = size().split("x").map(Number);
    try {
      controller.board.add_object_html(html(), title().trim() || "Smart Object", w, h, steps());
      controller.setTool("interact");
      props.onClose();
    } catch (e: any) {
      setError(String(e));
    }
  };
  const loadFile = () => {
    const i = document.createElement("input");
    i.type = "file";
    i.accept = ".html,.htm";
    i.onchange = async () => {
      const f = i.files?.[0];
      if (!f) return;
      const text = await f.text();
      setHtml(text);
      if (!title()) setTitle(text.match(/<title>([^<]*)<\/title>/i)?.[1]?.trim() || f.name.replace(/\.html?$/i, ""));
    };
    i.click();
  };
  return (
    <Dialog
      icon="object"
      width={960}
      title="Add a Smart Object"
      sub={<>Start from a ready-made lesson, or paste HTML your AI made — a diagram, simulation, quiz or game. It runs live on the board, offline and sandboxed, and you can write on top of it. Agents can also add objects for you through <code>{APP_SLUG} mcp</code>.</>}
      onClose={props.onClose}
    >
      <div class="object-tabs">
        <Seg value={tab()} options={[["presets", "Presets"], ["html", "Your own HTML"]] as const} onChange={setTab} />
      </div>
      {/* Both panels share one grid cell, so the dialog keeps its height
          (and the HTML draft) when switching tabs. */}
      <div class="object-panels">
        <div class="object-panel" classList={{ off: tab() !== "presets" }}>
          <PresetGallery onInserted={props.onClose} />
        </div>
        <div class="object-panel form" classList={{ off: tab() !== "html" }}>
          <Show when={error()}>
            <div class="error-box">{error()}</div>
          </Show>
          <div class="row">
            <div class="field">
              <label for="obj-title">Title</label>
              <input id="obj-title" class="input" value={title()} onInput={(e) => setTitle(e.currentTarget.value)} placeholder="e.g. TCP three-way handshake" />
            </div>
            <div class="field" style={{ flex: "0 0 120px" }}>
              <label for="obj-steps">Steps</label>
              <input id="obj-steps" class="input" type="number" min="0" max="500" value={steps()} onInput={(e) => setSteps(Number(e.currentTarget.value))} />
            </div>
            <div class="field" style={{ flex: "0 0 160px" }}>
              <label for="obj-size">Size</label>
              <select id="obj-size" value={size()} onChange={(e) => setSize(e.currentTarget.value)}>
                <option value="1920x1080">Full slide</option>
                <option value="1280x720">Large</option>
                <option value="960x540">Medium</option>
                <option value="640x480">Small</option>
              </select>
            </div>
          </div>
          <div class="field fill">
            <label for="obj-html">HTML</label>
            <textarea id="obj-html" value={html()} onInput={(e) => setHtml(e.currentTarget.value)} placeholder="<!doctype html> …" spellcheck={false} />
          </div>
          <div class="actions">
            <button class="btn" onClick={() => { setHtml(SAMPLE); setTitle("Binary place values"); setSteps(8); setError(null); }}>
              <Icon name="object" /><span class="label">Try a sample</span>
            </button>
            <button class="btn" onClick={loadFile}><Icon name="open" /><span class="label">Open .html</span></button>
            <div class="grow" />
            <button class="btn" onClick={props.onClose}>Cancel</button>
            <button class="btn solid" disabled={!html().trim()} onClick={add}>Add to page</button>
          </div>
        </div>
      </div>
    </Dialog>
  );
}

export function Seg<T extends string>(props: { value: T; options: readonly (readonly [T, string])[]; onChange: (v: T) => void }) {
  return (
    <div class="seg" role="radiogroup">
      <For each={props.options}>
        {([v, label]) => (
          <button role="radio" aria-checked={props.value === v} classList={{ on: props.value === v }} onClick={() => props.onChange(v)}>
            {label}
          </button>
        )}
      </For>
    </div>
  );
}

const ICON_STYLES: readonly (readonly [IconVariant, string])[] = [
  ["linear", "Linear"], ["line-duotone", "Duotone"], ["broken", "Broken"], ["bold", "Bold"],
];

export function SettingsDialog(props: { onClose: () => void }) {
  const p = prefs;
  const set = (x: Partial<Prefs>) => {
    setPrefs(x);
    controller.applyPrefs();
  };
  return (
    <Dialog icon="settings" title="Settings" sub={`Make ${APP_NAME} fit your board, your hands and your style. Saved in this browser.`} width={720} onClose={props.onClose}>
      <div class="section-title">Appearance</div>
      <div class="settings-grid">
        <div class="field">
          <label>Theme</label>
          <Seg value={p().theme} options={[["system", "System"], ["light", "Light"], ["dark", "Dark"]] as const} onChange={(v) => set({ theme: v })} />
        </div>
        <div class="field">
          <label>Accent</label>
          <div class="accent-row">
            <For each={ACCENTS}>
              {(a) => (
                <button
                  class="accent-dot"
                  classList={{ on: p().accent === a.id }}
                  style={{ background: a.color, color: a.color }}
                  title={a.label}
                  aria-label={`Accent ${a.label}`}
                  onClick={() => set({ accent: a.id })}
                />
              )}
            </For>
          </div>
        </div>
        <div class="field">
          <label>Icons</label>
          <Seg value={p().iconStyle} options={ICON_STYLES} onChange={(v) => set({ iconStyle: v })} />
        </div>
        <div class="field">
          <label>Toolbar</label>
          <Seg value={p().toolbar} options={[["bottom", "Bottom"], ["left", "Left"], ["right", "Right"]] as const} onChange={(v) => set({ toolbar: v })} />
        </div>
        <div class="field">
          <label>Size</label>
          <Seg value={p().density} options={[["compact", "Compact"], ["comfortable", "Comfortable"], ["large", "Large board"]] as const} onChange={(v) => set({ density: v })} />
        </div>
        <div class="field">
          <label>Statistics overlay</label>
          <Seg value={p().showStats ? "on" : "off"} options={[["off", "Hidden"], ["on", "Shown"]] as const} onChange={(v) => set({ showStats: v === "on" })} />
        </div>
      </div>

      <div class="section-title">Pen & touch</div>
      <div class="settings-grid">
        <div class="field">
          <label>Finger touches</label>
          <Seg value={p().touchMode} options={[["auto", "Auto"], ["draw", "Draw"], ["navigate", "Pan & zoom"]] as const} onChange={(v) => set({ touchMode: v })} />
          <span class="hint">Auto: fingers draw on touch-only boards and switch to pan & zoom once a pen is used (palm rejection).</span>
        </div>
        <div class="field">
          <label>Renderer</label>
          <Seg
            value={p().forceWebgl ? "webgl" : "webgpu"}
            options={[["webgl", "WebGL2"], ["webgpu", "WebGPU · beta"]] as const}
            onChange={(v) => { setPrefs({ forceWebgl: v === "webgl" }); location.reload(); }}
          />
          <span class="hint">Switching reloads the page. Your lesson is autosaved.</span>
        </div>
        <div class="field">
          <label for="smooth">Smoothing · {Math.round(p().smoothing * 100)}%</label>
          <input id="smooth" class="slider" type="range" min="0" max="1" step="0.05" value={p().smoothing} onInput={(e) => set({ smoothing: Number(e.currentTarget.value) })} />
          <span class="hint">Higher removes shake on large boards; lower feels more direct.</span>
        </div>
        <div class="field">
          <label for="pressure">Pen pressure · {p().pressure < 0.8 ? "soft" : p().pressure > 1.2 ? "firm" : "normal"}</label>
          <input id="pressure" class="slider" type="range" min="0.3" max="2" step="0.05" value={p().pressure} onInput={(e) => set({ pressure: Number(e.currentTarget.value) })} />
        </div>
      </div>

      <SmartInkSettings />

      <div class="section-title">Modes</div>
      <ModesSettings />

      <div class="section-title">Keyboard & clicker</div>
      <div class="kbd-grid">
        <For each={[["Pen", "P"], ["Highlighter", "H"], ["Eraser", "E"], ["Select", "V"], ["Laser", "L"], ["Text", "T"], ["Shapes", "R O N A"], ["Smart shapes", "M"], ["Use objects", "I"], ["Pan", "Space"], ["Next step / page", "→"], ["Previous", "←"], ["Fit page", "0"], ["Fullscreen", "F"], ["Undo / redo", "Ctrl Z / Y"], ["Save", "Ctrl S"], ["Copy / paste", "Ctrl C / V"], ["Duplicate", "Ctrl D"], ["Group / ungroup", "Ctrl G / ⇧G"], ["Merge ink", "Ctrl E"], ["Teach", "F5"], ["Modes", "Alt 1 2 3"]]}>
          {([label, key]) => <div><span>{label}</span><kbd>{key}</kbd></div>}
        </For>
      </div>

      <div class="actions" style={{ "margin-top": "24px" }}>
        <button class="btn" onClick={downloadQuiz}><Icon name="download" /><span class="label">Quiz answers</span></button>
        <button class="btn danger" onClick={async () => { await autosaveClear(); controller.board.new_lesson("Untitled lesson"); props.onClose(); }}>
          <Icon name="trash" /><span class="label">New empty lesson</span>
        </button>
        <div class="grow" />
        <button class="btn solid" onClick={props.onClose}>Done</button>
      </div>
      <p class="hint" style={{ "margin-top": "16px" }}>{APP_NAME} is open source (AGPL-3.0) by Shaitrish. Icons: Solar by 480 Design (CC BY 4.0). Typeface: Figtree (OFL).</p>
    </Dialog>
  );
}
