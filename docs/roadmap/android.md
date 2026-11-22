# Android app and overlay

`Status: 📋 planned · Phase 3 · Difficulty: advanced`

Rupok on Android tablets and Android-based interactive flat panels, including a floating overlay for annotating over any app.

## Goal

Many classrooms have interactive flat panels (IFPs) that run Android, and many
teachers own an Android tablet with a stylus. This plan ships:

1. **Rupok for Android**: the same board UI and Rust engine, packaged with
   Tauri 2's Android target.
2. **Android overlay**: a floating button that draws a transparent ink layer
   over any other app (a PDF reader, YouTube, a browser), like the desktop
   overlay on Windows/Linux.

## User story

- *As a teacher* with a classroom panel running Android, I install Rupok from
  an APK or store, open my lesson from a USB stick or Drive, and teach with the
  panel's touch and pens.
- *As a teacher*, I tap the floating Rupok bubble while a video is playing,
  circle something on screen, then save the annotated screenshot to my lesson.
- *As a teacher* with a Samsung tablet and S Pen, my writing has pressure and
  feels as responsive as on the desktop app.

## Scope

- Android build of `apps/desktop` (Tauri 2 mobile) using the `apps/board` UI.
- Pen, touch and palm rejection on Android WebView; multi-touch for panels.
- File handling: open/save `.rpk` through the Storage Access Framework; open
  from other apps (intent filter).
- Overlay mode: floating bubble, transparent full-screen ink surface over other
  apps, screenshot capture into the lesson.
- Performance on panel-class GPUs (Mali/PowerVR) at 4K.

## Non-goals

- iOS/iPadOS.
- Android TV / phones as first-class targets (phones work but are not
  optimised).
- Device management (MDM) features for school fleets.
- Rewriting the UI natively; the WebView UI is the product.

## Design

### Board app on Android

- Tauri 2 mobile: `apps/desktop/src-tauri` gains an Android target
  (`tauri android init`, generated project under
  `apps/desktop/src-tauri/gen/android`). The UI is the same `apps/board` build.
- Rendering: `Board::create(canvas, force_webgl)` in
  `crates/rupok-wasm/src/lib.rs` already prefers WebGPU and falls back to WebGL2
  (`Gpu::for_canvas` in `crates/rupok-render/src/lib.rs`). Android System WebView
  (Chromium) has WebGPU on recent versions/GPUs and WebGL2 everywhere else.
  The device uses `wgpu::Limits::downlevel_webgl2_defaults()`, so the same
  renderer runs on both.
- Input: Chromium on Android delivers `pointerType: "pen"` with pressure for
  S Pen and USI styluses, and `getCoalescedEvents()`/`getPredictedEvents()`;
  `Board::pointer_move(id, samples, predicted)` already takes coalesced and
  predicted samples. `TouchMode::Auto` palm rejection (fingers navigate once a
  pen is seen) applies unchanged.
- Panels with active pens that report as touch: add a per-device setting
  "treat thin touches as pen" using `PointerEvent.width/height` (contact size),
  forwarded as a new optional argument (UI-side heuristic, engine unchanged).
- Files: Tauri dialog/fs plugins for SAF; `AndroidManifest.xml` intent filters
  for `application/vnd.rupok.lesson+zip` and `*.rpk`.

### Overlay (new Tauri plugin, Kotlin + Rust)

```text
FloatingBubbleService (foreground service, Kotlin)
   ├─ bubble view (TYPE_APPLICATION_OVERLAY)       tap: toggle ink mode
   └─ ink overlay: transparent full-screen WebView loading apps/board in "overlay" mode
        ink mode on : receives touches/pen, draws
        ink mode off: FLAG_NOT_TOUCHABLE, touches pass through to the app below
Screenshot: MediaProjection (user consent once per session) → PNG → Board.add_image on a new page
```

