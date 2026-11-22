# Replay with audio

`Status: 📋 planned · Phase 2 · Difficulty: intermediate`

Record the teacher's voice during class and replay it in sync with every stroke, so a lesson file becomes a complete recorded lecture.

## Goal

Rupok already replays a page stroke by stroke from timestamps
(`crates/rupok-wasm/src/replay.rs`). Adding the teacher's voice turns every
lesson into a lecture recording that is a few megabytes instead of a large
screen video, stays sharp at any zoom, and remains editable. Students who
missed class can watch it; teachers can reuse it.

## User story

- *As a teacher*, I tap **Record** at the start of class; at the end I share
  the `.rpk` file and absent students hear me explain while the ink appears in
  sync.
- *As a teacher*, I can skip silences and jump to any page in the recording.
- *As a teacher*, I see clearly when the microphone is on, and I can delete the
  audio before sharing.
- *As a teacher* with a weak laptop, recording does not make my pen lag.

## Scope

- Microphone recording in the board (web) and desktop app.
- Audio stored as content-addressed assets in the package, in chunks.
- A recording timeline in the document: audio chunks and page navigation marks
  in wall-clock time.
- Real-time replay mode that follows the recording across pages with audio.
- Delete audio; trim start/end.

## Non-goals

- Video/webcam recording (MP4 export is a separate Phase 3 item).
- Audio editing beyond trim and delete.
- Transcription/subtitles (possible later with the AI agent).
- Recording other participants of an online call.

## Design

### Recording timeline (proposed; requires an RFC)

Add an optional root container to `doc.loro`:

```text
recordings : List<Map {
  id: String, started: f64 (wall clock ms), ended: f64,
  chunks: List<Map { asset: String, start: f64, duration_ms: f64, mime: String }>,
  marks:  List<Map { t: f64, page_id: String }>      // page shown from time t (wall clock)
}>
```

- **Requires an RFC** amending `spec/rupok-format.md` sections "Root containers"
  and "Replay". It is additive (readers ignore unknown root containers), so no
  version bump.
- Audio chunks are assets: `assets/<blake3>.webm` or `.ogg` (Opus). These
  extensions are already in the writer's "store, do not deflate" list
  (`is_precompressed` in `crates/rupok-format/src/package.rs`).
- Chunks of 5 minutes: a crash loses at most one chunk, and saving does not
  rewrite one huge blob. Opus at 24 kb/s mono ≈ 10.8 MB per hour.
- Page marks are needed because page navigation is UI state today
  (`Engine::goto_page` does not touch the document). While recording, each page
  change appends a mark (one commit).

### Timing correctness (prerequisite)

Replay places point *i* of a stroke at `created + dt[i]`. Today
`Engine::pointer_up` passes the **pen-up** `now_ms` as `created`
(`commit_stroke(..., now_ms)`), while the spec says writers SHOULD use pen-down
time (`spec/rupok-format.md#replay`). With audio, strokes would appear up to a
stroke-duration late. Fix: compute pen-down wall time as
`now_ms - (last_point.dt)` at pen-up, or capture `Date.now()` at pen-down in
`pointer_down` and keep it in `Gesture::Ink`. Note `StrokeBuilder::t0()` is in
the pointer-event clock (`event.timeStamp`), not wall-clock time.

### Capture

- Board (web): `MediaRecorder` with `audio/webm;codecs=opus` (Chromium,
  WebView2) or `audio/ogg;codecs=opus`; `getUserMedia({audio: {echoCancellation, noiseSuppression}})`.
  Each `dataavailable` chunk (timeslice 5 min) becomes an asset via a new
  Board method.
- Desktop: if the webview lacks `MediaRecorder` (check WebKitGTK versions), a
  native fallback in `apps/desktop/src-tauri/src/audio.rs` (new) using `cpal`
  capture and Opus encoding into Ogg pages, delivering chunks to the board over
  IPC.

### Engine and Board API (new)

