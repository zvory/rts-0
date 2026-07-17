# Phase 3 - Row-Dirty World Fog

## Phase Status

- [ ] Not started.

## Objective

Stop rebuilding the complete Pixi world-fog geometry when a real fog revision changes only a few
rows. First stabilize backend-owned visible/explored staging by immutable grid identity, then retain
one exact level buffer and Graphics object per fog row and rebuild only rows whose final level bytes
changed. Preserve every authoritative revision, current-frame visibility, run order, pixels,
failure retry, and teardown.

## Current Evidence and Pre-Implementation Gate

The planning baseline spent 0.313 ms/frame in `renderer.fogDraw`, 2.69% of `frame.work` and 6.44% of
`renderer.update`. It missed the fog cache and cleared the full Graphics object on all 1,249 frames.
Read-only stream analysis found an average of 6.92 changed rows and 8.65 changed tiles per transition,
with p95 19 and maximum 37 changed rows out of 126; 35 of 899 transitions did not change a final
fog-level tile.

Rerun the canonical profile and exact stream-level analysis after Phase 2 is merged. Defer this
phase without production changes if any of these are true:

- `renderer.fogDraw` is below 0.20 ms/frame or 1.5% of `frame.work`
- p95 changed rows exceed 25% of map rows
- remaining full-grid clear/retessellation on real fog revisions no longer explains material fog
  cost

This is a complexity gate, not a request to weaken fog revisions until the numbers qualify.
Cache misses below 90% skip or narrow Checkpoint A only; they do not reject row-dirty geometry when
real revisions still incur material whole-grid work.

## Checkpoint A - Revision-Staged Pixi Fog Facade

- Give `PixiPresentationAdapter` one private staging owner, preferably a focused
  `client/src/renderer/pixi_fog_staging.js` class.
- Stage `frame.visible` and `frame.explored` independently by immutable `GridSnapshotV1` object
  identity plus dimensions. Copy only a changed snapshot; do not assume equal numeric revision means
  equal content after a map/generation/replacement reset.
- Return a stable frozen facade whose methods close over private typed arrays and whose public
  metadata exposes the staged dimensions/revisions needed by the legacy renderer. Preserve the
  compatibility facade's existing `revealAll:false`; ordinary reveal/all-clear behavior is already
  materialized in the visible/explored grids and must not force a `PresentationFrameV1` change.
  Never expose mutable arrays across adapter boundaries.
- Reset or reallocate on map identity, generation, grid shape, or static-map changes. Release all
  staging on idempotent adapter destroy.
- Stage both required grids before calling the renderer so no render can observe a partial
  visible/explored mix. A complete staged facade may be reused on retry after an update or present
  failure; do not add transactional rollback or double buffering solely because presentation failed.
- Measure this checkpoint before adding row objects. If it independently clears the final acceptance
  gate, stop and defer Checkpoint B as unnecessary complexity.

## Checkpoint B - Exact Row-Dirty Geometry

- Replace the monolithic `_fogGfx` geometry with one renderer-owned fog container and one persistent
  `PIXI.Graphics` row per map row, kept in exact top-to-bottom order.
- For every real revision, compute the current final visual level for each tile:
  - 0: visible/clear
  - 1: explored/dim
  - 2: unexplored/dark
  - 3: unexplored impassable/dimmed according to the existing rule
- Compare candidate row bytes with that row's last committed `Uint8Array`. Byte comparison is the
  correctness authority; revisionless mutable fog must run exact row-byte comparisons on every
  call, and a hash may be only a diagnostic/prefilter that never authorizes a skip.
- Clear and retessellate only changed rows. Draw row geometry at local y=0, set
  `rowGraphics.position.y = tileY * tileSize`, and preserve the existing left-to-right run merging,
  x extents, colors, alphas, and fill order.
- Scan all rows required by the current revision in the same logical frame. Do not amortize rows,
  stagger work, or display a mixture of old and new fog.
- Commit a row's level buffer only after its draw succeeds; commit the global fog/map/key only after
  all changed rows succeed. A failure leaves stale metadata so the next frame repairs the row and
  global state.
- Conservatively invalidate on fog/map identity, generation, dimensions, tile size, terrain
  revision, replacement fog objects, and revisionless content. Add either a renderer-owned terrain
  revision incremented by every static-map/preview/tile-update path or an explicit fog-cache
  invalidation from those paths; in-place terrain mutation must not preserve stale level-3 geometry.
  Preserve the existing identical-fog/map/key zero-work fast path.
- Destroy all row Graphics, the container, level buffers, and staging state on reset/rematch/destroy.
- If row objects lower update time but increase Pixi present enough to miss the net gate, revert the
  row layer. Retain the simpler staging facade only if Checkpoint A independently passes.
- Mark this phase Done in this file in the accepted implementation or measured-no-go commit.

## Expected Touch Points

- `client/src/renderer/pixi_compatibility_adapter.js`
- one focused private staging helper under `client/src/renderer/`
- `client/src/renderer/index.js`
- `client/src/renderer/fog.js`
- `client/src/renderer/terrain.js` for explicit terrain-revision/invalidation ownership
- `tests/client_contracts/renderer_contracts.mjs`
- `tests/client_contracts/pixi_presentation_adapter_contracts.mjs`
- `tests/client_contracts/presentation_frame_contracts.mjs`
- `tests/client_contracts/camera_fog_contracts.mjs`
- `docs/design/client-ui.md` and `docs/design/client-rendering.md` where backend staging/cache
  behavior changes
- `docs/perf-tracing.md` only if reusable parity/dirty-row measurement tooling changes its workflow

