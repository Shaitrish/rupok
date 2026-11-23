# UI config (modes, toolbar, appearance)

A **UI config** is a small JSON file that tells the Rupok board which chrome
and which tools to show in each mode. A teacher imports it in
**Settings → Modes → Import UI config**; **Export UI config** writes the
current setup in the same format. The format is meant to be written by people
and by AI agents ("make me a Teach mode with only a red and a blue pen and no
undo"). Later the desktop app and the MCP server will be able to write it
directly.

The board stores the same `modes` object in the browser's `localStorage`
under `rupok.prefs` → `modes`. Source of truth: `apps/board/src/modes.ts`.

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
  "appearance": { "theme": "dark", "accent": "mint", "iconStyle": "linear", "toolbar": "bottom", "density": "large" }
}
```

- `format` (required) is exactly `"rupok-ui-config"`. `version` (required) is
  `1`; a newer version is refused.
- `modes` and `appearance` are optional. A mode that is missing keeps the
  teacher's current setup.
- Readers ignore what they do not know: unknown keys, unknown tool ids,
  unknown chrome keys and invalid values are dropped, never fatal.

### A mode

```json
{ "chrome": { "pages": false, "zoom": false }, "tools": ["pen", "highlighter", "eraser", "undo", "pager", "exit"] }
```

- `tools` is the toolbar **in order**. A separator is drawn automatically
  wherever the kind of entry changes (tools, actions, navigation, exit).
  Duplicates are removed.
- `chrome` maps chrome keys to `true` (show) or `false` (hide). A key that is
  missing takes the mode's default (below).

### Tool ids

| Id | Entry | Allowed in |
|---|---|---|
| `pen`, `highlighter`, `eraser` | Ink tools | prepare, teach |
| `lasso` | Select & move | prepare, teach |
| `laser` | Laser pointer | prepare, teach |
| `text`, `rect`, `ellipse`, `line`, `arrow` | Text and shapes | prepare, teach |
| `hand` | Pan | all |
| `interact` | Use Smart Objects | all |
| `smartink` | Smart shapes on/off: switches between Off and the last on-mode (Auto by default; set in Settings → Smart ink). Key `M` | prepare, teach |
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
| `insert` | PDF, Image and Smart Object buttons | prepare, teach |
| `replay` | Replay button in the top bar | all |
| `open` | Open (a primary "Open lesson" button in `view`) | all |
| `theme` | Light / dark toggle | all |
| `fullscreen` | Fullscreen button | all |
| `settings` | Settings | prepare, teach |
| `save` | Save | prepare, teach |
| `pages` | Pages panel (the teacher can still hide it) | all |
| `zoom` | Zoom chip. In `teach` it also turns the fit lock off: zooming then sticks instead of snapping back | all |
| `stats` | Renderer statistics, when enabled in Settings | all |
| `colors` | Ink colour swatches, while an ink tool is active | prepare, teach |
| `sizes` | Ink size buttons, while an ink tool or the eraser is active | prepare, teach |

The top bar is not drawn at all when every top-bar key is off (`switcher`,
`logo`, `title`, `pagesToggle`, `history`, `insert`, `replay`, `open`,
`theme`, `fullscreen`, `settings`, `save`); the slide then gets that space.

### Defaults

| Mode | Chrome on | Tools |
|---|---|---|
| `prepare` | everything except `logo` | `pen highlighter eraser lasso laser text rect ellipse line arrow hand interact smartink` |
| `teach` | `colors` | `pen highlighter eraser laser smartink undo pager exit` |
| `view` | `switcher title pagesToggle open theme fullscreen pages zoom` | `pager hand interact replay` |

### Appearance

All optional; invalid values are ignored.

| Key | Values |
|---|---|
| `theme` | `system`, `light`, `dark` |
| `accent` | `mint`, `graphite`, `blue`, `grass`, `coral`, `sun` |
| `iconStyle` | `linear`, `line-duotone`, `broken`, `bold` |
| `toolbar` | `bottom`, `left`, `right` |
| `density` | `compact`, `comfortable`, `large` |

## Example: a large classroom panel

A Teach mode for an 86-inch panel: big controls, the toolbar on the left
where a teacher can reach it, select and redo added, and the zoom chip kept.
The student view gets no pages panel.

```json
{
  "format": "rupok-ui-config",
  "version": 1,
  "modes": {
    "teach": {
      "chrome": { "colors": true, "sizes": true, "zoom": true },
      "tools": ["pen", "highlighter", "eraser", "lasso", "laser", "undo", "redo", "pager", "exit"]
    },
    "view": {
      "chrome": { "pages": false, "pagesToggle": false },
      "tools": ["pager", "replay"]
    }
  },
  "appearance": { "toolbar": "left", "density": "large" }
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
        "density": { "enum": ["compact", "comfortable", "large"] }
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
                     "fullscreen", "settings", "save", "pages", "zoom", "stats", "colors", "sizes"]
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
        }
      }
    }
  }
}
```

The schema accepts every id everywhere; the board then drops what a mode does
not allow (see the "Allowed in" columns), for example ink tools in `view`.