```rust
impl<S: InkSink> Engine<S> {
    pub fn recording_start(&mut self, now_ms: f64) -> String;          // adds a recordings entry + first mark
    pub fn recording_add_chunk(&mut self, bytes: Vec<u8>, mime: &str, start: f64, duration_ms: f64) -> Result<(), String>;
    pub fn recording_mark_page(&mut self, now_ms: f64);                // called from goto_page while recording
    pub fn recording_stop(&mut self, now_ms: f64);
    pub fn recording_delete(&mut self, id: &str);                      // removes entry; assets dropped on next save
}
```

`Engine::save` already drops unreferenced assets (`pkg.assets.retain(...)`);
extend its "used" set with recording chunk assets.

### Replay

`Replay::new` builds a per-page timeline and compresses idle gaps
(`MAX_GAP_MS = 900`). Audio cannot be compressed, so add a mode:

```rust
pub enum ReplayClock { Compressed, RealTime { recording: String } }
```

- `RealTime`: the timeline is wall-clock time from `recordings[i].started`.
  Events from all pages are included; the current page at time `t` comes from
  the marks; the engine switches pages (`load_page`) when crossing a mark.
- The UI plays audio with an `<audio>` element (chunks concatenated through
  MediaSource, or played sequentially) and drives `replay_seek(t)` from
  `audio.currentTime`. Audio is the master clock, so drift is impossible.
- "Skip silence": optional, uses the gap list to jump over pauses > 10 s while
  muting.

## Implementation tasks

1. Fix stroke `created` to pen-down wall time in `Engine::pointer_up`/`commit_stroke`; test that `created` ≤ pen-up time − last `dt`. (good-first-issue)
2. **RFC**: `recordings` root container and replay semantics. (intermediate)
3. Core: read/write `recordings` (new `crates/rupok-core/src/recording.rs`) with tests; unknown-container tolerance test. (intermediate)
4. Engine recording API + page marks + asset retention in `save`. (intermediate)
5. Board: record button, mic permission flow, recording indicator, MediaRecorder chunking. (intermediate)
6. Real-time replay clock across pages (`ReplayClock::RealTime`) with tests using synthetic marks. (advanced)
7. Audio-driven playback in the UI (seek bar, speed 1–2× with `playbackRate`, page chips on the timeline). (intermediate)
8. Desktop native capture fallback (`cpal` + Opus) if needed on Linux. (advanced)
9. Delete/trim audio UI; "share without audio" option on export. (good-first-issue)
10. Docs: privacy note for teachers (consent, local storage, deleting audio). (good-first-issue)

## Acceptance criteria and tests

- A 10-minute recording across 3 pages replays with each stroke starting
  within ±100 ms of the audio moment it was drawn (measured with a synthetic
  test: strokes created at known times, audio with clicks at those times).
- Seeking to any time shows exactly the ink present at that moment on the
  correct page.
- Saving and reopening keeps recordings; deleting a recording removes its
  assets from the next saved package.
- Old files without `recordings` behave exactly as today (compressed replay).
- Recording for 60 minutes does not change `rupok-bench` results and adds no
  dropped pen frames (manual check with the stats overlay).

## Performance budget

- Audio size: ≤ 12 MB per hour (Opus 24 kb/s mono).
- Recording CPU: ≤ 3 % of one core on a mid-range laptop; no impact on the wet
  frame p99 ≤ 4 ms budget.
- Seek in real-time replay: ≤ 200 ms to show the right page and ink for a
  2-hour class.

## Open questions

- One recording per lesson or several (one per class session)? The list
  supports several; the UI may start with one.
- Should audio live in the lesson package or in a sidecar file so the lesson
  stays small for syncing ([google-drive-sync](./google-drive-sync.md))?
- Smart Object step changes are commits, so they replay; do objects need a
  replay-time `mode: "replay"` signal from the host for every seek?

## References

- `spec/rupok-format.md` — "Root containers", "Replay", "Assets"
- `spec/smart-object.md` — "Modes" (`replay`), "Steps"
- `crates/rupok-wasm/src/replay.rs`, `crates/rupok-wasm/src/engine.rs` (`replay_*`, `pointer_up`, `save`)
- [google-drive-sync](./google-drive-sync.md), [web-viewer](./web-viewer.md), [standalone-html-export](./standalone-html-export.md)
