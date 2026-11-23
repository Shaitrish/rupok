// The settings registry: every setting in Settings is one entry here.
//
// A feature adds a setting by registering it (see docs/settings.md); the
// dialog (components/settings/SettingsDialog.tsx) renders whatever is
// registered, so nobody edits the dialog to add a setting. Common controls
// (toggle, segmented, select, slider, colour, text, key, action) render
// generically from the entry; complex UI registers a `custom` row or a
// full-width `section` component.
//
// Ids are stable, dotted and camelCase ("touch.palmEraser"). They are the
// deep-link targets of `openSettings(id)` (settings/open.ts), so never
// rename one that shipped.

import { type Component, createSignal } from "solid-js";
import type { IconName } from "../icons.generated";

// ------------------------------------------------------------- categories

export type CategoryId =
  | "general" | "appearance" | "pen" | "touch" | "modes" | "ai" | "files" | "keyboard" | "about"
  | (string & {});

export interface Category {
  id: CategoryId;
  label: string;
  /** One line under the pane title. */
  description: string;
  icon: IconName;
  /** Sidebar position (lower first). Built-ins use 10, 20, … 90. */
  order: number;
  /** Extra words that find the whole category (English and Bangla). */
  keywords?: string[];
}

// ---------------------------------------------------------------- entries

export interface Option<T extends string = string> {
  value: T;
  label: string;
}

interface Base {
  /** Stable dotted id, also the deep-link target: "touch.palmEraser". */
  id: string;
  category: CategoryId;
  /** Sub-heading inside the category pane. Entries without one come first. */
  group?: string;
  label: string;
  /** One line of help under the label. Keep it short and concrete. */
  description?: string;
  /** Extra search words: synonyms, old names, Bangla. */
  keywords?: string[];
  /** Position inside its group (lower first; default 100). Most-used first. */
  order?: number;
  /** Hide the entry entirely when false (e.g. desktop-only settings). */
  when?: () => boolean;
  /** Dim and disable the control when false (it stays findable). */
  enabled?: () => boolean;
}

export type ToggleSetting = Base & { type: "toggle"; get: () => boolean; set: (v: boolean) => void; default?: boolean };
export type SegmentedSetting = Base & { type: "segmented"; options: Option[]; get: () => string; set: (v: string) => void; default?: string };
export type SelectSetting = Base & { type: "select"; options: Option[]; get: () => string; set: (v: string) => void; default?: string };
export type SliderSetting = Base & {
  type: "slider"; min: number; max: number; step: number;
  /** The value as shown next to the slider ("50%", "soft"). */
  format?: (v: number) => string;
  get: () => number; set: (v: number) => void; default?: number;
};
export type ColourSetting = Base & {
  type: "colour"; options: (Option & { color: string })[];
  get: () => string; set: (v: string) => void; default?: string;
};
export type TextSetting = Base & {
  type: "text"; placeholder?: string; maxLength?: number; secret?: boolean;
  get: () => string; set: (v: string) => void; default?: string;
};
/** A keyboard shortcut, shown read-only as key caps. */
export type KeySetting = Base & { type: "key"; keys: string[] };
export type ActionSetting = Base & {
  type: "action"; button: string; icon?: IconName; danger?: boolean;
  /** Ask once more before running ("Click again to …"). */
  confirm?: string;
  run: () => void | Promise<void>;
};
/** A row with the standard label and help, and a custom control. `block`
 * puts the control under the text instead of on the right. */
export type CustomSetting = Base & { type: "custom"; component: Component<SettingProps>; layout?: "inline" | "block"; reset?: () => void };
/** A full-width component with its own layout (Modes editor, Connect AI). */
export type SectionSetting = Base & { type: "section"; component: Component<SettingProps>; reset?: () => void };

/** Props given to custom and section components: the ids of the row's
 * label and help text, for aria-labelledby / aria-describedby. */
export interface SettingProps {
  labelId: string;
  descId: string;
}

export type SettingDef =
  | ToggleSetting | SegmentedSetting | SelectSetting | SliderSetting | ColourSetting | TextSetting
  | KeySetting | ActionSetting | CustomSetting | SectionSetting;

// ---------------------------------------------------------------- storage

const [categoryList, setCategoryList] = createSignal<Category[]>([]);
const [entryList, setEntryList] = createSignal<SettingDef[]>([]);

/** Add or replace a category (same id replaces, so HMR is safe). */
export function registerCategory(c: Category) {
  setCategoryList((list) => [...list.filter((x) => x.id !== c.id), c].sort((a, b) => a.order - b.order));
}

/** Add or replace settings (same id replaces). */
export function registerSettings(defs: SettingDef[]) {
  const ids = new Set(defs.map((d) => d.id));
  setEntryList((list) => [...list.filter((x) => !ids.has(x.id)), ...defs]);
}
export const registerSetting = (def: SettingDef) => registerSettings([def]);

