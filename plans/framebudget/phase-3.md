# Phase 3 - Make Stable Layers Revision-Driven

## Phase Status

- [ ] Pending.

## Objective

Recover recurring frame headroom by updating stable visual data only when its authoritative or UI
revision changes. Fix fog facade/grid caching, cache minimap entity blips separately from animated
overlays, retain HP/selection geometry while moving it every frame, and cache occupied-trench
lookups and shapes by trench/occupancy state.

This is the final measured checkpoint. Consume the exact active-player 200/300 workloads introduced
in Phase 1 and report whether the complete client frame clears the proposed 60 FPS gate after all
five priorities have landed.

Rerun the unchanged client-only Hellhole stream once as a renderer-ceiling comparison before the
serious active-player matrix; do not use its prediction-free result as the release gate.

## Constraints and Non-Goals

- Preserve server-authoritative current visibility and client explored history. No fog rule,
  snapshot, secrecy, or protocol change is authorized.
- Use revision/dirty caching rather than stale mutable aliases. The first frame after a real state
  change must present the change correctly.
- Keep per-frame position updates for interpolated/predicted entities, selection rings, and health
  bars even when their geometry is retained.
- Keep camera footprint, pings, artillery markers, border pulses, projectiles, fades, smoke, muzzle
  flashes, and other genuinely animated effects at display cadence.
- A 30 Hz maximum for minimap entity blips is permitted; blanket minimap or combat-effect
  throttling is not.
- Base trench decals are already content-signature cached. Do not replace that working path or
  claim its texture upload as the current recurring problem.
- Do not add viewport culling, LOD, a production supply-cap change, balance changes, or a new
  rendering backend.
- Keep CPU-throttle results labeled as same-machine stress evidence, not low-end-device
  certification.

## Expected Touch Points

- `client/src/fog.js`
- `client/src/frame_recovery.js`
- `client/src/presentation/grid_snapshot.js`
- `client/src/presentation/frame.js`
- `client/src/renderer/pixi_compatibility_adapter.js`
- `client/src/renderer/fog.js`
- `client/src/minimap.js`
- `client/src/renderer/entities.js`
- `client/src/renderer/trenches.js`
- `client/src/renderer/index.js`
- `client/src/frame_profiler.js` and render diagnostics only as needed for cache proof
- focused fog, presentation, renderer, minimap, trench, selection, HP, and teardown tests
- `scripts/client-perf-harness.mjs` only where the final comparison/report needs stable cache
  counters or gate output
- `docs/design/client-rendering.md`, `docs/design/client-ui.md`, and `docs/perf-tracing.md`

## Implementation Work

### 1. Make fog revision-driven end to end

- Carry stable visible/explored revision information from GameState through the existing immutable
  grid snapshots. Run the authoritative visibility comparison and explored-history merge only when
  the source visibility revision changes.
- Give the Pixi adapter backend-owned reusable visible/explored typed buffers. Copy a grid only when
  its revision changes and reuse the same frozen fog facade identity while both revisions remain
  stable.
- Make the renderer fog cache depend on map identity, dimensions/style, and visible/explored
  revisions rather than a freshly allocated frame wrapper.
- On an unchanged frame, perform zero full-grid copies, zero full-grid fog comparisons, and zero fog
  Graphics clears/rebuilds. A changed revision must update the renderer on the first presented
  frame and must never expose hidden tiles.
- Keep reveal-all behavior limited to its existing authorized local/dev paths.

### 2. Split minimap state from animated overlays

- Add one cached offscreen minimap entity/blip layer containing neutral/enemy blips, foreground
  player blips, and the merged player outline.
- Invalidate the blip layer for an entity-source revision, relevant owner/relationship/style change,
  map transform/size change, or a predicted/current pose update admitted by a maximum 30 Hz cadence.
- Composite the cached blip layer during each minimap render, but continue drawing camera footprint,
  pings, artillery marker animation, border pulse, and other time-based overlays every RAF.
- Preserve existing terrain, resource, and fog layer caches and current z-order, scout-plane shapes,
  relationship colors, supply-scaled blip sizes, commands, targeting previews, pointer behavior,
  and teardown.
- Add diagnostics distinguishing blip rebuilds from cache hits and recording stable invalidation
  reasons without per-entity labels.

### 3. Retain HP and selection geometry

- Draw selection rings in local coordinates, position them from the current visual pose every RAF,
  and redraw only when the geometry key changes. The key must cover entity kind/size and ownership
  relationship color.
- Draw HP/construction/deconstruction bars in local coordinates, move them every RAF, and redraw only
  when kind/size, HP/max HP, or progress/status changes.
- Reconcile pool visibility immediately when an entity becomes selected/deselected, damaged/healed,
  completed, hidden, or removed.
- Keep the ordinary entity eligibility scan initially. Add selected/damaged/progress sets only if
  measured evidence shows the scan remains material after geometry caching.

### 4. Cache occupied-trench lookup and shapes

- Preserve or memoize normalized trenches and their ID lookup by a stable trench revision rather
  than normalizing, allocating, sorting, and mapping the same list every RAF.
- Give each occupied-trench shadow/lip a deterministic render key containing trench revision/id,
  occupant id, position, radius, and any style input.
- Retain the two Graphics objects and redraw their deterministic polygons only when the key changes.
  Moving, dying, hiding, entering, or leaving occupants must reconcile on the first presented frame.
