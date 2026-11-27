# Settings: how to add a setting

Every setting in the board's **Settings** dialog is one entry in the settings
registry, `apps/board/src/settings/registry.ts`. The dialog
(`apps/board/src/components/settings/SettingsDialog.tsx`) renders whatever is
registered: the sidebar, the search, the rows, "Reset this section" and deep
links all come from the entries. **To add a setting you add one registry
entry. You never edit the dialog.**

## Where entries live

Each file in `apps/board/src/settings/defs/` registers its entries when it is
imported, and `settings/index.ts` imports every file in that folder
(`import.meta.glob`). So you can either:

- add an entry to the file of the category it belongs to (`touch.ts`,
  `files.ts`, …), or
- add a new file for your feature, e.g. `settings/defs/dock.ts`. Nothing else
  needs to import it.

Keep the controls of a complex feature in its own component under
`components/settings/` (or next to the feature) and register it as a
`custom` row or a `section`.

## Categories

| Id | Sidebar | What goes there |
|---|---|---|
| `general` | General | Role (Teacher/Student), your name, where lessons live |
| `appearance` | Appearance | Layout (Classic by default on the teacher board; Modern), the Classic islands, theme, accent, button size, dock side, icons, panels |
| `pen` | Pen & ink | Pressure, smoothing, writing hand; group "Smart ink" |
| `touch` | Touch & gestures | Finger touches, palm eraser, swipes, taps |
| `modes` | Teaching modes | The mode editor and the Smart Dock |
| `ai` | AI & agents | Connect AI, handwriting to text, API keys |
| `files` | Files & export | PDF export defaults, quiz answers, new lesson |
| `keyboard` | Keyboard shortcuts | One `key` entry per shortcut |
| `about` | About | Version and licences; group "Troubleshooting" |

Use an existing category. `registerCategory()` exists for the rare feature
that truly needs a new one.

## An entry

```ts
// apps/board/src/settings/defs/dock.ts
import { registerSettings } from "../registry";
import { pref } from "../bind";

registerSettings([
  {
    id: "modes.dockAutoHide",           // stable, dotted; also the deep-link target
    category: "modes",
    group: "Smart Dock",                // optional sub-heading in the pane
    order: 200,                         // lower first; put the most-used first
    type: "toggle",
    label: "Hide the dock while writing",
    description: "It fades out under the pen and comes back when you lift it.",
    keywords: ["toolbar", "fade", "ডক"],  // synonyms, old names, Bangla
    ...pref("dockAutoHide"),            // get, set and default for a key in `rupok.prefs`
  },
]);
```

Common fields (all entries):

| Field | |
|---|---|
| `id` | Required. `<category>.<camelCase>`. Never rename one that shipped: it is a deep-link target. |
| `category`, `label` | Required. Labels are short and sentence case. |
| `description` | One line of help under the label. Concrete, no marketing. |
| `group` | Sub-heading. Entries without a group come first. |
| `order` | Position (default 100). The most-used settings go first. |
| `keywords` | Extra search words, English and Bangla. Labels, descriptions, option names and the category are searched already. |
| `when` | `() => boolean`: hide the entry (e.g. desktop only). |
| `enabled` | `() => boolean`: dim and disable it but keep it findable (e.g. "Palm size" while the palm eraser is off). |

Control types:

| `type` | Extra fields |
|---|---|
| `toggle` | `get(): boolean`, `set(v)`, `default?` |
| `segmented` | `options: {value, label}[]` (2–4 short options), `get(): string`, `set(v)`, `default?` |
| `select` | as `segmented`, for longer lists |
| `slider` | `min`, `max`, `step`, `format?(v) => string`, `get(): number`, `set(v)`, `default?` |
| `colour` | `options: {value, label, color}[]`, `get`, `set`, `default?` |
| `text` | `placeholder?`, `maxLength?`, `secret?`, `get(): string`, `set(v)` (on change/Enter), `default?` |
| `key` | `keys: string[]`: a shortcut shown as key caps, e.g. `["Ctrl Z"]` |
| `action` | `button`, `icon?`, `danger?`, `confirm?` (asks once more), `run()` |
| `custom` | `component` (gets `{ labelId, descId }`), `layout?: "inline" \| "block"`, `reset?()` |
| `section` | `component`: full width with its own layout (Modes editor, Connect AI), `reset?()` |

Bindings (`settings/bind.ts`):

- `pref("key")` binds to a key of the board prefs (`rupok.prefs`, `state.ts`):
  `get`, a `set` that saves and applies at once, and the `default` from
  `PREF_DEFAULTS`. Add the key to `Prefs` and `defaults` in `state.ts` first.
- `jsonStore(key, defaults)` is a small signal-backed JSON object under its own
  localStorage key, for features that keep their own (e.g. `rupok.export`).
- Anything else: write `get`/`set` yourself. `get` must read a Solid signal so
  the control updates.

Rules:

- **Changes apply at once.** No Save button: `set` stores and applies.
- **Defaults make reset work.** An entry with `default` (or a `reset` for
  `custom`/`section`) is reset by "Reset this section". Leave `default` out for
  things a reset must not touch (role, name, API keys, calibration).
- Custom components use the shared classes (`st-btn`, `st-input`,
  `st-inline`, `st-status`) and tokens, are at least 40px tall, and label
  their controls with the `labelId`/`descId` they receive.
- Secrets (API keys) never go in `rupok.prefs` or the UI config export.

## Deep links

```ts
import { openSettings } from "../settings/open";
openSettings("touch.palmEraser"); // opens Touch & gestures, scrolls to it, highlights and focuses it
openSettings("ai");               // a category
openSettings();                   // the last category
```

Use it from popovers and hints ("Open Settings" in To text, a gesture hint,
"Customize dock…"). The old section names `ai`, `smart`, `gestures`, `modes`,
`library` and `dock` still work. In the browser console and in e2e tests:
`rupokSettings.open("touch.palmEraser")`.
