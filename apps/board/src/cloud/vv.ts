// Loro version vectors: the engine hands them out encoded (`Board.live_version`,
// Loro's `VersionVector::encode`, a postcard map of peer id u64 → counter i32),
// the Rupok Cloud API speaks JSON `{ "<peer decimal>": counter }`.
// postcard: varint(map length), then per entry varint(u64 peer), zigzag-varint(i32 counter).

import type { VV } from "./api";

function readVarint(b: Uint8Array, p: { i: number }): bigint {
  let x = 0n;
  let shift = 0n;
  for (;;) {
    if (p.i >= b.length) throw new Error("truncated version vector");
    const byte = b[p.i++];
    x |= BigInt(byte & 0x7f) << shift;
    if (!(byte & 0x80)) return x;
    shift += 7n;
    if (shift > 70n) throw new Error("bad varint");
  }
}

function writeVarint(out: number[], x: bigint) {
  do {
    let byte = Number(x & 0x7fn);
    x >>= 7n;
    if (x) byte |= 0x80;
    out.push(byte);
  } while (x);
}

export function decodeVV(bytes: Uint8Array): VV {
  const p = { i: 0 };
  const out: VV = {};
  if (!bytes.length) return out;
  const n = Number(readVarint(bytes, p));
  for (let k = 0; k < n; k++) {
    const peer = readVarint(bytes, p);
    const z = readVarint(bytes, p);
    const counter = Number((z >> 1n) ^ -(z & 1n));
    out[peer.toString()] = counter;
  }
  return out;
}

export function encodeVV(vv: VV): Uint8Array {
  const entries = Object.entries(vv).filter(([, c]) => Number.isFinite(c));
  const out: number[] = [];
  writeVarint(out, BigInt(entries.length));
  for (const [peer, c] of entries) {
    writeVarint(out, BigInt(peer));
    const v = BigInt(Math.trunc(c));
    writeVarint(out, v >= 0n ? v << 1n : (-v << 1n) - 1n);
  }
  return new Uint8Array(out);
}

/** True when `a` has everything `b` has. */
export function covers(a: VV, b: VV): boolean {
  return Object.entries(b).every(([p, c]) => (a[p] ?? 0) >= c);
}
