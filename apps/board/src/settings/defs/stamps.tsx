// Settings → Pen & ink: pen styles and classroom stamps.

import { For } from "solid-js";
import { BRUSHES, TOOL_BRUSHES } from "../../brushes";
import { STAMPS } from "../../stamps";
import { myStamps } from "../../stampLib";
import { PREF_DEFAULTS, prefs, setPrefs } from "../../state";
import { StampIcon } from "../../components/Stamps";
import { pref } from "../bind";
import { type SettingProps, registerSettings } from "../registry";

/** Toggle chips: which stamps the long-press quick menu holds. */
function Favorites(p: SettingProps) {
  const favs = () => prefs().stampFavorites;
  return (
    <div class="stamp-grid settings-stamps" role="group" aria-labelledby={p.labelId} aria-describedby={p.descId}>
      <For each={[...STAMPS.map((s) => ({ id: s.id, label: s.label, wide: s.render === "label" })), ...myStamps().map((s) => ({ id: s.id, label: s.name, wide: s.art === "text" }))]}>
        {(s) => {
          const on = () => favs().includes(s.id);
          return (
            <button
              class="tool stamp-cell"
              classList={{ on: on(), wide: s.wide }}
              aria-pressed={on()}
              aria-label={s.label}
              title={s.label}
              onClick={() => setPrefs({ stampFavorites: on() ? favs().filter((f) => f !== s.id) : [...favs(), s.id].slice(-8) })}
            >
              <StampIcon id={s.id} />
            </button>
          );
        }}
      </For>
    </div>
  );
}

registerSettings([
  {
    id: "pen.style", category: "pen", order: 5, type: "select", label: "Pen style",
    description: "How the pen draws. The dock's pen menu has the same styles with a preview; each tool remembers its own.",
    keywords: ["brush", "ballpoint", "fountain", "calligraphy", "marker", "pencil", "chalk", "dashed", "dotted", "neon", "rainbow", "magic ink", "কলম", "তুলি"],
    options: TOOL_BRUSHES.pen.map((id) => ({ value: id, label: BRUSHES.find((b) => b.id === id)!.label })),
    ...pref("penBrush"),
  },
  {
    id: "pen.markerOpacity", category: "pen", order: 6, type: "slider", label: "Marker opacity",
    description: "How see-through marker strokes are. Each stroke keeps the opacity it was drawn with.",
    keywords: ["transparency", "translucent", "marker", "স্বচ্ছ"],
    min: 0.3, max: 1, step: 0.05, format: (v) => `${Math.round(v * 100)}%`,
    ...pref("markerOpacity"),
  },
  {
    id: "pen.stampQuickMenu", category: "pen", group: "Stamps", order: 300, type: "toggle", label: "Hold the pen for stamps",
    description: "Hold the pen still on the board for half a second: your favourite stamps appear around it, with Celebrate in the middle.",
    keywords: ["stamp", "sticker", "star", "tick", "radial", "long press", "quick menu", "স্ট্যাম্প", "তারা"],
    ...pref("stampQuickMenu"),
  },
  {
    id: "pen.stampSounds", category: "pen", group: "Stamps", order: 305, type: "toggle", label: "Stamp sounds",
    description: "Custom stamps may play their short sound (a chime, a clap…) when placed. Off: silent.",
    keywords: ["mute", "sound", "audio", "chime", "শব্দ"],
    ...pref("stampSounds"),
  },
  {
    id: "pen.stampFavorites", category: "pen", group: "Stamps", order: 310, type: "custom", layout: "block",
    label: "Quick menu stamps",
    description: "Up to eight. You can also hold a stamp in the dock's Stamps menu to add or remove it.",
    keywords: ["favourites", "favorites", "stamp", "sticker", "প্রিয়"],
    component: Favorites,
    reset: () => setPrefs({ stampFavorites: [...PREF_DEFAULTS.stampFavorites] }),
  },
]);
