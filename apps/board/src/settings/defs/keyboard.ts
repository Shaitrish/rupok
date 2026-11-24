// Settings → Keyboard shortcuts (read-only for now). Each shortcut is its own
// entry, so searching "eraser" or "undo" finds its key. Presentation
// clickers send the arrow keys or Page Up / Down, so they work as is.

import { type KeySetting, registerSettings } from "../registry";

type K = [id: string, label: string, keys: string[], keywords?: string[]];
const groups: [group: string, base: number, items: K[]][] = [
  ["Tools", 100, [
    ["pen", "Pen", ["P"], ["কলম"]],
    ["highlighter", "Highlighter", ["H"]],
    ["eraser", "Eraser", ["E"], ["ইরেজার", "মুছুন"]],
    ["select", "Select / lasso", ["V"]],
    ["laser", "Laser pointer", ["L"]],
    ["text", "Text", ["T"]],
    ["shapePicker", "Shapes and tables (picker)", ["S"], ["shapes", "table", "insert", "আকৃতি", "টেবিল"]],
    ["shapes", "Rectangle, ellipse, line, arrow", ["R", "O", "N", "A"], ["shapes"]],
    ["shapeSquare", "Square, circle, 15° lines (while drawing a shape)", ["Shift"], ["shapes", "constrain"]],
    ["smartShapes", "Smart shapes on / off", ["M"], ["smart ink"]],
    ["interact", "Use objects", ["I"], ["smart object", "interact"]],
    ["pan", "Pan (hold)", ["Space"], ["hand", "move"]],
  ]],
  ["Slides and view", 200, [
    ["next", "Next step or slide", ["→", "Page Down"], ["clicker", "forward"]],
    ["prev", "Previous step or slide", ["←", "Page Up"], ["clicker", "back"]],
    ["fit", "Fit the slide", ["0"], ["zoom"]],
    ["zoom", "Zoom in / out", ["+", "−"]],
    ["fullscreen", "Full screen", ["F"]],
    ["teach", "Teach (present)", ["F5"], ["present", "slideshow"]],
    ["modes", "Prepare, Teach, Student view", ["Alt 1", "Alt 2", "Alt 3"], ["mode"]],
  ]],
  ["Editing", 300, [
    ["undo", "Undo", ["Ctrl Z"], ["আনডু"]],
    ["redo", "Redo", ["Ctrl Y", "Ctrl ⇧ Z"]],
    ["copy", "Copy / paste", ["Ctrl C", "Ctrl V"]],
    ["duplicate", "Duplicate", ["Ctrl D"]],
    ["selectAll", "Select everything", ["Ctrl A"]],
    ["delete", "Delete the selection", ["Delete"]],
    ["group", "Group / ungroup", ["Ctrl G", "Ctrl ⇧ G"]],
    ["merge", "Merge ink", ["Ctrl E"]],
  ]],
  ["Files", 400, [
    ["save", "Save", ["Ctrl S"]],
    ["open", "Open", ["Ctrl O"]],
    ["exportPdf", "Export PDF", ["Ctrl ⇧ E"], ["pdf"]],
  ]],
];

registerSettings(
  groups.flatMap(([group, base, items]) =>
    items.map(([id, label, keys, keywords], i): KeySetting => ({
      id: `keyboard.${id}`, category: "keyboard", group, order: base + i, type: "key", label, keys,
      keywords: ["shortcut", "key", "hotkey", ...(keywords ?? [])],
    })),
  ),
);