Do not change `client/src/fog.js` authority/revision behavior, server visibility, snapshot or wire
shapes, `PresentationFrameV1`, minimap fog, Babylon fog, camera/DPR/fidelity, or layer order.

## Characterization and Focused Tests

Adapter tests must prove:

- Frames reusing identical grid snapshots perform no copy and reuse the stable facade.
- A visible-only or explored-only replacement copies only that grid.
- A new snapshot with a colliding revision after generation/map reset still copies.
- Shape changes reallocate safely; reset and destroy release staging idempotently.
- No render observes a partial visible/explored mix; a complete facade can be reused on retry.

Renderer tests must prove:

- The initial frame creates world-space normalized rectangles and fill sequences identical to the
  monolithic implementation after row transforms are applied; raw row-local y arguments are
  intentionally different.
- An unchanged revision performs no row scan/clear/tessellation.
- One-tile and discontiguous-row changes clear only the exact affected rows.
- Dark-to-clear, dark-to-dim, dim-to-clear, clear-to-dim, impassable level 3, all-clear, and all-dark
  grid transitions use exact current colors/alphas. A direct-renderer fallback fixture may still
  cover a legacy `revealAll` fog object without adding reveal state to the presentation facade.
- Same-revision replacement fog and revisionless content cannot produce a false cache hit.
- Map/terrain/tile-size/dimension/generation changes invalidate all required state.
- A throwing row draw does not commit that row/global key and the next call repairs it.
- Camera pan/zoom shows no seams; reset/rematch/leave-reenter/destroy leaks no row or display object.

Run at least:

```bash
node tests/client_contracts/renderer_contracts.mjs
node tests/client_contracts/pixi_presentation_adapter_contracts.mjs
node tests/client_contracts/presentation_frame_contracts.mjs
node tests/client_contracts/camera_fog_contracts.mjs
node tests/client_contracts.mjs
node scripts/check-client-architecture.mjs
node tests/select-suites.mjs --verify
node scripts/check-docs-health.mjs
git diff --check
```

GitHub's `Main test gate` remains authoritative.

## Before/After Performance Gate

Use the phase parent and candidate as separate clean worktrees with identical local browser/settings.
Perform two unrecorded warmups per commit, then nine paired unprofiled 15-second canonical runs,
alternating AB/BA order. Compute exact averages from each summary's `totalMs / count` for
`frame.work`, `renderer.update`, `renderer.present`, and `renderer.fogDraw`; retain every ignored
artifact and report both commits, settings, run order, and per-pair result. Report each improvement
as the median of the nine matched per-pair percentage changes, not a percentage between two unpaired
revision medians.

Run one parent and candidate flamegraph after unprofiled acceptance to prove fog/Graphics work fell
rather than moving into Pixi present, GC, or work outside the measured frame. The phase may merge
only when all of these hold:

- median paired `renderer.fogDraw` improves at least 50%
- median `renderer.update` improves at least 3%
- median whole `frame.work` improves at least 1.5%
- at least eight of nine pairs improve both update and whole-frame average
- median `renderer.present` regresses no more than 1%, frame-work p95 no more than 2%, and max no
  more than 5%
- rendered/reconciled fog revisions do not decrease, and no runtime errors or diagnostic overflow
  appears

If the result is inconclusive, run one fresh 11-pair sequence with unchanged commits and do not pool
the two experiments. If it still misses the threshold, revert/defer the row/staging complexity.

## Exact-Pixel Fog Gate

From the 900-frame stream, compute the exact final fog-level grid per tick using visible state,
cumulative explored state, terrain, and the current impassable rule. Select 16 transition ticks
without replacement using a fixed printed PRNG seed, then capture parent and candidate through the
ordinary snapshot decode, `GameState`, fog, presentation, and Pixi path with identical 1440x900
viewport, DPR 1, camera, alpha=1, absolute render clock, and ready assets.

Decode each PNG to RGBA and require byte identity with zero changed pixels. Record the seed, tick
list, state tick, fog revisions, dimensions, asset readiness, and hashes; synthetic contract tests
cover reveal-all, impassable edges, resets, and no-change revisions in addition to these random
transition samples.

## Local Gameplay Test Focus

Use the project-local `interact` workflow with an ordinary authoritative active-player Pixi match.
Move sight across terrain and confirm newly visible tiles clear in the same frame, lost sight becomes
explored/dim, never-seen terrain stays dark, unexplored impassable terrain keeps its current shade,
and current fog remains in the same order relative to remembered/intel/reveal layers. Exercise
camera pan/zoom, smoke/server visibility changes, selection/commands around hidden entities, map
reset, rematch, and leave/re-enter; inspect for row seams or stale disclosure.

## Complexity Stop Criteria

Stop if exact pixels, authoritative fog freshness, or failure recovery cannot be proven. Stop if the
design requires changes outside Pixi-private staging/renderer ownership, a generalized render cache,
or a fog rule/protocol change. Complete as a measured no-go if the extra row objects, buffers, and
invalidation branches do not clear the end-to-end threshold even when dirty-row counters improve.

## PR and Handoff Requirements

- Implement on a fresh `zvorygin/` branch after Phase 2 is merged and reachable from
  `origin/main`, regardless of Phase 2's accepted/no-go result.
- Run `scripts/agent-pr.sh --verification "<focused checks, fog parity, and nine-pair before/after gate passed or measured no-go documented>"`, then `scripts/wait-pr.sh <pr>` and verify reachability.
- The handoff must report the staging and row-cache ownership/invalidation rules, parent/checkpoint/
  final artifacts, pairwise and median results, dirty-row distribution, revision counts, parity
  seed/ticks/hashes, complexity inventory, and manual authority/seam/teardown checks.
