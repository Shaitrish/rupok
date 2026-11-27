// Where each Classic island sits (layout.ts, components/Dock.tsx): its own
// `place` when the teacher (or an agent) moved it, else by its order (first
// left, last right, the rest in the middle) on the dock's edge, mirrored
// when the side islands are swapped. Pure, for the unit tests.

import type { DockConfig, DockGroup, IslandPlace } from "./modes";

export type Edge = "top" | "bottom";
export type Slot = "left" | "centre" | "right";

export const placeOf = (edge: Edge, slot: Slot): IslandPlace => `${edge}-${slot}` as IslandPlace;
export const splitPlace = (p: IslandPlace): [Edge, Slot] => p.split("-") as [Edge, Slot];
const mirror = (s: Slot): Slot => (s === "left" ? "right" : s === "right" ? "left" : s);

/** The place of every island, in group order. */
export function islandPlaces(d: DockConfig, swapped: boolean): IslandPlace[] {
  const gs = d.groups ?? [];
  const n = gs.length;
  const edge: Edge = d.position === "top" ? "top" : "bottom";
  return gs.map((g, i) => {
    if (g.place) {
      const [e, s] = splitPlace(g.place);
      return placeOf(e, swapped ? mirror(s) : s);
    }
    const s: Slot = n === 1 ? "centre" : i === 0 ? "left" : i === n - 1 ? "right" : "centre";
    return placeOf(edge, swapped ? mirror(s) : s);
  });
}

/** Groups with every island's place written out (and the swap folded in). */
export function bakePlaces(d: DockConfig, swapped: boolean): DockGroup[] {
  const places = islandPlaces(d, swapped);
  return (d.groups ?? []).map((g, i) => ({ ...g, place: places[i] }));
}

/** Move island `i` to `to`; an island already alone there takes its old place. */
export function moveIsland(d: DockConfig, swapped: boolean, i: number, to: IslandPlace): DockGroup[] {
  const gs = bakePlaces(d, swapped);
  if (!gs[i]) return gs;
  const from = gs[i].place!;
  const others = gs.filter((g, j) => j !== i && g.place === to);
  if (others.length === 1) others[0].place = from;
  gs[i] = { ...gs[i], place: to };
  return gs;
}

/** Every island to the other edge (top ↔ bottom), keeping its third. */
export function flipIslands(d: DockConfig, swapped: boolean): DockGroup[] {
  return bakePlaces(d, swapped).map((g) => {
    const [e, s] = splitPlace(g.place!);
    return { ...g, place: placeOf(e === "top" ? "bottom" : "top", s) };
  });
}

/** Are the islands (mostly) at the top? */
export function islandsAtTop(d: DockConfig, swapped: boolean): boolean {
  const p = islandPlaces(d, swapped);
  return p.length > 0 && p.filter((x) => x.startsWith("top")).length * 2 > p.length;
}
