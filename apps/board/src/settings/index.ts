// Loads every settings definition. Each file in ./defs registers its own
// entries with the registry on import, so a feature adds settings by adding
// (or editing) one file there: no import list to update, no dialog edits.
// See docs/settings.md.

import.meta.glob("./defs/*.{ts,tsx}", { eager: true });

export * from "./registry";
export { openSettings, closeSettings, settingsOpen } from "./open";
