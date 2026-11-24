// Small line drawings of the Shapes picker's shapes, in the style of the
// app's Solar icons (24 × 24, 1.5 px strokes in currentColor).

import { Show } from "solid-js";
import { parseTable } from "../shapes";

const star = (() => {
  const pts: string[] = [];
  for (let i = 0; i < 10; i++) {
    const a = -Math.PI / 2 + (i * Math.PI) / 5;
    const r = i % 2 ? 3.9 : 9.2;
    pts.push(`${(12 + r * Math.cos(a)).toFixed(2)} ${(12.8 + r * Math.sin(a)).toFixed(2)}`);
  }
  return `M${pts.join(" L")} Z`;
})();

const pentagon = (() => {
  const pts: string[] = [];
  for (let i = 0; i < 5; i++) {
    const a = -Math.PI / 2 + (i * 2 * Math.PI) / 5;
    pts.push(`${(12 + 9 * Math.cos(a)).toFixed(2)} ${(12.8 + 9 * Math.sin(a)).toFixed(2)}`);
  }
  return `M${pts.join(" L")} Z`;
})();

/** Path data per shape id (closed shapes end in Z, so they can show a fill). */
const PATHS: Record<string, string> = {
  rect: "M3.5 6.5 H20.5 V17.5 H3.5 Z",
  rrect: "M7.5 6.5 H16.5 A4 4 0 0 1 20.5 10.5 V13.5 A4 4 0 0 1 16.5 17.5 H7.5 A4 4 0 0 1 3.5 13.5 V10.5 A4 4 0 0 1 7.5 6.5 Z",
  square: "M5 5 H19 V19 H5 Z",
  ellipse: "M3 12 A9 6 0 1 0 21 12 A9 6 0 1 0 3 12 Z",
  circle: "M4 12 A8 8 0 1 0 20 12 A8 8 0 1 0 4 12 Z",
  triangle: "M12 4 L20.5 19 H3.5 Z",
  "right-triangle": "M5 4 V19 H20 Z",
  diamond: "M12 3 L21 12 L12 21 L3 12 Z",
  pentagon,
  hexagon: "M7.5 4.5 H16.5 L21 12 L16.5 19.5 H7.5 L3 12 Z",
  star,
  parallelogram: "M7.5 6.5 H21 L16.5 17.5 H3 Z",
  trapezoid: "M7.5 6.5 H16.5 L21 17.5 H3 Z",
  line: "M4.5 19.5 L19.5 4.5",
  arrow: "M4.5 19.5 L19.5 4.5 M19.5 4.5 H12.5 M19.5 4.5 V11.5",
  "double-arrow": "M3.5 12 H20.5 M3.5 12 L7.5 8 M3.5 12 L7.5 16 M20.5 12 L16.5 8 M20.5 12 L16.5 16",
  "dashed-line": "M4.5 19.5 L19.5 4.5",
  "curved-arrow": "M3.5 18 Q12 1.5 20.5 18 M20.5 18 L20.6 11.8 M20.5 18 L15.3 14.9",
  connector: "M2.5 4.5 H9.5 V9.5 H2.5 Z M14.5 14.5 H21.5 V19.5 H14.5 Z M9.5 7 H12 V17 H14.5 M14.5 17 L12.5 15 M14.5 17 L12.5 19",
  table: "M3.5 5 H20.5 V19 H3.5 Z M3.5 9.7 H20.5 M3.5 14.3 H20.5 M9.2 5 V19 M14.8 5 V19",
  numberline: "M2.5 12 H21.5 M6.5 9.5 V14.5 M12 9.5 V14.5 M17.5 9.5 V14.5 M2.5 12 L5 10 M2.5 12 L5 14 M21.5 12 L19 10 M21.5 12 L19 14",
  axes: "M12 21 V3 M3 12 H21 M12 3 L10 5.5 M12 3 L14 5.5 M21 12 L18.5 10 M21 12 L18.5 14 M7.5 11 V13 M16 11 V13 M11 7.5 H13 M11 16.5 H13",
  venn2: "M3 12 A6 6 0 1 0 15 12 A6 6 0 1 0 3 12 Z M9 12 A6 6 0 1 0 21 12 A6 6 0 1 0 9 12 Z",
  venn3: "M4 9.5 A5.5 5.5 0 1 0 15 9.5 A5.5 5.5 0 1 0 4 9.5 Z M9 9.5 A5.5 5.5 0 1 0 20 9.5 A5.5 5.5 0 1 0 9 9.5 Z M6.5 14.5 A5.5 5.5 0 1 0 17.5 14.5 A5.5 5.5 0 1 0 6.5 14.5 Z",
  terminator: "M7 8 H17 A4 4 0 0 1 17 16 H7 A4 4 0 0 1 7 8 Z",
  process: "M3.5 7 H20.5 V17 H3.5 Z",
  decision: "M12 4.5 L21.5 12 L12 19.5 L2.5 12 Z",
  data: "M7.5 7 H21 L16.5 17 H3 Z",
  callout: "M5.5 4.5 H18.5 A2 2 0 0 1 20.5 6.5 V13.5 A2 2 0 0 1 18.5 15.5 H11 L6.5 19.5 V15.5 H5.5 A2 2 0 0 1 3.5 13.5 V6.5 A2 2 0 0 1 5.5 4.5 Z",
  brace: "M15.5 3 Q11.5 3 11.5 6.5 V9.5 Q11.5 12 8.5 12 Q11.5 12 11.5 14.5 V17.5 Q11.5 21 15.5 21",
  "brace-down": "M3 8.5 Q3 12.5 6.5 12.5 H9.5 Q12 12.5 12 15.5 Q12 12.5 14.5 12.5 H17.5 Q21 12.5 21 8.5",
};

/** Whether `id` draws a closed outline (shows the fill preview). */
const closed = (id: string) => (PATHS[id] ?? "").trimEnd().endsWith("Z");

export function ShapeGlyph(props: { id: string; size?: number; fill?: string | null }) {
  const key = () => (parseTable(props.id) ? "table" : props.id);
  return (
    <svg
      class="shape-glyph"
      width={props.size ?? 24}
      height={props.size ?? 24}
      viewBox="0 0 24 24"
      aria-hidden="true"
      fill="none"
      stroke="currentColor"
      stroke-width="1.5"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <Show when={props.fill && closed(key())}>
        <path d={PATHS[key()]} fill={props.fill!} stroke="none" fill-rule="evenodd" />
      </Show>
      <path d={PATHS[key()] ?? PATHS.rect} stroke-dasharray={key() === "dashed-line" ? "3 3" : undefined} />
    </svg>
  );
}
