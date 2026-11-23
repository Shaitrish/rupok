import { ICONS, type IconName, type IconVariant } from "../icons.generated";
import { prefs } from "../state";
import { APP_NAME } from "../brand";

/** Solar icon. Active tools render the bold-duotone weight for a clear "on" state. */
export function Icon(props: { name: IconName; size?: number; active?: boolean; variant?: IconVariant }) {
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
