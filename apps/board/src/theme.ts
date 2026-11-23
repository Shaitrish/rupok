// Website theme (all marketing pages): "system", "light" or "dark".
//
// The choice is stored in localStorage and applied as <html data-theme> by a
// tiny inline script in <head>, before first paint (see THEME_HEAD_SCRIPT in
// site/lib.mjs and index.html). "system" removes the attribute, so the CSS
// follows prefers-color-scheme. This module wires the nav toggle
// (.theme-toggle buttons with data-theme-set) and tells listeners when the
// effective theme changes.

export type ThemeChoice = "system" | "light" | "dark";
export type Theme = "light" | "dark";

const KEY = "rupok.site-theme";
const CHOICES: ThemeChoice[] = ["system", "light", "dark"];
const media = matchMedia("(prefers-color-scheme: dark)");

function stored(): ThemeChoice {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

let choice: ThemeChoice = stored();
const listeners = new Set<(t: Theme) => void>();

/** The theme the page shows now. */
export function currentTheme(): Theme {
  return choice === "system" ? (media.matches ? "dark" : "light") : choice;
}

/** Call `fn` whenever the effective theme changes. */
export function onThemeChange(fn: (t: Theme) => void): void {
  listeners.add(fn);
}

let last = currentTheme();
function apply() {
  const root = document.documentElement;
  if (choice === "system") delete root.dataset.theme;
  else root.dataset.theme = choice;
  const t = currentTheme();
  document.querySelector('meta[name="theme-color"]')?.setAttribute("content", t === "dark" ? "#111418" : "#ffffff");
  document.querySelectorAll<HTMLButtonElement>(".theme-toggle [data-theme-set]").forEach((b) => {
    b.setAttribute("aria-checked", String(b.dataset.themeSet === choice));
  });
  if (t !== last) {
    last = t;
    listeners.forEach((fn) => fn(t));
  }
}

export function setThemeChoice(next: ThemeChoice): void {
  choice = next;
  try {
    if (next === "system") localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, next);
  } catch { /* storage unavailable: the choice lasts for this page */ }
  apply();
}

document.querySelectorAll<HTMLButtonElement>(".theme-toggle [data-theme-set]").forEach((b) => {
  b.addEventListener("click", () => {
    const want = b.dataset.themeSet as ThemeChoice;
    // On narrow screens only the current option shows: clicking it cycles.
    setThemeChoice(want === choice ? CHOICES[(CHOICES.indexOf(choice) + 1) % CHOICES.length] : want);
  });
});
media.addEventListener("change", apply);
// Another tab changed the choice.
addEventListener("storage", (e) => {
  if (e.key === KEY || e.key === null) {
    choice = stored();
    apply();
  }
});
apply();
