# LAN "Send to board"

`Status: 📋 planned · Phase 2 · Difficulty: intermediate`

Send a lesson, a page, an image or a Smart Object from a laptop or phone to the classroom board over the local network, with a pairing code and no cloud, USB stick or account.

## Goal

The classroom board PC is often shared, locked down and badly connected to
the internet, while the teacher prepares on a personal laptop or phone. Make
"get this onto the board" a two-tap action on the same Wi-Fi/LAN, safe on a
school network full of students' devices.

## User story

- *As a teacher*, I walk into class, open Rupok on my laptop, tap "Send to
  board", pick "Room 12 board", type the 4-digit code shown on the board, and
  my lesson opens there.
- *As a teacher*, I take a photo of a student's notebook with my phone and send
  it to the board to discuss it; it appears on the current page.
- *As a teacher*, a student cannot push anything onto my board: every incoming
  item waits for me to accept it on the board.
- *As a teacher*, I keep working on my laptop and the board updates the same
  lesson live until I stop sharing.

## Scope

- Board side (desktop app): "Receive" mode with discovery, pairing and an
  accept/decline prompt.
- Sender side: desktop app, and any phone browser (no install) through a QR
  code.
- Payloads: whole `.rpk` lesson; a page; an image; a Smart Object HTML file.
- Optional live mode: incremental Loro updates from sender to board.
- Encryption and authentication derived from the pairing code.

## Non-goals

- Internet relay or NAT traversal (both devices must reach each other on the
  LAN).
- Students sending work in bulk (Phase 3 "submissions").
- Screen mirroring.

## Design

### Components

```text
crates/rupok-lan/                    (new, AGPL, native only)
  src/discovery.rs   mDNS/DNS-SD service "_rupok._tcp" (mdns-sd crate)
  src/pairing.rs     SPAKE2 from a 4–6 digit code → session key
  src/transport.rs   framed, encrypted messages over TCP (or WebSocket for browsers)
  src/server.rs      board-side listener (axum or hyper), size limits
apps/desktop/src-tauri/src/lan.rs  (new) Tauri commands/events
apps/board/src/lan/                (new) Receive panel, Send dialog, phone sender page
```

### Flow

```text
board (receiver)                               laptop (sender)
  "Receive" on → advertise _rupok._tcp ──────▶ browse, list "Room 12 board"
  show code 4821 + QR(url, fingerprint)
                         ◀──────── connect, SPAKE2(code)  ─────────
                         ◀──────── offer {kind, title, size} ───────
  prompt: "Laptop 'Rahman' sends 'Binary numbers' (2.1 MB)"  [Accept] [Decline]
                         ──────── accept ──────────────────────────▶
                         ◀──────── payload chunks (encrypted) ──────
  validate → open / insert
```

- **Pairing**: the board shows a fresh code each time Receive is turned on
  (valid 5 minutes). SPAKE2 turns the low-entropy code into a strong session
  key without exposing it to offline guessing; 5 wrong attempts lock pairing
  for a minute.
- **Phone senders**: the QR code encodes
  `http://<board-ip>:<port>/send#k=<one-time key>`. The board serves a small
  sender page (part of `apps/board` build); the key in the URL fragment never
  reaches the network in clear and encrypts uploads. A page served over plain
  `http://` from a LAN address is not a secure context, so `crypto.subtle` is
  unavailable: the page bundles a small audited JS implementation instead
  (e.g. `tweetnacl` secretbox, XSalsa20-Poly1305), and the board side uses the
  matching Rust crate.
- **Trusted devices**: after a successful pairing the sender may be
  remembered (public key pinned) so the next send skips the code but still
  shows the Accept prompt.

### Payload handling on the board

| Payload | Board action (after Accept) |
|---|---|
| `.rpk` | `Package::read` (all reader limits apply), open in a new tab/window or replace after confirmation |
| Page | Sender extracts a single-page lesson; board copies it after the current page (same copy logic as `Engine::add_page_from` in [desktop-overlay](./desktop-overlay.md)) |
| Image | `Board::add_image(bytes, ext, w, h)` on the current page |
| Smart Object HTML | `Board::add_object_html(...)` — validation errors are shown, nothing is placed |
| Live updates | `Engine::apply_remote` from [acp-agent-panel](./acp-agent-panel.md), batched every 500 ms |

- Hard limits: 200 MB per transfer, 1 transfer at a time per sender, offers
  expire after 60 s.
- Nothing from the network executes before validation; Smart Objects only run
  in the sandbox (`spec/smart-object.md#sandbox`).

### Live mode

The sender sends a snapshot once, then `Lesson::export_updates(&last_vv)`
deltas. Because both replicas share history, the board can keep writing on
the same lesson and the edits merge. When the sender stops, the board keeps
its copy (and can save it).

## Implementation tasks

1. **mDNS discovery** (advertise + browse) with a CLI demo binary.
   *(good-first-issue)*
2. **SPAKE2 pairing + encrypted framing** with unit tests (wrong code fails,
   tampering detected). *(intermediate)*
3. **Board listener** with limits, offer/accept protocol and timeouts.
   *(intermediate)*
4. **Receive panel UI** (code, QR, pending offers, trusted devices).
   *(intermediate)*
5. **Send dialog** in the desktop app (device list, payload picker,
   progress). *(intermediate)*
6. **Phone sender page** with bundled secretbox encryption (no secure context needed) and camera upload.
   *(intermediate)*
7. **Payload handlers** for `.rpk`, image and Smart Object. *(good-first-issue each)*
8. **Page payload** (single-page extraction + insert). *(intermediate)*
9. **Live mode** on top of `apply_remote`. *(advanced)*
10. **Trusted devices** (pinned keys, revoke). *(intermediate)*

## Acceptance criteria and tests

- `cargo test -p rupok-lan`: two in-process peers pair with the right code and
  exchange a payload; a wrong code fails; flipping one ciphertext byte is
  rejected.
- An offer is never applied without Accept (test the board state machine).
- A `.rpk` containing `../evil` is rejected by `Package::read` and shows an
  error; a Smart Object with a CDN script shows the `network` validation error.
- Phone flow manual test: Android Chrome and iOS Safari can send a photo via QR.
- Live mode: 100 sender strokes and 20 board strokes made concurrently end up
  on both sides in the same order.

## Performance budget

| Metric | Budget |
|---|---|
| Discovery to device listed | ≤ 2 s on a typical school LAN |
| 20 MB lesson transfer on 802.11n | ≤ 10 s |
| Live mode stroke latency (sender pen-up → visible on board) | ≤ 300 ms |

## Open questions

- Many school networks block multicast (mDNS) or isolate Wi-Fi clients. Fall
  back to "type the board's IP" shown under the QR code; is that enough?
- TCP vs WebSocket for desktop senders (one transport for both is simpler).
- Should the board run Receive permanently in the background, or only when
  the teacher turns it on? (Security vs convenience; default: off.)

## References

- `spec/rupok-format.md` — `#reader-limits-and-safety`
- `spec/smart-object.md` — `#sandbox`, `#validation`
- `crates/rupok-wasm/src/lib.rs` — `Board::add_image`, `Board::add_object_html`, `Board::load`
- Related: [acp-agent-panel](./acp-agent-panel.md), [desktop-overlay](./desktop-overlay.md),
  [google-drive-sync](./google-drive-sync.md), [live-broadcast](./live-broadcast.md)
