// Bindings from registry entries to stored preferences.

import { createSignal } from "solid-js";
import { controller } from "../controller";
import { PREF_DEFAULTS, type Prefs, prefs, setPrefs } from "../state";

/** get/set/default for a board pref (`rupok.prefs`); set applies at once. */
export function pref<K extends keyof Prefs>(key: K) {
  return {
    get: () => prefs()[key] as any,
    set: (v: any) => {
      setPrefs({ [key]: v } as Partial<Prefs>);
      controller.applyPrefs();
    },
    default: PREF_DEFAULTS[key] as any,
  };
}

/** A small JSON object in localStorage under its own key, as a signal:
 * `read()` and `write(patch)`. For features that keep their own key (e.g.
 * the PDF export defaults in `rupok.export`, read when the dialog opens). */
export function jsonStore<T extends object>(key: string, defaults: T) {
  const load = (): T => {
    try {
      return { ...defaults, ...JSON.parse(localStorage.getItem(key) || "{}") };
    } catch {
      return { ...defaults };
    }
  };
  const [value, setValue] = createSignal<T>(load());
  const write = (patch: Partial<T>) => {
    const next = { ...load(), ...patch };
    setValue(() => next);
    try {
      localStorage.setItem(key, JSON.stringify(next));
    } catch { /* storage unavailable */ }
  };
  return { read: value, write, defaults };
}
