import type { IconVariant } from "../icons.generated";
import { LIB_ICONS, type LibIconName } from "../library/icons.generated";
import { prefs } from "../state";

/** A Library icon (Solar), in the teacher's icon style like the board's icons. */
export function LIcon(props: { name: LibIconName; size?: number; active?: boolean }) {
  const variant = (): IconVariant => {
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
      innerHTML={LIB_ICONS[props.name][variant()]}
    />
  );
}

/** "Just now", "5 min ago", "Yesterday", "3 Nov". */
export function ago(ms: number): string {
  if (!ms) return "";
  const s = (Date.now() - ms) / 1000;
  if (s < 60) return "Just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  if (s < 172800) return "Yesterday";
  const d = new Date(ms);
  return d.toLocaleDateString(undefined, { day: "numeric", month: "short", ...(d.getFullYear() !== new Date().getFullYear() ? { year: "numeric" } : {}) });
}

/** 1.2 MB */
export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(0)} KB`;
  if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(n < 10 * 1024 ** 2 ? 1 : 0)} MB`;
  return `${(n / 1024 ** 3).toFixed(1)} GB`;
}
