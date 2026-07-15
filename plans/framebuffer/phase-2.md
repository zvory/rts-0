# Phase 2 - Reuse Fog Work Across Stable Snapshots and Sources

## Phase Status

- [ ] Not started.

## Objective

Stop recomputing identical fog on every browser animation frame and stop recasting line-of-sight
rays for unchanged fallback sources. Preserve exact visible/explored grids, opaque-target behavior,
terrain occlusion, Lab/replay reset behavior, and the server-provided visibility fast path. Keep
all cache state client-local and presentation-only.

## Evidence

The baseline spends 21.6% CPU self-time in `Fog._rayClear`; `match.fog` averages 6.1 ms and has an
8 ms p95. The stream uses full-world dev snapshots, so `visibleTiles` is intentionally empty and a
spectator fallback stamps every non-neutral entity. `frame_recovery` nevertheless supplies
authoritative, non-predicted entity views, which are stable between received snapshots, while the
browser RAF runs much faster than the authored 30 Hz stream.

Normal fogged players receive the server-authoritative `visibleTiles` grid. That path currently
rescans the whole grid every RAF even though the grid also changes only when a snapshot is applied.

## Scope

### Add an honest local snapshot revision

- Give `GameState` a monotonic client-local snapshot revision that advances whenever an
  authoritative snapshot is applied and across Lab map resets, replay seeks, or other state
  replacement that can reuse a simulation tick with different contents.
- Pass that revision into `Fog.update` through an options object or another explicit injected
  value. Do not infer cache validity only from tick number, array identity, wall time, or predicted
  positions.
- When map dimensions, terrain, reveal-all mode, viewer identity, or fog-source ownership semantics
  change, invalidate the relevant cache before the next presented frame.
- When the authoritative revision and all fog inputs are unchanged, skip both server-grid scanning
  and fallback stamping while retaining current revisions and explored history.

### Cache exact fallback source masks

- Keep one bounded cache entry per current stable fog source id. Its key must include every input
  that affects visibility: kind/sight/footprint, finite authoritative position, terrain/map
  revision, and any viewer/mode distinction required by the existing source selection.
- A cache entry should contain a compact list or mask of the exact visible tile indices produced by
  the existing line-of-sight rule. Reuse it for an unchanged source; recompute only moved, added,
  changed, or invalidated sources; remove entries for sources no longer present.
- Union cached masks into the reusable next-visible grid once per new snapshot and update explored
  history from that union. Do not let stale source contributions survive removal or movement.
- Bound cache memory by current sources and map area. Clear it on reset/destroy and do not build a
  cross-match or unbounded position cache.
- Preserve the existing DDA corner, opaque-target, footprint-origin, map-edge, and rock-blocking
  behavior exactly. Optimization may reorganize the work but must not quantize unit positions or
  substitute approximate visibility.
- Aggregate cache hit/miss/rebuild/skip diagnostics through Phase 1's cheap counter path. Do not add
  a new per-ray profiler cost.

## Expected Touch Points

- `client/src/state.js`
- `client/src/fog.js`
- `client/src/frame_recovery.js`
- focused camera/fog, state, replay/Lab reset, profiler, and client-perf contract tests
- `docs/design/client-ui.md` if the snapshot-revision/fog-cache lifecycle becomes a durable module
  contract
- `docs/perf-tracing.md` for new bounded diagnostics

Do not change server fog, projection, compact snapshot fields, the checked-in Hellhole stream, or
entity interpolation/prediction.

## Verification

Add direct parity tests that run the cached and uncached algorithms over representative source
sets and compare the complete `visibleGrid`, `exploredGrid`, and revisions. Cover an unchanged
snapshot, moved source, removed source, new source with a reused id, changed terrain, Lab reset,
replay seek/revision change, reveal-all toggle, building footprint, map edge, diagonal rock corner,
opaque target, malformed source, server-visible grid, and no-revision compatibility call.

Run:

```bash
node tests/client_contracts.mjs
node scripts/check-client-architecture.mjs
node scripts/client-flamegraph.mjs --preview
node scripts/client-flamegraph.mjs --workload supply-300-active --seconds 15 --preview
node scripts/check-docs-health.mjs
git diff --check
```

Use the admissible active workload restored in Phase 1. Retain paired Hellhole and active summaries
from the same settings as Phase 1.

## Acceptance Evidence

- Fog work executes at most once for an unchanged authoritative snapshot revision.
- Static fallback sources reuse exact cached masks; moved/removed sources produce a grid identical
  to a clean uncached recomputation.
- Server-provided `visibleTiles` remains authoritative and no local source can reveal a hidden
  entity or affect selection/commands.
- The canonical profile shows a material reduction in `_rayClear` sampled time and `match.fog`
  average work relative to the Phase 1 baseline without moving cost into unattributed frame work.
  Report measured values instead of adding a machine-specific CI timing threshold.
- The active profile proves whether normal server-grid fog is already negligible; do not apply the
  fallback result to production-cap claims without that evidence.
- Fog and cache state are clean after map reset, replay seek, and leave/re-enter.

## Manual Visual Test

Use the project-local `interact` skill to arrange a small authoritative Pixi scene with two friendly
sources, one moving source, a rock occluder, an enemy behind the occluder, and one building. Inspect
the scene, capture one clean 1000x700 DPR 1 PNG after fog has settled, inspect it once, reject any
visibility leak or missing texture, close the session, and include only its Tailnet Preview URL in
the handoff. Manually move the camera and let the source cross the occluder boundary so fog updates
on new snapshots while remaining stable between them; repeat after one leave/re-enter cycle.

## PR and Handoff Requirements

Mark this phase Done in the implementation commit. Run
`scripts/agent-pr.sh --verification "<fog parity checks and both profiles passed>"`, then
`scripts/wait-pr.sh <pr>` and verify the phase head is reachable from `origin/main`.

The handoff must describe the snapshot-revision owner, every cache key/invalidation, parity test
coverage, Phase 1 versus Phase 2 fog/profile numbers, the active server-grid result, retained
artifact settings, the Interact Preview URL, and the Phase 3 rig-sampling work plus its manual test.
