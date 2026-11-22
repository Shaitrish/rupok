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

/** The Rupok mark: a lowercase "r" drawn in one stroke (matter) with an
 * idea spark above it (mind). Source of truth: /brand/rupok-mark.svg. */
export function Mark(props: { size?: number }) {
  return (
    <svg viewBox="0 0 64 64" width={props.size ?? 28} height={props.size ?? 28} aria-label={APP_NAME} role="img">
      <defs>
        <linearGradient id="rp-bg" x1="8" y1="4" x2="56" y2="60" gradientUnits="userSpaceOnUse">
          <stop offset="0" stop-color="#7B76F0" />
          <stop offset="1" stop-color="#4440C0" />
        </linearGradient>
        <linearGradient id="rp-sheen" x1="32" y1="4" x2="32" y2="34" gradientUnits="userSpaceOnUse">
          <stop offset="0" stop-color="#fff" stop-opacity=".2" />
          <stop offset="1" stop-color="#fff" stop-opacity="0" />
        </linearGradient>
      </defs>
      <rect x="4" y="4" width="56" height="56" rx="17" fill="url(#rp-bg)" />
      <rect x="4" y="4" width="56" height="56" rx="17" fill="url(#rp-sheen)" />
      <path d="M19 47.5V32c0-6.6 5.4-10.3 11.8-9.8" fill="none" stroke="#fff" stroke-width="6" stroke-linecap="round" stroke-linejoin="round" />
      <path d="M42 12.5C42.9 17.3 44.2 18.6 48.5 19C44.2 19.4 42.9 20.7 42 25.5C41.1 20.7 39.8 19.4 35.5 19C39.8 18.6 41.1 17.3 42 12.5Z" fill="#FFD36E" />
    </svg>
  );
}