- Preserve the existing content-signature cache for the base trench decal canvas/texture.

### 5. Prove cache correctness before timing success

Add focused contracts that hold a frame stable for multiple RAFs and assert:

- fog grid copy, comparison, clear, and redraw counts stay at zero after the first frame;
- changing only visible or explored revision invalidates exactly once and produces correct tile
  levels without leaking hidden sentinel data;
- minimap blips rebuild no faster than their admitted state/cadence while camera and pings continue
  changing every RAF;
- HP/selection Graphics retain geometry across position-only frames and redraw exactly once for each
  HP, progress, relationship, or selection geometry change;
- occupied-trench normalization/lookup and polygon drawing remain cached until trench or occupancy
  state changes;
- all caches reset on map replacement, renderer/minimap destruction, and rematch creation.

## Structural Acceptance Evidence

- Stable 126x126 fog frames perform no repeated 15,876-cell source scan, no two-grid adapter copy,
  and no fog Graphics rebuild.
- One fog revision change produces one source update, at most one copy per changed grid, and one
  renderer rebuild.
- Stable minimap frames perform no entity-wide blip rerasterization; the animated overlay still
  advances at display cadence.
- Position-only selected/damaged entity frames update transforms without `Graphics.clear()` or
  geometry regeneration.
- Stable occupied trenches perform no normalization/sort/map rebuild and no occupant polygon redraw.
- Cache diagnostics are bounded and do not add per-entity Map/string work to every frame.

## Focused Automated Verification

```bash
node tests/client_contracts.mjs
node tests/minimap_input_contracts.mjs
node scripts/check-client-architecture.mjs
node scripts/check-docs-health.mjs
node tests/select-suites.mjs --verify
tests/run-all.sh --only-browser-scenarios=smoke
git diff --check
```

Use narrower focused contract entry points as they exist on the implementation branch, but retain
the aggregate client contracts and minimap input suite because caching must not alter selection,
commands, fog, or pointer behavior. GitHub's `Main test gate` remains authoritative.

## Interact Lab Manual Test

Use the project-local `interact` skill and `interact lab` commands. Arrange one small authoritative
Pixi scene containing a damaged selected moving unit, a second relationship color, an occupied
trench, and a fog boundary; inspect the authoritative state before capture. Pan the camera, allow the
unit to move, change HP or selection once, and confirm the cached geometry follows immediately while
unchanged fog/trench shapes remain visually stable.

Capture one clean 1000x700 DPR 1 PNG, inspect it once, and reject stale HP, incorrect ring color,
missing trench lip/shadow, fog leakage, frozen pings, blank output, or missing textures. Close the
session and include only the returned Tailnet Preview URL in the handoff.

## Final 200/300 Measurement Checkpoint

First rerun the canonical client-only workload at default settings:

```bash
node scripts/client-perf-harness.mjs --workload supply-300-hellhole-stream --seconds 10
```

Then run the exact Phase 1 workloads with fixed composition and seed. Run the serious repeated
matrix without traces, then capture a trace for the first failing or worst representative cell:

```bash
node scripts/client-perf-harness.mjs \
  --stress-matrix \
  --workload supply-200-active \
  --workload supply-300-active \
  --matrix-cpu 1,2,4 \
  --matrix-viewport default,large \
  --matrix-dpr 1,2 \
  --matrix-repeat 3 \
  --seconds 10
```

For every cell retain workload assertions, supply/entity composition, frame work/update/present
p50/p95/max, FPS/frame-gap and RAF-dispatch evidence, cache counters, GC/long-task evidence where
available, and top recurring phase. Compare against both the Phase 1 baseline and Phase 2 result on
the same machine.

The proposed local gate is:

- 300 supply at 1x CPU across the matrix: `frame.work` p95 no more than 12 ms;
- 300 supply at 4x CPU, default viewport, DPR 1: `frame.work` p95 no more than 16.67 ms and no
  sustained below-60 sample window attributable to client work;
- no 200-supply cell regresses materially from the identical Phase 1 baseline;
- 2x/4x large-viewport DPR 2 cells remain required diagnostic evidence even when they are not used
  as the release gate.

If a gate fails, report the first failing cell, margin, top measured phase, and cache counters. Do
not change the supply cap or invent another implementation phase in this plan.

## Player-Facing Outcome

Unchanged fog, selection, HP, trench, minimap, and combat visuals with less recurring work and fewer
allocation/Graphics spikes. The final evidence tells the user whether 300 supply has sufficient
client headroom; completing the code does not itself authorize the cap change.

## PR and Handoff Requirements

- Start only after Phase 2 is merged and reachable from `origin/main`; implement on a fresh
  `zvorygin/` branch and mark this phase Done in the implementation commit.
- Run `scripts/agent-pr.sh --verification "<focused checks, Interact review, and final matrix passed>"`,
  then `scripts/wait-pr.sh <pr>` and verify the phase head is reachable from `origin/main`.
- The handoff must include cache invalidation contracts, exact structural counter results, the
  Interact Preview URL, the unchanged client-only Hellhole comparison, all matrix/trace artifact
  paths,
  before/after margins, the first failing cell if any, and a direct recommendation to keep 200 or
  proceed to a separate cap-change decision.
- When this phase marks all phase files Done, allow the PR helper to archive the plan as required by
  repository policy.
