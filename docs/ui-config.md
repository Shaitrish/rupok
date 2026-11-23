# UI config (modes, toolbar, appearance)

A **UI config** is a small JSON file that tells the Rupok board which chrome
and which tools to show in each mode. A teacher imports it in
**Settings → Teaching modes → Import UI config**; **Export UI config** writes the
current setup in the same format. The format is meant to be written by people
and by AI agents ("make me a Teach mode with only a red and a blue pen and no
undo"). Later the desktop app and the MCP server will be able to write it
directly.

The board stores the same `modes` object in the browser's `localStorage`
under `rupok.prefs` → `modes`. Source of truth: `apps/board/src/modes.ts`.

Adding a setting to the Settings dialog is separate: see
[settings.md](settings.md) (one registry entry, no dialog edits).

## Modes

| Id | Name | What it is for |
|---|---|---|
| `prepare` | Prepare | Building the lesson: every tool and panel. |
| `teach` | Teach | Presenting. The slide fills the screen edge to edge with no margin (on resize, page change and fullscreen; a zoom or pan snaps back when released). In a window that is not 16:9 the page touches the limiting edges and the rest is painted in the page colour. Chrome floats over the slide and the toolbar fades after 3 idle seconds. F5 enters, Esc or the exit button leaves. |
| `view` | Student | Read-only: page through, zoom, use Smart Objects, replay. Opening `/board/?view` starts here. |

The desktop app also shows **Overlay** in the mode switcher (draw over any
app). It has no config.

## File format

```json
{
  "format": "rupok-ui-config",
  "version": 1,
  "modes": { "prepare": { … }, "teach": { … }, "view": { … } },
  "appearance": { "theme": "dark", "accent": "mint", "iconStyle": "linear", "toolbar": "bottom", "density": "large", "hand": "right" }
}
```

- `format` (required) is exactly `"rupok-ui-config"`. `version` (required) is
  `1`; a newer version is refused.
- `modes`, `appearance` and `gestures` are optional. A mode that is missing keeps the
  teacher's current setup.
- Readers ignore what they do not know: unknown keys, unknown tool ids,
  unknown chrome keys and invalid values are dropped, never fatal.

### A mode

```json
{
  "chrome": { "pages": false, "zoom": false },
  "tools": ["pen", "highlighter", "eraser", "laser", "undo", "pager", "exit"],
  "dock": { "position": "bottom", "orientation": "auto", "minimized": false, "pinned": ["pager", "exit"] }
}
```

- `tools` is everything the mode offers, **in order**: the tools on the dock
  and the ones in its ⋯ menu. Tool shortcuts work for these tools. A
  separator is drawn automatically between groups (ink, colours, create,
  point, history, navigation, exit). Duplicates are removed.
- `dock` says where the Smart Dock starts and which tools sit on it (below).
- `chrome` maps chrome keys to `true` (show) or `false` (hide). A key that is
  missing takes the mode's default (below).

### Dock

The drawing toolbar is the **Smart Dock**. It shows a few core tools, the
tools the mode pins, and what the moment needs: "Use object" and a step
pager while a Smart Object is on screen, replay controls during a replay,
text size while editing text. Everything else in `tools` waits in the ⋯ menu.

- **Core tools** (`pen`, `highlighter`, `eraser`, `laser`, `undo`) are
  always on the dock in `prepare` and `teach`. They cannot be removed: if
  `tools` leaves one out, it is added at the end.
- `pinned`: the other tools that sit on the dock, as tool ids, plus
  `"shapes"` for the one Shapes button (rectangle, ellipse, line, arrow and
  the Smart shapes toggle in its flyout). Pinning any shape id pins
  `"shapes"`. Pins that are not in `tools`, and core tools, are dropped.
  When `pinned` is missing but `tools` is given (a config written before the
  dock), every tool in `tools` is pinned, so the dock shows them all.
- `position`: where the dock starts. `"auto"` (follow Settings → Appearance → Dock side:
  `appearance.toolbar`), `"bottom"`, `"top"`, `"left"`, `"right"`,
  `"bottom-left"`, `"bottom-right"`, `"top-left"`, `"top-right"`, or
  `{ "x": 0..1, "y": 0..1 }` to float it (fractions of the free screen area;
  0 is flush left/top, 1 flush right/bottom). On the left and right edges it
  stands vertical.
- `orientation`: `"auto"` (vertical on the left and right edges), or
  `"horizontal"` / `"vertical"` to force one.
- `minimized`: start as the small round bubble (tap it to open the dock).

Teachers drag the dock by its grip; it snaps to the edges and corners and
remembers where it was left per mode and per screen size (in the browser,
`rupok.dock`, not in this file). Changing a mode's `dock` (here or in
Settings → Teaching modes) resets that memory, so the config takes effect.

