# Phase 1 - Retained Status Geometry

## Phase Status

- [ ] Not started.

## Objective

Retain HP, construction, and deconstruction bar geometry in the existing per-entity Pixi pool.
Keep position and visibility current on every rendered frame, but issue new Graphics draw commands
only when the exact bar output changes. Use the existing static-slot lifecycle so this optimization
does not introduce another cache system.

## Current Evidence and Scope Gate

The planning baseline reported `renderer.selectionHp` at 0.5 ms average and 133.6
`renderer.graphics.clear.hpBars` operations per frame. It also reported 132.9 reused HP-bar
objects per frame, proving that object reuse currently still clears and retessellates geometry.
The canonical sample had no selected entities, so it does not support a standalone performance
claim for selection-ring caching.

Before editing, run the shared baseline protocol from current `origin/main`. Defer this phase as a
measured no-go if HP-bar clears are no longer substantial or `renderer.selectionHp` is below the
measurement noise floor; do not implement a cache merely because this plan once observed the
hotspot.

## Implementation Work

- Before changing renderer code, implement the shared `scripts/client-render-parity.mjs` command
  specified in `plan.md`. Its base/candidate worktree launch, exact snapshot-frame stepping, fixed
  capture, ready-asset wait, decoded-RGBA comparison, JSON summary, failure exit, and diff artifacts
  are prerequisites for this and later phases; capture the Phase 1 base set before the first runtime
  edit.
- Add focused contracts for parity CLI validation, deterministic tick selection, exact state/tick/
  capture inputs, byte-identical success, one-pixel failure, missing assets, browser/render errors,
  and cleanup of both isolated local servers. The tool may use existing `window.__rts.match`,
  `SnapshotStreamNet` internals, and fixed-capture methods from Puppeteer test code, but must not add a
  production bridge or modify the measured frame.
- Convert HP/status bars from world-coordinate draw commands to entity-local geometry. Set the
  pooled Graphics position from the current interpolated entity coordinates on every frame so
  movement remains smooth and current.
- Build an exact render key from the values that determine draw output, including:
  - entity kind and the resulting unit size or building footprint
  - map tile size
  - clamped HP/construction/deconstruction fraction
  - construction/deconstruction/ordinary status kind
  - resulting foreground color and fixed background geometry
- Reuse `_staticSlot(poolName, id, renderKey)` in `client/src/renderer/entities.js`. Clear and draw
  only on a miss, and commit `rtsStaticRenderKey` only after every draw command succeeds so a failed
  frame retries on the next frame.
- Preserve selected full-health bars, damaged bars, construction bars, and deconstruction bars.
  Selection show/hide must take effect in the current frame even when retained geometry is reused.
- Selection-ring retention is optional only as a small sibling use of the same helper. Include it
  only if the targeted selection workload shows measurable repeated work, the key is limited to
  calculated ring dimensions plus resolved relationship color, and no new state/lifecycle is
  needed; otherwise leave rings unchanged.
- Preserve the existing `_seen`, sweep, eviction, rematch, and destroy ownership. Do not add a
  second map of cache keys or a generalized Graphics-cache abstraction.
- Add exact cache hit/miss/clear diagnostics needed to prove behavior, using the existing bounded
  counter vocabulary rather than per-entity dynamic labels.
- Mark this phase Done in this file in the implementation or measured-no-go commit.

## Expected Touch Points

- `client/src/renderer/entities.js`
- `client/src/renderer/entity_state.js` only if a pure exact status-key helper belongs there
- `client/src/renderer/index.js` only for bounded diagnostics or pool ownership
- `client/src/renderer/layers.js` only for lifecycle assertions, not a sweep redesign
- `tests/client_contracts/renderer_contracts.mjs`
- `scripts/client-render-parity.mjs`
- a focused parity-driver contract under `tests/client_contracts/`
- `package.json` / `package-lock.json` only if a small portable PNG-decoding dependency is required

Building production overlays, world fog, minimap fog, rigs, generic Pixi batching, pool sweeping,
the presentation frame, and selection authority are outside this phase.

## Characterization and Focused Tests

