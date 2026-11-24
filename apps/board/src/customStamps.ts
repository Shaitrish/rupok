// The teacher's custom stamps ("My stamps"): stored on this device in
// IndexedDB, copied into a lesson when placed (the lesson's root `stamps`
// map, so a shared lesson shows them), shared as stamp packs (a small JSON
// file), and made or changed by the teacher (StampEditor.tsx) or their AI
// agent (create_stamp / update_stamp through live.ts).
//
// Every definition passes `Board.check_stamp_json` (rupok_live::stamps::check)
// before it is kept: SVG is sanitised and drawn only as an image; a Smart
// stamp's HTML only runs as a Smart Object in the sandboxed iframe.
//
// Behaviour is declarative: a placement animation (CSS), a short burst,
// a counter badge, a sound made with WebAudio (no files, honours Settings →
// Stamps → sounds) and an auto-fade, which removes the stamp after N seconds
// as its own undo step.

import { Board } from "./wasm/rupok.js";
import { idbGet, idbPut } from "./library/idb";
import { type CustomStamp, type StampBehavior, customSize, myStamps, resolveStamp, setMyStamps } from "./stampLib";
import { prefs, setPrefs, showToast } from "./state";
import type { Controller } from "./controller";

const KEY = "stamps.v1";
export const PACK_FORMAT = "rupok-stamp-pack";
export const PACK_VERSION = 1;
const MAX_STAMPS = 200;

let loaded: Promise<void> | null = null;

/** Load "My stamps" from IndexedDB (once). */
export function loadMyStamps(): Promise<void> {
  if (!loaded) {
    loaded = (async () => {
      try {
        const v = await idbGet<{ stamps?: unknown[] }>("kv", KEY);
        const list: CustomStamp[] = [];
        for (const raw of v?.stamps ?? []) {
          const c = checkStamp(raw);
          if (c) list.push(c);
        }
        setMyStamps(list.slice(0, MAX_STAMPS));
      } catch { /* storage unavailable: an empty library */ }
    })();
  }
  return loaded;
}

async function persist(list: CustomStamp[]) {
  setMyStamps(list);
  try {
    await idbPut("kv", KEY, { stamps: list });
  } catch { /* storage unavailable: kept for this session */ }
}

/** A definition checked by the core (null when refused). Throws with the
 * reason when `strict`. */
export function checkStamp(raw: unknown, strict = false): CustomStamp | null {
  try {
    const out = JSON.parse(Board.check_stamp_json(JSON.stringify(raw))) as CustomStamp;
    const id = (raw as { id?: string })?.id;
    if (typeof id === "string" && /^[a-z0-9-]{1,32}$/.test(id)) out.id = id;
    if (!out.id) out.id = newId(out.name);
    return out;
  } catch (e) {
    if (strict) {
      let msg = String((e as any)?.message ?? e);
      try { msg = JSON.parse(msg).error ?? msg; } catch { /* plain text */ }
      throw new Error(msg);
    }
    return null;
  }
}

/** `c-<slug>-<4 chars>`, unique in the library. */
export function newId(name: string): string {
  const slug = name.toLowerCase().normalize("NFKD").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 20) || "stamp";
  for (;;) {
    const id = `c-${slug}-${Math.random().toString(36).slice(2, 6)}`;
    if (!myStamps().some((s) => s.id === id)) return id;
  }
}

/** Add or replace a stamp in the library (checked first). */
export async function saveStamp(def: Partial<CustomStamp>): Promise<CustomStamp> {
  await loadMyStamps();
  const prev = def.id ? myStamps().find((s) => s.id === def.id) : undefined;
  const clean = checkStamp({ ...prev, ...def, html: def.html ?? prev?.html }, true)!;
  if (prev?.html && !clean.html) clean.html = prev.html;
  const list = prev ? myStamps().map((s) => (s.id === clean.id ? clean : s)) : [...myStamps(), clean];
  if (list.length > MAX_STAMPS) throw new Error(`My stamps holds at most ${MAX_STAMPS} stamps`);
  await persist(list);
  return clean;
}

export async function deleteStamp(id: string): Promise<boolean> {
  await loadMyStamps();
  if (!myStamps().some((s) => s.id === id)) return false;
  await persist(myStamps().filter((s) => s.id !== id));
  if (prefs().stampFavorites.includes(id)) setPrefs({ stampFavorites: prefs().stampFavorites.filter((f) => f !== id) });
  return true;
}

export async function renameStamp(id: string, name: string) {
  return saveStamp({ id, name });
}

/** Move a stamp one place earlier (-1) or later (+1) in the palette. */
export async function moveStamp(id: string, dir: -1 | 1) {
  const list = [...myStamps()];
  const i = list.findIndex((s) => s.id === id), j = i + dir;
  if (i < 0 || j < 0 || j >= list.length) return;
  [list[i], list[j]] = [list[j], list[i]];
  await persist(list);
}

// -------------------------------------------------------------- packs

export interface StampPack {
  format: typeof PACK_FORMAT;
  version: number;
  name?: string;
  stamps: CustomStamp[];
}

/** A stamp pack of the given stamps (default: all of "My stamps"). */
export function exportPack(ids?: string[], name = "My stamps"): StampPack {
  const list = ids ? myStamps().filter((s) => ids.includes(s.id)) : myStamps();
  return { format: PACK_FORMAT, version: PACK_VERSION, name, stamps: list.map(({ bundle: _b, ...s }) => s) };
}