### Tool ids

| Id | Entry | Allowed in |
|---|---|---|
| `pen`, `highlighter`, `eraser` | Ink tools | prepare, teach |
| `lasso` | Select & move | prepare, teach |
| `laser` | Laser pointer | prepare, teach |
| `text`, `rect`, `ellipse`, `line`, `arrow` | Text and shapes | prepare, teach |
| `hand` | Pan | all |
| `interact` | Use Smart Objects | all |
| `smartink` | Smart shapes on/off: switches between Off and the last on-mode (Auto by default; set in Settings → Pen & ink → Smart shapes). Key `M` | prepare, teach |
| `undo`, `redo` | Undo / redo buttons | prepare, teach |
| `replay` | Replay this page (a labelled button in `view`) | all |
| `pager` | Back / next with a page indicator (steps through Smart Object builds in `teach`) | all |
| `exit` | Leave the mode (back to where Teach was entered from) | teach |

### Chrome keys

| Key | Element | Allowed in |
|---|---|---|
| `switcher` | Mode switcher (top left) | all |
| `logo` | Rupok logo | all |
| `title` | Lesson title (read-only in `view`) | all |
| `pagesToggle` | Button that shows/hides the pages panel | all |
| `history` | Undo and redo in the top bar | prepare, teach |
| `insert` | The **Insert ▾** menu: Image, PDF, Smart Object, Text | prepare, teach |
| `replay` | Replay button in the top bar | all |
| `open` | Open file in the ⋯ menu (an "Open lesson" button on the bar in `view`) | all |
| `theme` | Light / dark: in the ⋯ menu (a button on the bar in `view`) | all |
| `fullscreen` | Fullscreen in the ⋯ menu | all |
| `settings` | Settings button | prepare, teach |
| `save` | The quiet save state next to the title ("Saved" / "Unsaved"; click to save), and Save, Download a copy, Export PDF and Share in the ⋯ menu | prepare, teach |
| `ai` | AI assistant status icon (Connect AI; a mint dot when connected, pulsing while the AI edits) | prepare, teach |
| `overflow` | The ⋯ menu: files, export, theme, fullscreen, keyboard shortcuts, help | all |
| `pages` | Pages panel (the teacher can still hide it) | all |
| `zoom` | Zoom chip. In `teach` it also turns the fit lock off: zooming then sticks instead of snapping back | all |
| `stats` | Renderer statistics, when enabled in Settings → About → Performance overlay | all |
| `colors` | The three recent colours on the dock (the full palette is a tap on the active pen away) | prepare, teach |
| `sizes` | Ink sizes in the pen, highlighter and eraser popover | prepare, teach |

The top bar is not drawn at all when every top-bar key is off (`switcher`,
`logo`, `title`, `pagesToggle`, `history`, `insert`, `replay`, `open`,
`theme`, `fullscreen`, `settings`, `save`, `ai`, `overflow`); the slide then
gets that space. In Teach (no bar) a connected AI shows as one small status
button at the top right.

How the top bar uses these keys:

- **The ⋯ menu** holds what a teacher rarely needs mid-lesson. With
  `overflow: false` the everyday ones (open, export PDF, theme, fullscreen)
  come back as icons on the bar instead.
- **Always reachable.** Theme and fullscreen are always in ⋯. When `settings`
  or `ai` is off, Settings and Connect AI move into ⋯ rather than vanishing,
  so a config cannot lock a teacher out. Keys for things a mode should not do
  (`insert`, `history`, `replay`, `open`, `save`) hide them everywhere.
- **Narrow windows.** The bar never overflows. As it runs out of room it hides
  button labels, then moves Replay, Insert, Undo/Redo, Settings, the AI
  status, the mode switcher and the pages button into ⋯, in that order (the ⋯
  button appears even with `overflow: false` once something has moved).

### Defaults

| Mode | Chrome on | Tools | Pinned on the dock |
|---|---|---|---|
| `prepare` | everything except `logo` | `pen highlighter eraser rect ellipse line arrow smartink text lasso laser hand interact undo redo` | `shapes text lasso` |
| `teach` | `colors sizes` | `pen highlighter eraser laser smartink undo pager exit` | `smartink pager exit` |
| `view` | `switcher title pagesToggle open theme fullscreen overflow pages zoom` | `pager hand interact replay` | `pager hand interact replay` |

`position` is `"auto"` in `prepare` and `teach` and `"bottom"` in `view`;
`orientation` is `"auto"` and `minimized` is `false` everywhere.