export function unregisterSetting(id: string) {
  setEntryList((list) => list.filter((x) => x.id !== id));
}

export const categories = categoryList;
export const categoryById = (id: string) => categoryList().find((c) => c.id === id);
export const settingById = (id: string) => entryList().find((s) => s.id === id);
const visible = (s: SettingDef) => !s.when || s.when();
/** Every visible setting. */
export const allSettings = () => entryList().filter(visible);

export interface Group {
  name: string | undefined;
  items: SettingDef[];
}

/** A category's settings in display order, grouped. Ungrouped entries come
 * first; groups follow in the order of their lowest `order`. */
export function groupsOf(category: string): Group[] {
  const items = allSettings()
    .filter((s) => s.category === category)
    .sort((a, b) => (a.order ?? 100) - (b.order ?? 100));
  const groups = new Map<string | undefined, SettingDef[]>();
  for (const s of items) {
    const list = groups.get(s.group) ?? [];
    list.push(s);
    groups.set(s.group, list);
  }
  return [...groups.entries()]
    .map(([name, items]) => ({ name, items }))
    .sort((a, b) => (a.name === undefined ? -1 : b.name === undefined ? 1 : (a.items[0].order ?? 100) - (b.items[0].order ?? 100)));
}

// ----------------------------------------------------------------- reset

/** Whether "Reset this section" would change this entry. */
export function resettable(s: SettingDef): boolean {
  if (s.type === "custom" || s.type === "section") return !!s.reset;
  return "default" in s && s.default !== undefined;
}

/** Put one entry back to its default. */
export function resetSetting(s: SettingDef) {
  switch (s.type) {
    case "custom":
    case "section":
      s.reset?.();
      return;
    case "key":
    case "action":
      return;
    default:
      if (s.default !== undefined && s.get() !== s.default) (s.set as (v: unknown) => void)(s.default);
  }
}

export function resetCategory(category: string) {
  for (const s of allSettings()) if (s.category === category && resettable(s)) resetSetting(s);
}

// ----------------------------------------------------------------- search

export const normalize = (t: string) => t.normalize("NFC").toLocaleLowerCase().replace(/\s+/g, " ").trim();

function haystack(s: SettingDef): { label: string; rest: string } {
  const c = categoryById(s.category);
  const opts = "options" in s ? s.options.map((o) => o.label).join(" ") : "";
  const keys = s.type === "key" ? s.keys.join(" ") : "";
  return {
    label: normalize(s.label),
    rest: normalize([s.description, s.group, opts, keys, ...(s.keywords ?? []), c?.label, ...(c?.keywords ?? [])].filter(Boolean).join(" ")),
  };
}

export interface SearchHit {
  setting: SettingDef;
  score: number;
}

/** Settings matching every word of `query`, best first. Labels weigh most,
 * then option names and keywords, then descriptions. */
export function searchSettings(query: string): SearchHit[] {
  const words = normalize(query).split(" ").filter(Boolean);
  if (!words.length) return [];
  const order = new Map(categoryList().map((c, i) => [c.id, i]));
  const hits: SearchHit[] = [];
  for (const s of allSettings()) {
    const h = haystack(s);
    let score = 0;
    let ok = true;
    for (const w of words) {
      if (h.label.startsWith(w)) score += 6;
      else if (new RegExp(`(^|[\\s(/·-])${w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`).test(h.label)) score += 5;
      else if (h.label.includes(w)) score += 4;
      else if (h.rest.includes(w)) score += 1;
      else { ok = false; break; }
    }
    if (ok) hits.push({ setting: s, score });
  }
  return hits.sort(
    (a, b) => b.score - a.score || (order.get(a.setting.category) ?? 99) - (order.get(b.setting.category) ?? 99) || (a.setting.order ?? 100) - (b.setting.order ?? 100),
  );
}

// ------------------------------------------------------------- deep links

/** Old section names (the pre-registry `openSettings("ai")` callers). */
const LEGACY: Record<string, string> = {
  ai: "ai.claudeKey",
  smart: "pen.smartShapes",
  gestures: "touch",
  modes: "modes",
  library: "general.library",
  dock: "modes",
};

/** Resolve a deep-link target (a setting id, a category id or an old
 * section name) to the category to show and the setting to highlight. */
export function resolveTarget(target: string | undefined): { category: string; setting?: string } | null {
  if (!target) return null;
  const t = LEGACY[target] ?? target;
  const s = settingById(t);
  if (s) return { category: s.category, setting: s.id };
  if (categoryById(t)) return { category: t };
  // "touch.somethingNew" that is not registered (yet): at least the category.
  const head = t.split(".")[0];
  if (categoryById(head)) return { category: head };
  return null;
}
