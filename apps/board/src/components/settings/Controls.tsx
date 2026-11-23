// The generic controls of Settings, rendered from registry entries
// (settings/registry.ts). One row pattern: label and one line of help on the
// left, the control on the right. Every control is at least 40px tall for
// fingers on a classroom board, keyboard operable, and labelled for screen
// readers through the row's label and help ids.

import { For, Match, Show, Switch, createSignal, onCleanup } from "solid-js";
import { Dynamic } from "solid-js/web";
import type {
  ActionSetting, ColourSetting, KeySetting, Option, SegmentedSetting, SelectSetting, SettingDef, SliderSetting, TextSetting, ToggleSetting,
} from "../../settings/registry";
import { Icon } from "../Icon";

interface Aria {
  labelId: string;
  descId: string;
  disabled?: boolean;
}

export function SwitchControl(props: { def: ToggleSetting } & Aria) {
  return (
    <button
      type="button"
      class="st-switch"
      role="switch"
      aria-checked={props.def.get()}
      aria-labelledby={props.labelId}
      aria-describedby={props.descId || undefined}
      disabled={props.disabled}
      onClick={() => props.def.set(!props.def.get())}
    >
      <span class="st-switch-track" aria-hidden="true"><span class="st-switch-knob" /></span>
    </button>
  );
}

/** Arrow keys move between the options of a radio group (roving tabindex). */
function radioKeys(e: KeyboardEvent, options: Option[], value: string, pick: (v: string) => void) {
  const i = options.findIndex((o) => o.value === value);
  let j = -1;
  if (e.key === "ArrowRight" || e.key === "ArrowDown") j = (i + 1) % options.length;
  else if (e.key === "ArrowLeft" || e.key === "ArrowUp") j = (i - 1 + options.length) % options.length;
  else if (e.key === "Home") j = 0;
  else if (e.key === "End") j = options.length - 1;
  if (j < 0) return;
  e.preventDefault();
  pick(options[j].value);
  const group = (e.currentTarget as HTMLElement).closest("[role=radiogroup]");
  queueMicrotask(() => (group?.querySelectorAll<HTMLElement>("[role=radio]")[j])?.focus());
}

export function SegmentedControl(props: { def: SegmentedSetting } & Aria) {
  return (
    <div class="st-seg" role="radiogroup" aria-labelledby={props.labelId} aria-describedby={props.descId || undefined} aria-disabled={props.disabled}>
      <For each={props.def.options}>
        {(o) => {
          const on = () => props.def.get() === o.value;
          return (
            <button
              type="button"
              role="radio"
              aria-checked={on()}
              tabIndex={on() ? 0 : -1}
              classList={{ on: on() }}
              disabled={props.disabled}
              onClick={() => props.def.set(o.value)}
              onKeyDown={(e) => radioKeys(e, props.def.options, props.def.get(), props.def.set)}
            >
              {o.label}
            </button>
          );
        }}
      </For>
    </div>
  );
}

export function SelectControl(props: { def: SelectSetting } & Aria) {
  return (
    <span class="st-select">
      <select
        aria-labelledby={props.labelId}
        aria-describedby={props.descId || undefined}
        disabled={props.disabled}
        value={props.def.get()}
        onChange={(e) => props.def.set(e.currentTarget.value)}
      >
        <For each={props.def.options}>{(o) => <option value={o.value} selected={o.value === props.def.get()}>{o.label}</option>}</For>
      </select>
      <Icon name="down" size={16} />
    </span>
  );
}

export function SliderControl(props: { def: SliderSetting } & Aria) {
  const d = props.def;
  const pct = () => ((d.get() - d.min) / (d.max - d.min)) * 100;
  const text = () => (d.format ? d.format(d.get()) : String(d.get()));
  return (
    <span class="st-slider">
      <input
        type="range"
        min={d.min}
        max={d.max}
        step={d.step}
        value={d.get()}
        disabled={props.disabled}
        aria-labelledby={props.labelId}
        aria-describedby={props.descId || undefined}
        aria-valuetext={text()}
        style={{ "--p": `${pct()}%` }}
        onInput={(e) => d.set(Number(e.currentTarget.value))}
      />
      <output class="st-slider-value" aria-hidden="true">{text()}</output>
    </span>
  );
}

export function ColourControl(props: { def: ColourSetting } & Aria) {
  return (
    <div class="st-swatches" role="radiogroup" aria-labelledby={props.labelId} aria-describedby={props.descId || undefined}>
      <For each={props.def.options}>
        {(o) => {
          const on = () => props.def.get() === o.value;
          return (
            <button
              type="button"
              role="radio"
              class="st-swatch"
              classList={{ on: on() }}
              aria-checked={on()}
              aria-label={o.label}
              title={o.label}
              tabIndex={on() ? 0 : -1}
              disabled={props.disabled}
              style={{ "--sw": o.color }}
              onClick={() => props.def.set(o.value)}
              onKeyDown={(e) => radioKeys(e, props.def.options, props.def.get(), props.def.set)}
            >
              <span aria-hidden="true" />
            </button>
          );
        }}
      </For>
    </div>
  );
}

