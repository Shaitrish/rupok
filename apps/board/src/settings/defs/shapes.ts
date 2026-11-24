// Settings → Teaching modes → Shapes: what happens after a shape is placed
// (the Shapes picker, shapes.ts), per mode.

import { type AfterPlace, shapeStore } from "../../shapes";
import { registerSettings } from "../registry";

const options = [
  { value: "keep", label: "Keep drawing" },
  { value: "select", label: "Select it" },
  { value: "pen", label: "Back to pen" },
];

registerSettings([
  {
    id: "modes.shapeAfterTeach", category: "modes", group: "Shapes", order: 300, type: "segmented",
    label: "After a shape in Teach",
    description: "Keep the shape tool to place more, select the new shape to resize or fill it, or go back to the pen.",
    keywords: ["shape", "table", "insert", "tool", "আকৃতি", "টেবিল"],
    options,
    get: () => shapeStore.read().afterTeach,
    set: (v) => shapeStore.write({ afterTeach: v as AfterPlace }),
    default: shapeStore.defaults.afterTeach,
  },
  {
    id: "modes.shapeAfterPrepare", category: "modes", group: "Shapes", order: 310, type: "segmented",
    label: "After a shape in Prepare",
    description: "Keep the shape tool to place more, select the new shape to resize or fill it, or go back to the pen.",
    keywords: ["shape", "table", "insert", "tool", "আকৃতি", "টেবিল"],
    options,
    get: () => shapeStore.read().afterPrepare,
    set: (v) => shapeStore.write({ afterPrepare: v as AfterPlace }),
    default: shapeStore.defaults.afterPrepare,
  },
]);
