# Renderer LOD and tile pyramid

`Status: 📋 planned · Phase 3 · Difficulty: advanced`

Keep zoomed-out views of huge boards as smooth as zoomed-in ones by drawing cached raster tiles and decimated geometry instead of every segment.

## Goal

`rupok-render` draws every visible stroke segment as an SDF capsule instance.
That is ideal at normal zoom, but when a teacher zooms out over a long class
(100,000 strokes on an infinite board), everything is visible at once and the
frame cost grows with the total ink. This plan adds level-of-detail so frame
time depends on screen size, not on how much ink exists:

1. **Geometric LOD**: pre-decimated vertex ranges for low zoom levels.
2. **Tile pyramid**: cached raster tiles of dry ink at power-of-two scales,
   used below a zoom threshold.

## User story

- *As a teacher* at the end of a 3-hour revision session on one infinite board,
  I pinch out to see everything and the board still moves smoothly, so I can
  find the diagram from the first hour.
- *As a teacher* on an old school PC with integrated graphics, zooming out does
  not freeze the app.
- *As a teacher*, ink looks the same when I zoom back in (no blurry tiles at
  normal zoom).

## Scope

- Zoom-dependent rendering path in `crates/rupok-render/src/ink.rs`.
- Tile cache with GPU memory cap and invalidation on edits.
- Point decimation per LOD level.
- New bench budget for whole-board pans.

## Non-goals

- Changing stored data or the `.rpk` format (LOD is a cache, never saved).
- LOD for DOM items (Smart Objects, images, text); the UI can hide or freeze
  them when tiny (separate small task, listed below).
- Vector export at low zoom (export uses full geometry).

## Design

### Current renderer (facts the design builds on)

- All dry strokes live in one vertex buffer (`InkRenderer::verts`), with
  separator vertices between strokes and per-stroke depth (`depth_of`,
  `DEPTH_STEP`, `MAX_DEPTH_IDX = 2^22 - 2`).
- Culling is per chunk of `CHUNK = 2048` vertices using `chunk_bounds`;
  `compact()` lays strokes out in Morton order (`morton_cell`) so chunks are
  spatially coherent; `visible_ranges` merges visible chunks into instanced
  draws.
- Dry ink renders into a cached layer texture (`Layer`, `ensure_layer`) that is
  reused while only the wet stroke changes (`dry_dirty`, `needs_redraw`).
- `ink.wgsl` clamps radii to about 0.6 device px (`min_r`) so thin ink stays
  visible when zoomed out, which makes far-zoom frames fill-rate heavy.
- `bench/src/main.rs` measures "pan, whole board" (zoom 0.11, all strokes
  visible) but has **no budget** for it yet.

### 1. Geometric LOD (first, simpler)

For each stroke, precompute decimated point lists for levels
`L1..L3` where the screen-space tolerance at zoom `z_k = 2^-k` is 0.5 device
px: run `rupok_ink::simplify::simplify(points, eps = 0.5 / z_k, p_eps)` and drop
strokes whose bounding box is smaller than one pixel at that level (draw them
as a single dot).

- Store each level as an extra region in the same vertex buffer, laid out per
  level so each level has its own `chunk_bounds_lod[k]`.
- `render()` picks level `k = clamp(floor(-log2(zoom * dpr)), 0, 3)` and draws
  that level's visible ranges.
- `add_stroke` appends to all levels; `remove_stroke` tombstones in all levels
  (the existing `tombstones` mechanism generalises to a list of slots per
  stroke).
- Memory: levels shrink fast (decimation), expected ≤ +40 % vertices.

### 2. Tile pyramid (second, for extreme zoom-out)

Below `zoom * dpr < 0.25`, draw dry ink from raster tiles:

```text
level k: world tile size = 512 * 2^k / tile_scale units, rendered at 512×512 px
tile key: (k, tx, ty)      cache: GPU texture array (WebGL2-safe: 2D array, ≤ 256 layers)
frame: visible tiles at level k ─▶ hit: blit quad │ miss: draw placeholder from level k+1, enqueue render
budget: render at most N tiles per frame (N = 2) so pans never stall
```

- Tiles are rendered with the existing ink pipeline into an offscreen target
  using a camera that covers the tile (reuse `write_globals` and
  `visible_ranges` with the tile rectangle).