- New plugin crate `apps/desktop/plugins/tauri-plugin-overlay` (Rust side for
  commands, Kotlin side for the service). Permission
  `SYSTEM_ALERT_WINDOW` (user grants in settings) and a foreground-service
  notification.
- The overlay WebView runs the board engine with a transparent page background
  (`Board.set_background("plain", 0x00000000)` already exists) and no chrome
  except a small toolbar (pen, highlighter, eraser, clear, capture, close).
- "Save" adds the screenshot plus the ink to the current lesson: capture PNG,
  `add_page`, `add_image` full-frame, then re-add the overlay strokes on top
  (same coordinates).

### Performance on panels

- 4K panels with weak GPUs: render at a capped device pixel ratio (setting:
  "render scale", default 1.0 at 4K, applied in `Board::resize(css_w, css_h, dpr)`).
- Rely on the dry layer cache (only wet ink redraws while writing) and on
  [renderer LOD](./renderer-lod.md) for zoomed-out views.
- Keep the WebView alive in the background to avoid reload on app switch.

## Implementation tasks

1. Tauri Android target builds and runs the board (debug APK in CI artifact). (intermediate)
2. Device test matrix doc + a manual QA checklist (S Pen tablet, USI pen Chromebook-class tablet, two IFP brands). (good-first-issue)
3. Pen/touch verification on Android WebView: pressure, coalesced and predicted events, palm rejection; fix UI-side gaps. (intermediate)
4. SAF open/save and intent filters for `.rpk`. (intermediate)
5. Render scale setting wired to `Board::resize`. (good-first-issue)
6. Overlay plugin: bubble service, permission flow, pass-through toggle. (advanced)
7. Overlay ink WebView in transparent "overlay" mode with minimal toolbar. (intermediate)
8. MediaProjection screenshot → new lesson page with ink preserved. (advanced)
9. Release pipeline: signed APK/AAB in CI, F-Droid metadata (all dependencies must be FOSS-compatible). (intermediate)
10. Docs: install guide for panels (sideloading, permissions). (good-first-issue)

## Acceptance criteria and tests

- The Android build opens, edits and saves a `.rpk` that the desktop build
  opens byte-compatibly (same `Package::read` result).
- On a Samsung Galaxy Tab S-class device with S Pen: pressure varies line width;
  measured pen-to-ink latency ≤ 50 ms (high-speed camera or the stats overlay
  frame timings).
- On a 4K Android panel (Mali-G52-class GPU): writing at 60 fps with no dropped
  frames for 5 minutes; pan @100 % stays smooth.
- Overlay: ink mode off passes every touch to the app below; ink mode on draws;
  capture produces a page with the screenshot and the ink aligned within 2 px.
- Removing the overlay permission degrades gracefully (feature hidden, no
  crash).

## Performance budget

- Wet frame p99 ≤ 8 ms on panel-class GPUs (desktop budget is 4 ms; panels get
  a relaxed target), dry pan p99 ≤ 16.7 ms at 4K with render scale 1.0.
- Cold start to usable board ≤ 3 s on a 2021 mid-range tablet.
- APK size ≤ 40 MB (wasm engine + UI + vendored `rupoklib` libraries).

## Open questions

- Which IFP vendors' Android builds ship a WebView recent enough for WebGPU,
  and how many are stuck on old WebView versions (WebGL2 only)?
- Should the overlay draw with a native wgpu surface (the native
  `rupok-render` path) instead of a WebView for lower latency?
- Distribution: Play Store, F-Droid, direct APK, or all three?

## References

- `crates/rupok-wasm/src/lib.rs` — `Board::create`, `Board::resize`, `pointer_move`
- `crates/rupok-render/src/lib.rs` — `Gpu::for_canvas`, device limits
- `spec/rupok-format.md` — package compatibility
- [tauri-desktop](./tauri-desktop.md), [desktop-overlay](./desktop-overlay.md), [renderer-lod](./renderer-lod.md), [handwriting-recognition](./handwriting-recognition.md)
