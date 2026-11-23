// Settings → About: version and licences, then troubleshooting switches.

import { setPrefs, prefs } from "../../state";
import { AboutSection, GraphicsInfo } from "../../components/settings/About";
import { pref } from "../bind";
import { registerSettings } from "../registry";

registerSettings([
  {
    id: "about.info", category: "about", order: 10, type: "section", label: "About Rupok",
    description: "Version, licences and links.",
    keywords: ["version", "licence", "license", "AGPL", "open source", "credits", "fonts", "github"],
    component: AboutSection,
  },
  {
    id: "about.renderer", category: "about", group: "Troubleshooting", order: 100, type: "segmented", label: "Renderer",
    description: "Try WebGPU on new devices. Switching reloads the page; your lesson is autosaved.",
    keywords: ["webgl", "webgpu", "gpu", "graphics", "slow", "blank board"],
    options: [{ value: "webgl", label: "WebGL2" }, { value: "webgpu", label: "WebGPU · beta" }],
    get: () => (prefs().forceWebgl ? "webgl" : "webgpu"),
    set: (v) => {
      setPrefs({ forceWebgl: v === "webgl" });
      location.reload();
    },
    default: "webgl",
  },
  {
    id: "about.stats", category: "about", group: "Troubleshooting", order: 110, type: "toggle", label: "Performance and pen latency overlay",
    description: "Live pen-to-frame time, input rate, frames per second and the graphics card, in a corner of the board. Write for a few seconds, then read it.",
    keywords: ["statistics", "fps", "stats", "debug", "latency", "lag", "laggy", "delay", "slow pen", "smooth", "ink"],
    ...pref("showStats"),
  },
  {
    id: "about.graphics", category: "about", group: "Troubleshooting", order: 120, type: "custom", layout: "block", label: "Graphics",
    description: "The graphics card the board draws with. “Software” means no GPU: ink will lag; update the graphics driver.",
    keywords: ["gpu", "graphics card", "adapter", "driver", "software rendering", "swiftshader", "basic render"],
    component: GraphicsInfo,
  },
]);
