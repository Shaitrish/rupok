import { createContext, useContext } from "solid-js";
import { DRAWN, type DrawnName, ICONS, type IconName, type IconVariant } from "../icons.generated";
import { prefs } from "../state";
import { APP_NAME } from "../brand";

/** "classic": icons inside the Classic islands use our own bolder set
 * (icons/drawn/ci-*.svg), drawn to read from the back of a classroom. */
export const IconSet = createContext<"solar" | "classic">("solar");

/** Solar names → our Classic drawings. */
const CLASSIC: Partial<Record<IconName, DrawnName>> = {
  menu: "ci-menu", monitor: "ci-screen", toolbox: "ci-toolbox", share: "ci-share", search: "ci-search", select: "ci-select",
  pen: "ci-pen", eraser: "ci-eraser", rect: "ci-shapes", hand: "ci-hand", recognize: "ci-recognize", stamp: "ci-stamp",
  ai: "ci-ai", undo: "ci-undo", redo: "ci-redo", plus: "ci-addpage", prev: "ci-prev", next: "ci-next",
};

/** Solar icon. Active tools render the bold-duotone weight for a clear "on" state. */
export function Icon(props: { name: IconName; size?: number; active?: boolean; variant?: IconVariant }) {
  const set = useContext(IconSet);
  const drawn = () => (set === "classic" ? CLASSIC[props.name] : undefined);
  if (drawn()) {
    return (
      <svg
        class="icon ci"
        classList={{ on: !!props.active }}
        viewBox="0 0 24 24"
        width={props.size ?? 20}
        height={props.size ?? 20}
        aria-hidden="true"
        innerHTML={DRAWN[drawn()!].body}
      />
    );
  }
  const variant = (): IconVariant => {
    if (props.variant) return props.variant;
    const base = prefs().iconStyle;
    return props.active && (base === "linear" || base === "line-duotone" || base === "broken") ? "bold-duotone" : base;
  };
  return (
    <svg
      class="icon"
      viewBox="0 0 24 24"
      width={props.size ?? 20}
      height={props.size ?? 20}
      aria-hidden="true"
      innerHTML={ICONS[props.name][variant()]}
    />
  );
}

/** The Rupok mark, "Split": a rounded square cut on the diagonal with the
 * mint half slid away (form changing). The ink half follows the text
 * colour so it works on light and dark. Source of truth: /brand/rupok-mark.svg. */
export function Mark(props: { size?: number }) {
  return (
    <svg viewBox="0 0 64 64" width={props.size ?? 28} height={props.size ?? 28} aria-label={APP_NAME} role="img" class="rp-mark">
      <g stroke-width="4" stroke-linejoin="round">
        <path d="M12 18A6 6 0 0 1 18 12H44L12 44Z" fill="currentColor" stroke="currentColor" />
        <path d="M52 20V46A6 6 0 0 1 46 52H20Z" fill="var(--brand-mint, #20C997)" stroke="var(--brand-mint, #20C997)" />
      </g>
    </svg>
  );
}