Add characterization before changing geometry, then prove:

- An unchanged damaged entity that moves updates `Graphics.position` but issues no `clear`,
  `drawRect`, or other geometry command.
- HP fraction, the 66% and 33% color thresholds, construction fraction, deconstruction fraction,
  status kind, kind/footprint, and tile-size changes each invalidate exactly once.
- Selected full-health, damaged unselected, construction, and deconstruction behavior remains
  identical.
- Selection toggling hides or re-shows the correct bar in the same frame without stale geometry.
- If selection-ring retention is included, own/ally/enemy/neutral relationship and ring-size
  changes invalidate exactly once.
- A throwing draw command leaves no committed key; the following frame retries and reaches the
  same warm-cache state as a clean draw.
- Fog disappearance/reappearance, death, pool hiding, 120-frame eviction, rematch, and
  `Renderer.destroy()` leave no visible stale object or leaked cache/resource.
- HP bars remain above entity layers with the existing alpha, colors, rectangle dimensions, and
  interpolation.

Run at least:

```bash
node tests/client_contracts/renderer_contracts.mjs
node tests/client_contracts/client_render_parity_contracts.mjs
node tests/client_contracts.mjs
node scripts/check-client-architecture.mjs
node tests/select-suites.mjs --verify
tests/run-all.sh --only-browser-scenarios=smoke
git diff --check
```

GitHub's `Main test gate` remains authoritative.

## Before/After Performance Gate

Follow the shared five-sample ABBA protocol with the canonical stream before and after. Compare
`frame.work`, `match.renderer`, `renderer.update`, `renderer.present`,
`renderer.selectionHp`, HP cache hits/misses, and HP Graphics clears. Run
`selected-unit-hud-stress` before and after as a secondary targeted selection-path check, but do not
combine it with or substitute it for the sole supply-scale result.

The phase may merge only when the five-sample medians show all of:

- at least 75% fewer HP-bar clears per frame
- at least 40% lower `renderer.selectionHp` average
- either at least 2% lower total `frame.work` average or at least 3% lower combined
  `renderer.update + renderer.present` average
- no more than 2% regression in `frame.work` p95 or median max, no more than 2% regression in
  `renderer.present` median average or p95, and no new long-frame/runtime errors

The candidate flamegraph must show less geometry construction/triangulation work beneath the HP
path rather than cost moved outside the measured frame. Cache-counter success alone does not earn
the production complexity.

## Exact-Pixel Gate

Use the shared seeded 16-tick canonical-stream capture and require byte-identical decoded RGBA.
Supplement those random ticks with targeted fixed-state captures for moving damaged infantry and
vehicles, selected full-health entities, HP crossing 66% and 33%, construction/deconstruction
progress, selection show/hide, and relationship-color changes. The targeted captures do not
replace the random samples.

## Local Gameplay Test Focus

Use the project-local `interact` workflow with an ordinary authoritative release match or Lab
scene. Inspect moving damaged units, selection toggles, healing/damage threshold transitions,
construction and deconstruction progress, fog hide/reveal, death, rematch, and leave/re-enter.
Bars must remain attached to the current interpolated position with no frame of lag, stale color,
or stale visibility.

## Complexity Stop Criteria

Stop and revert the runtime experiment if exact pixels cannot be preserved, if it needs changes
outside the private Pixi overlay/pool surface, if it adds a second generalized cache/lifecycle, or
if the performance threshold is missed. A no-go completion should retain the benchmark/parity
evidence and continue to Phase 2; it must not retain speculative cache state in production.

## PR and Handoff Requirements

- Implement on a fresh `zvorygin/` branch from current `origin/main` after any preceding work has
  merged.
- Run `scripts/agent-pr.sh --verification "<focused checks, parity, and repeated before/after gate passed or measured no-go documented>"`, then `scripts/wait-pr.sh <pr>` and verify reachability.
- The handoff must include cache-key inputs, failure/reset/destroy behavior, base/candidate commits,
  run order and artifact paths, five-sample medians, clear-count delta, parity seed/ticks/hashes,
  complexity inventory, manual overlay checks, and whether Phase 2 may assume any new helper.