### Appearance

All optional; invalid values are ignored.

| Key | Values |
|---|---|
| `theme` | `system`, `light`, `dark` |
| `accent` | `mint`, `graphite`, `blue`, `grass`, `coral`, `sun` |
| `iconStyle` | `linear`, `line-duotone`, `broken`, `bold` |
| `toolbar` | `bottom`, `left`, `right`: where the dock starts in modes whose `dock.position` is `"auto"` |
| `density` | `compact`, `comfortable`, `large` |
| `hand` | `right`, `left`: the writing hand. Dock popovers open away from it, so the arm never covers them |

### Gestures

Classroom touch gestures for interactive boards (**Settings → Touch & gestures**).
All optional; a key that is missing keeps the teacher's current setting, and
invalid values are ignored (numbers are clamped to their range). The board
stores them in `localStorage` under `rupok.gestures`. Source of truth:
`apps/board/src/gestures.ts` (`GestureConfig`).

```json
"gestures": {
  "palmErase": true, "palmSize": 40, "eraserScale": 1, "handCluster": true, "thumbScrub": true,
  "swipe": true, "swipeFingers": 2, "flick": true, "swipeSensitivity": 0.5,
  "tapUndo": true, "tapRedo": true, "doubleTapFit": true, "longPressDock": true,
  "largeTargets": false, "hints": true
}
```

| Key | Values | Default | What it does | Button or shortcut |
|---|---|---|---|---|
| `palmErase` | `true`, `false` | `true` | Rubbing with the palm, the side of the hand or a thumb erases ink, like a board duster. Objects, images and text are never erased. One rub is one undo step. A pen touching the board ends the rub (the pen wins). | Eraser tool, `E` |
| `palmSize` | 12–200 (CSS px) | `40` | Contact diameter from which one touch is a palm, on boards that report touch size. It must also be about 1.6× the board's usual finger, so a board that reports one size for every touch never erases with a finger. "Calibrate my board" sets it. | |
| `eraserScale` | 0.5–2 | `1` | Eraser radius relative to the hand on the board. | Eraser sizes |
| `handCluster` | `true`, `false` | `true` | For boards that don't report touch size (most IR frames, Linux): 3 or more touches close together that rub erase. | |
| `thumbScrub` | `true`, `false` | `true` | One touch scrubbing fast back and forth erases, only when fingers don't draw (a pen board, the Pan tool). | |
| `swipe` | `true`, `false` | `true` | A fast horizontal swipe with `swipeFingers` fingers: left for the next slide or Smart Object step, right for the previous. Never while the pen draws. | `→` `←`, Page Down / Up, the pager |
| `swipeFingers` | `2`, `3` | `2` | Fingers of a swipe. With 3, a 3-finger rub is still a hand eraser. | |
| `flick` | `true`, `false` | `true` | In Teach and the student view, a one-finger flick also changes the slide, when fingers don't draw (a pen board, the Pan or Use objects tool). Never inside a Smart Object in use. | as `swipe` |
| `swipeSensitivity` | 0–1 | `0.5` | 0 is strict (a long, fast swipe: about 200 px at 1.2 px/ms), 1 is eager (about 60 px at 0.3 px/ms). The swipe must stay within 30° of horizontal. | |
| `tapUndo` | `true`, `false` | `true` | Two-finger tap: undo. With `doubleTapFit` on it waits 350 ms for a second tap. | Undo, `Ctrl Z` |
| `tapRedo` | `true`, `false` | `true` | Three-finger tap: redo. | Redo, `Ctrl Shift Z` |
| `doubleTapFit` | `true`, `false` | `true` | Two-finger double-tap: fit the slide to the screen. | `0` |
| `longPressDock` | `true`, `false` | `true` | Two fingers held still for 0.65 s: the Smart Dock comes next to them. | Drag the dock |
| `largeTargets` | `true`, `false` | `false` | Bigger buttons, switches, sliders and selection handles. | Settings |
| `hints` | `true`, `false` | `true` | A short hint the first time each gesture fires, with a link to turn it off. | |
| `calibration` | object or `null` | `null` | Written by "Calibrate my board": `finger`, `palm` (px), `contacts`, `spread` (px), `reportsSize`, `at`. With `reportsSize: false` contact size is ignored and hand clusters erase. Exports leave it out (it belongs to one board). | |

Example: a pen-only IR board in a large room, with three-finger swipes and
nothing that could surprise a class:

```json
"gestures": { "swipeFingers": 3, "swipeSensitivity": 0.3, "thumbScrub": false, "tapRedo": false, "largeTargets": true }
```

