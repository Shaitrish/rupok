// Settings → Appearance.

import { ACCENTS } from "../../state";
import { layout } from "../../layout";
import { setLayout } from "../../components/DockEditor";
import { pref } from "../bind";
import { registerSettings } from "../registry";

registerSettings([
  {
    id: "appearance.layout", category: "appearance", order: 5, type: "segmented", label: "Layout",
    description: "Modern: a top bar and one dock. Classic: big buttons in three islands along the bottom and a ☰ menu.",
    keywords: ["classic", "modern", "islands", "big icons", "toolbar", "note3", "familiar", "লেআউট", "ক্লাসিক"],
    options: [{ value: "modern", label: "Modern" }, { value: "classic", label: "Classic" }],
    get: layout,
    set: (v: string) => setLayout(v === "classic" ? "classic" : "modern"),
    default: "modern",
  },
  {
    id: "appearance.islandSwap", category: "appearance", order: 6, type: "toggle", label: "Menu island on the right",
    description: "Classic layout: the menu island and the page island trade sides. The arrow tab at each side island's edge does the same.",
    keywords: ["swap", "islands", "left", "right", "mirror", "classic", "ক্লাসিক"],
    when: () => layout() === "classic",
    ...pref("islandSwap"),
  },
  {
    id: "appearance.theme", category: "appearance", order: 10, type: "segmented", label: "Theme",
    description: "Light, dark, or follow this device.",
    keywords: ["dark mode", "light mode", "night", "থিম", "অন্ধকার", "ডার্ক", "আলো"],
    options: [{ value: "system", label: "System" }, { value: "light", label: "Light" }, { value: "dark", label: "Dark" }],
    ...pref("theme"),
  },
  {
    id: "appearance.accent", category: "appearance", order: 20, type: "colour", label: "Accent colour",
    description: "Marks the selected tool and the main button.",
    keywords: ["color", "highlight colour", "রং", "অ্যাকসেন্ট"],
    options: ACCENTS.map((a) => ({ value: a.id, label: a.label, color: a.color })),
    ...pref("accent"),
  },
  {
    id: "appearance.density", category: "appearance", order: 30, type: "segmented", label: "Button size",
    description: "Large board makes every button bigger for touch panels.",
    keywords: ["size", "density", "compact", "big", "touch targets", "smartboard", "আকার", "বড়"],
    options: [{ value: "compact", label: "Compact" }, { value: "comfortable", label: "Comfortable" }, { value: "large", label: "Large board" }],
    ...pref("density"),
  },
  {
    id: "appearance.dockSide", category: "appearance", order: 40, type: "segmented", label: "Dock side",
    description: "Where the Smart Dock sits when a mode does not pin it.",
    keywords: ["toolbar", "position", "left", "right", "bottom", "ডক", "টুলবার"],
    options: [{ value: "bottom", label: "Bottom" }, { value: "left", label: "Left" }, { value: "right", label: "Right" }],
    ...pref("toolbar"),
  },
  {
    id: "appearance.iconStyle", category: "appearance", order: 50, type: "segmented", label: "Icon style",
    description: "The weight of the icons in the toolbar and menus.",
    keywords: ["icons", "আইকন", "bold", "duotone"],
    options: [{ value: "linear", label: "Linear" }, { value: "line-duotone", label: "Duotone" }, { value: "broken", label: "Broken" }, { value: "bold", label: "Bold" }],
    ...pref("iconStyle"),
  },
  {
    id: "appearance.pagesPanel", category: "appearance", order: 60, type: "toggle", label: "Pages panel",
    description: "Show the page thumbnails at the side while you prepare.",
    keywords: ["thumbnails", "slides list", "sidebar", "পৃষ্ঠা"],
    ...pref("showPages"),
  },
]);