export function TextControl(props: { def: TextSetting } & Aria) {
  return (
    <input
      class="st-input"
      type={props.def.secret ? "password" : "text"}
      autocomplete="off"
      spellcheck={false}
      maxLength={props.def.maxLength}
      placeholder={props.def.placeholder}
      value={props.def.get()}
      disabled={props.disabled}
      aria-labelledby={props.labelId}
      aria-describedby={props.descId || undefined}
      onChange={(e) => props.def.set(e.currentTarget.value)}
      onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
    />
  );
}

export function KeyCaps(props: { def: KeySetting }) {
  return (
    <span class="st-keys" aria-label={props.def.keys.join(" or ")}>
      <For each={props.def.keys}>
        {(k, i) => (
          <>
            <Show when={i() > 0}><span class="st-keys-or" aria-hidden="true">/</span></Show>
            <span class="st-key-combo" aria-hidden="true">
              <For each={k.split(" ")}>{(part) => <kbd>{part}</kbd>}</For>
            </span>
          </>
        )}
      </For>
    </span>
  );
}

export function ActionControl(props: { def: ActionSetting } & Aria) {
  const [armed, setArmed] = createSignal(false);
  let t: number | undefined;
  onCleanup(() => clearTimeout(t));
  const click = () => {
    if (props.def.confirm && !armed()) {
      setArmed(true);
      clearTimeout(t);
      t = window.setTimeout(() => setArmed(false), 3500);
      return;
    }
    setArmed(false);
    void props.def.run();
  };
  return (
    <button
      type="button"
      class="st-btn"
      classList={{ danger: !!props.def.danger, armed: armed() }}
      disabled={props.disabled}
      aria-describedby={props.descId || undefined}
      onClick={click}
    >
      <Show when={props.def.icon}>{(i) => <Icon name={i()} size={18} />}</Show>
      <span>{armed() ? props.def.confirm : props.def.button}</span>
    </button>
  );
}

/** The control for any non-section entry. */
export function Control(props: { def: SettingDef } & Aria) {
  const p = props;
  return (
    <Switch>
      <Match when={p.def.type === "toggle"}><SwitchControl {...p} def={p.def as ToggleSetting} /></Match>
      <Match when={p.def.type === "segmented"}><SegmentedControl {...p} def={p.def as SegmentedSetting} /></Match>
      <Match when={p.def.type === "select"}><SelectControl {...p} def={p.def as SelectSetting} /></Match>
      <Match when={p.def.type === "slider"}><SliderControl {...p} def={p.def as SliderSetting} /></Match>
      <Match when={p.def.type === "colour"}><ColourControl {...p} def={p.def as ColourSetting} /></Match>
      <Match when={p.def.type === "text"}><TextControl {...p} def={p.def as TextSetting} /></Match>
      <Match when={p.def.type === "key"}><KeyCaps def={p.def as KeySetting} /></Match>
      <Match when={p.def.type === "action"}><ActionControl {...p} def={p.def as ActionSetting} /></Match>
      <Match when={p.def.type === "custom" || p.def.type === "section"}>
        <Dynamic component={(p.def as Extract<SettingDef, { component: unknown }>).component} labelId={p.labelId} descId={p.descId} />
      </Match>
    </Switch>
  );
}

const domId = (id: string) => `setting-${id.replace(/[^\w-]/g, "-")}`;
export { domId };

/** One settings row (or a full-width section). */
export function SettingRow(props: { def: SettingDef; highlight: boolean }) {
  const base = () => domId(props.def.id);
  const disabled = () => (props.def.enabled ? !props.def.enabled() : false);
  const block = () => props.def.type === "section" || (props.def.type === "custom" && props.def.layout === "block");
  return (
    <div
      id={base()}
      class="st-row"
      classList={{ hl: props.highlight, block: block(), section: props.def.type === "section", off: disabled(), key: props.def.type === "key" }}
      data-setting={props.def.id}
    >
      <Show
        when={props.def.type !== "section"}
        fallback={
          <>
            <span id={`${base()}-l`} class="st-sr">{props.def.label}</span>
            <span id={`${base()}-d`} class="st-sr">{props.def.description}</span>
            <Control def={props.def} labelId={`${base()}-l`} descId={`${base()}-d`} />
          </>
        }
      >
        <div class="st-row-text">
          <div class="st-label" id={`${base()}-l`}>{props.def.label}</div>
          <Show when={props.def.description}>
            <div class="st-help" id={`${base()}-d`}>{props.def.description}</div>
          </Show>
        </div>
        <div class="st-control">
          <Control def={props.def} labelId={`${base()}-l`} descId={props.def.description ? `${base()}-d` : ""} disabled={disabled()} />
        </div>
      </Show>
    </div>
  );
}