/** Import a pack: every stamp is checked; ids that clash get new ones. */
export async function importPack(raw: unknown): Promise<{ added: number; refused: number }> {
  if (!raw || typeof raw !== "object") throw new Error("not a JSON object");
  const p = raw as Partial<StampPack>;
  if (p.format !== PACK_FORMAT) throw new Error(`"format" must be "${PACK_FORMAT}"`);
  if (typeof p.version !== "number" || p.version > PACK_VERSION) throw new Error(`unsupported version ${p.version}`);
  if (!Array.isArray(p.stamps)) throw new Error("no stamps in the pack");
  await loadMyStamps();
  let added = 0, refused = 0;
  const list = [...myStamps()];
  for (const s of p.stamps.slice(0, MAX_STAMPS)) {
    const c = checkStamp(s);
    if (!c) { refused++; continue; }
    if (list.some((x) => x.id === c.id)) c.id = newId(c.name);
    list.push(c);
    added++;
  }
  await persist(list.slice(0, MAX_STAMPS));
  return { added, refused };
}

export function downloadPack(pack = exportPack()) {
  const blob = new Blob([JSON.stringify(pack, null, 2)], { type: "application/json" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = `${(pack.name ?? "stamps").replace(/[^\w -]+/g, "-")}.rupok-stamps.json`;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 2000);
}

export function pickPack() {
  const i = document.createElement("input");
  i.type = "file";
  i.accept = ".json,application/json";
  i.onchange = async () => {
    const f = i.files?.[0];
    if (!f) return;
    try {
      const r = await importPack(JSON.parse(await f.text()));
      showToast(`Added ${r.added} stamp${r.added === 1 ? "" : "s"}${r.refused ? ` (${r.refused} refused)` : ""}`, 4000);
    } catch (e: any) {
      showToast(`Could not import ${f.name}: ${e?.message ?? e}`, 6000);
    }
  };
  i.click();
}

// ---------------------------------------------------------- placement

/** Place a custom stamp centred on a board point, as the teacher (one undo
 * step; the definition goes into the lesson with it). Returns the item id. */
export function placeCustom(c: Controller, id: string, x: number, y: number, size?: number): string | undefined {
  const d = resolveStamp(id);
  if (!d) return undefined;
  const { w, h } = customSize(d, size);
  const op = { op: "add_stamp", stamp: id, x: x - w / 2, y: y - h / 2, size, def: d };
  try {
    const r = JSON.parse(c.board.live_apply(JSON.stringify(op), ""));
    return r.item_id as string | undefined;
  } catch (e) {
    let msg = String((e as any)?.message ?? e);
    try { msg = JSON.parse(msg).error ?? msg; } catch { /* plain */ }
    showToast(`Could not place the stamp: ${msg}`, 5000);
    return undefined;
  }
}

// ----------------------------------------------------------- behaviour

let audio: AudioContext | null = null;

/** A short built-in sound, synthesised (no files, works offline). */
export function playSound(kind: StampBehavior["sound"]) {
  if (!kind || kind === "none" || !prefs().stampSounds) return;
  try {
    audio ??= new AudioContext();
    const a = audio;
    const t0 = a.currentTime + 0.01;
    const tone = (f: number, at: number, dur: number, type: OscillatorType = "sine", gain = 0.18) => {
      const o = a.createOscillator(), g = a.createGain();
      o.type = type;
      o.frequency.setValueAtTime(f, t0 + at);
      g.gain.setValueAtTime(0, t0 + at);
      g.gain.linearRampToValueAtTime(gain, t0 + at + 0.01);
      g.gain.exponentialRampToValueAtTime(0.0001, t0 + at + dur);
      o.connect(g).connect(a.destination);
      o.start(t0 + at);
      o.stop(t0 + at + dur + 0.02);
    };
    const noise = (at: number, dur: number, gain = 0.25) => {
      const n = a.sampleRate * dur;
      const buf = a.createBuffer(1, n, a.sampleRate);
      const ch = buf.getChannelData(0);
      for (let i = 0; i < n; i++) ch[i] = (Math.random() * 2 - 1) * (1 - i / n) ** 3;
      const src = a.createBufferSource(), g = a.createGain();
      src.buffer = buf;
      g.gain.value = gain;
      src.connect(g).connect(a.destination);
      src.start(t0 + at);
    };
    switch (kind) {
      case "chime": tone(1046, 0, 0.5); tone(1568, 0.08, 0.6, "sine", 0.12); break;
      case "pop": tone(660, 0, 0.09, "triangle", 0.25); break;
      case "ding": tone(1318, 0, 0.7, "sine", 0.2); break;
      case "tada": tone(523, 0, 0.18, "triangle"); tone(659, 0.12, 0.18, "triangle"); tone(784, 0.24, 0.5, "triangle", 0.22); break;
      case "clap": noise(0, 0.12); noise(0.14, 0.12, 0.2); break;
      case "whoosh": noise(0, 0.35, 0.12); break;
    }
  } catch { /* no audio */ }
}

const fadeTimers = new Map<string, number>();

/** Run a freshly placed stamp's behaviour that is not drawing: its sound and
 * its auto-fade (the stamp is removed after N seconds, as an undo step). */
export function startBehavior(c: Controller, itemId: string, b: StampBehavior | undefined) {
  if (!b) return;
  playSound(b.sound);
  if (b.fade_after && b.fade_after > 0 && !fadeTimers.has(itemId)) {
    fadeTimers.set(itemId, window.setTimeout(() => {
      fadeTimers.delete(itemId);
      const node = document.querySelector<HTMLElement>(`.item.stamp[data-item="${CSS.escape(itemId)}"]`);
      if (!node) return;
      node.classList.add("fading");
      window.setTimeout(() => {
        if (c.board.read_only()) return;
        c.board.delete_item(itemId);
        c.sync(true);
      }, 450);
    }, b.fade_after * 1000));
  }
}