- Invalidation: `add_stroke`/`remove_stroke`/`compact` record the stroke's
  bounds; tiles intersecting dirty bounds are marked stale at every level.
  Stale tiles keep drawing until re-rendered (one frame of staleness is fine).
- LRU eviction under a cap: 96 MB of tile memory by default, 48 MB on WebGL2.
- Wet ink always draws as vectors on top, so pen latency is unchanged.

### API changes (`crates/rupok-render/src/lib.rs`, `ink.rs`)

```rust
pub struct LodConfig { pub geometric: bool, pub tiles: bool, pub tile_mem_bytes: u64 }
impl InkRenderer {
    pub fn set_lod(&mut self, cfg: LodConfig);
}
pub struct RenderStats { /* existing fields */ pub lod_level: u8, pub tiles_drawn: u32, pub tiles_rendered: u32 }
```

`Board::stats_json` in `crates/rupok-wasm/src/lib.rs` reports the new fields.

### DOM items when zoomed out (UI)

At `zoom < 0.2`, `apps/board` replaces Smart Object iframes with their last
thumbnail (or a titled placeholder) and pauses them; this keeps dozens of
iframes from rendering at a few pixels each.

## Implementation tasks

1. Bench: add a budget "pan frame p99 whole board" (start at a measured baseline, target 8.3 ms) and print LOD stats. (good-first-issue)
2. Generalise slots to multiple vertex regions per stroke (refactor only, no LOD yet); all existing tests and bench pass. (intermediate)
3. Geometric LOD levels with `simplify`, level selection, per-level culling; headless pixel test comparing L0 vs L2 at zoom 0.25 (≤ 2 % differing pixels). (advanced)
4. Tile cache data structure (keys, LRU, memory accounting) with unit tests, no GPU. (intermediate)
5. Tile rendering into a texture array and blit; WebGL2 limits check. (advanced)
6. Dirty-bounds invalidation across levels; test that a stroke added at low zoom appears within 2 frames. (intermediate)
7. Per-frame tile render budget and placeholder from the parent level. (intermediate)
8. UI: iframe placeholders at low zoom. (good-first-issue)
9. `set_lod` toggles in the stats overlay for debugging. (good-first-issue)

## Acceptance criteria and tests

- `cargo run -p rupok-bench --release -- --check` passes with the new whole-board
  budget on the CI GPU (or software adapter with a relaxed limit).
- Visual parity: at zoom ≥ 0.5 the LOD path is not used (identical output to
  today); at lower zoom, a headless comparison against full geometry differs in
  ≤ 2 % of pixels with no missing strokes larger than 1 px.
- Erasing or adding a stroke while zoomed out shows the change within 2 frames.
- GPU memory for tiles never exceeds the configured cap (assert in debug
  builds).
- All existing budgets hold: stroke commit p99 ≤ 5 ms, open lesson ≤ 1000 ms,
  pan frame p99 @100 % ≤ 8.3 ms, wet frame p99 ≤ 4 ms.

## Performance budget

- Whole-board pan (100k strokes, zoom 0.11, 1080p): p99 ≤ 8.3 ms on the bench
  machine; ≤ 16.7 ms on integrated graphics (Intel UHD 620 class).
- Extra memory: ≤ +40 % vertex memory for geometric LOD; tile cache ≤ 96 MB
  (48 MB on WebGL2).
- Adding a stroke: LOD bookkeeping ≤ 0.2 ms, keeping stroke commit p99 ≤ 5 ms.

## Open questions

- Is geometric LOD alone enough for 100k strokes? Measure before building tiles.
- Should tiles be cached across sessions (on disk) for instant zoom-out after
  opening a big lesson, or is lazy rendering enough?
- How should highlighter (translucent) ink behave in tiles? Tiles must
  preserve the "no self-overlap darkening" rule from
  `spec/rupok-format.md#stroke-width-and-rendering`.

## References

- `spec/rupok-format.md` — "Stroke width and rendering"
- `crates/rupok-render/src/ink.rs`, `crates/rupok-render/src/ink.wgsl`, `bench/src/main.rs`
- `crates/rupok-ink/src/simplify.rs`
- [android](./android.md) (weak GPUs on classroom panels)
