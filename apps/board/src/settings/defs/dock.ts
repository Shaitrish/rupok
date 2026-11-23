// Settings → Teaching modes → Smart Dock. Per-mode dock options (position,
// orientation, start minimised, pinned tools) live in the mode editor
// (modes.editor, components/ModesSettings.tsx); the writing hand is pen.hand
// and the starting side appearance.toolbar.

import { resetDockAll } from "../../dock";
import { showToast } from "../../state";
import { registerSettings } from "../registry";

registerSettings([
  {
    id: "modes.dockReset", category: "modes", group: "Smart Dock", order: 200, type: "action",
    label: "Reset dock positions",
    description: "Puts the dock back where each mode starts, on every screen size, and opens it if it was minimised.",
    keywords: ["dock", "toolbar", "position", "move", "drag", "lost", "minimised", "bubble", "ডক", "টুলবার"],
    button: "Reset", icon: "restart",
    run: () => {
      resetDockAll();
      showToast("Dock positions reset");
    },
  },
]);
