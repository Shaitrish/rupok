import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import { Icon } from "./Icon";
import { PresetGallery } from "./PresetGallery";
import { APP_SLUG } from "../brand";

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
  // Escape closes it wherever focus is (after a tap, focus is often on the board).
  onMount(() => {
    const key = (e: KeyboardEvent) => e.key === "Escape" && props.onClose();
    window.addEventListener("keydown", key);
    onCleanup(() => window.removeEventListener("keydown", key));
  });
  return (
    <div class="scrim" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
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

// Settings moved to components/settings/SettingsDialog.tsx. Every setting is
// an entry in the settings registry (src/settings/, docs/settings.md): add a
// setting there, not here.
