// Website (landing page) bootstrap: fonts, styles and Solar icons.
import "@fontsource-variable/figtree";
import "@fontsource-variable/noto-sans-bengali";
import "./site.css";
import { ICONS, type IconName } from "./icons.generated";

for (const el of document.querySelectorAll<HTMLElement>("[data-icon]")) {
  const name = el.dataset.icon as IconName;
  const body = ICONS[name]?.["line-duotone"];
  if (body) el.innerHTML = `<svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">${body}</svg>`;
}
