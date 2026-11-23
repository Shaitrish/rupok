// Settings → Teaching modes: the per-mode editor (chrome, tools, Smart Dock)
// with UI config import and export (components/ModesSettings.tsx).

import { controller } from "../../controller";
import { mode, setPrefs } from "../../state";
import { ModesSettings } from "../../components/ModesSettings";
import { registerSettings } from "../registry";

registerSettings([
  {
    id: "modes.editor", category: "modes", order: 10, type: "section", label: "Mode editor",
    description: "Pick a mode, then choose what it shows, its tools in order, and where the Smart Dock starts.",
    keywords: [
      "Prepare", "Teach", "Student view", "toolbar", "tools", "dock", "Smart Dock", "pin", "minimised", "orientation",
      "import UI config", "export UI config", "json", "chrome", "hide", "show", "ডক", "টুলবার", "মোড",
    ],
    component: ModesSettings,
    reset: () => {
      setPrefs({ modes: {} });
      controller.updateInsets(mode() === "teach");
    },
  },
]);
