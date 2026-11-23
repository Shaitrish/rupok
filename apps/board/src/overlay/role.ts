// Which overlay page this is, from the URL (kept tiny: App.tsx imports it).
//   /board/?overlay          the ink layer (desktop overlay window, or the
//                            whole overlay with an inline toolbar in a browser)
//   /board/?overlay=toolbar  the desktop app's floating toolbar window

export type OverlayRole = "ink" | "toolbar";

export const overlayRole: OverlayRole | null = (() => {
  const q = new URLSearchParams(location.search);
  if (!q.has("overlay")) return null;
  return q.get("overlay") === "toolbar" ? "toolbar" : "ink";
})();