## Example: a large classroom panel

A Teach mode for an 86-inch panel: big controls, the dock on the left where
a teacher can reach it, select and redo pinned, and the zoom chip kept. The
student view gets no pages panel.

```json
{
  "format": "rupok-ui-config",
  "version": 1,
  "modes": {
    "teach": {
      "chrome": { "colors": true, "sizes": true, "zoom": true },
      "tools": ["pen", "highlighter", "eraser", "lasso", "laser", "undo", "redo", "pager", "exit"],
      "dock": { "position": "left", "pinned": ["lasso", "redo", "pager", "exit"] }
    },
    "view": {
      "chrome": { "pages": false, "pagesToggle": false },
      "tools": ["pager", "replay"]
    }
  },
  "appearance": { "density": "large", "hand": "left" }
}
```

## JSON Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Rupok UI config",
  "type": "object",
  "required": ["format", "version"],
  "properties": {
    "format": { "const": "rupok-ui-config" },
    "version": { "type": "integer", "minimum": 1, "maximum": 1 },
    "modes": {
      "type": "object",
      "properties": {
        "prepare": { "$ref": "#/$defs/mode" },
        "teach": { "$ref": "#/$defs/mode" },
        "view": { "$ref": "#/$defs/mode" }
      }
    },
    "appearance": {
      "type": "object",
      "properties": {
        "theme": { "enum": ["system", "light", "dark"] },
        "accent": { "enum": ["mint", "graphite", "blue", "grass", "coral", "sun"] },
        "iconStyle": { "enum": ["linear", "line-duotone", "broken", "bold"] },
        "toolbar": { "enum": ["bottom", "left", "right"] },
        "density": { "enum": ["compact", "comfortable", "large"] },
        "hand": { "enum": ["right", "left"] }
      }
    },
    "gestures": {
      "type": "object",
      "properties": {
        "palmErase": { "type": "boolean" },
        "palmSize": { "type": "number", "minimum": 12, "maximum": 200 },
        "eraserScale": { "type": "number", "minimum": 0.5, "maximum": 2 },
        "handCluster": { "type": "boolean" },
        "thumbScrub": { "type": "boolean" },
        "swipe": { "type": "boolean" },
        "swipeFingers": { "enum": [2, 3] },
        "flick": { "type": "boolean" },
        "swipeSensitivity": { "type": "number", "minimum": 0, "maximum": 1 },
        "tapUndo": { "type": "boolean" },
        "tapRedo": { "type": "boolean" },
        "doubleTapFit": { "type": "boolean" },
        "longPressDock": { "type": "boolean" },
        "largeTargets": { "type": "boolean" },
        "hints": { "type": "boolean" },
        "calibration": { "type": ["object", "null"] }
      }
    }
  },
  "$defs": {
    "mode": {
      "type": "object",
      "properties": {
        "chrome": {
          "type": "object",
          "propertyNames": {
            "enum": ["switcher", "logo", "title", "pagesToggle", "history", "insert", "replay", "open", "theme",
                     "fullscreen", "settings", "save", "ai", "overflow", "pages", "zoom", "stats", "colors", "sizes"]
          },
          "additionalProperties": { "type": "boolean" }
        },
        "tools": {
          "type": "array",
          "uniqueItems": true,
          "items": {
            "enum": ["pen", "highlighter", "eraser", "lasso", "laser", "text", "rect", "ellipse", "line", "arrow",
                     "hand", "interact", "smartink", "undo", "redo", "replay", "pager", "exit"]
          }
        },
        "dock": {
          "type": "object",
          "properties": {
            "position": {
              "oneOf": [
                { "enum": ["auto", "bottom", "top", "left", "right", "bottom-left", "bottom-right", "top-left", "top-right"] },
                {
                  "type": "object",
                  "required": ["x", "y"],
                  "properties": { "x": { "type": "number", "minimum": 0, "maximum": 1 }, "y": { "type": "number", "minimum": 0, "maximum": 1 } }
                }
              ]
            },
            "orientation": { "enum": ["auto", "horizontal", "vertical"] },
            "minimized": { "type": "boolean" },
            "pinned": {
              "type": "array",
              "uniqueItems": true,
              "items": {
                "enum": ["shapes", "lasso", "text", "rect", "ellipse", "line", "arrow", "hand", "interact", "smartink",
                         "redo", "replay", "pager", "exit"]
              }
            }
          }
        }
      }
    }
  }
}
```

The schema accepts every id everywhere; the board then drops what a mode does
not allow (see the "Allowed in" columns), for example ink tools in `view`.
