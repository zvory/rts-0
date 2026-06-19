# Phase 1 - Minimap Static Layer Cache

## Phase Status

- [ ] Pending.

## Objective

Remove the recurring per-frame cost of redrawing static minimap terrain and static resource marks.
The minimap should still update dynamic entities, fog, viewport, and pings every frame as needed, but
the terrain tile loop should not run on every `requestAnimationFrame`.

## Work

- Add a minimap-owned static layer cache, preferably an offscreen canvas or similarly boring browser
  primitive, for terrain and any static resource marks that do not depend on per-frame entity state.
- Rebuild the static layer only when inputs change:
  - map object or terrain changes;
  - minimap canvas size, device pixel ratio, or CSS transform changes;
  - tile size, minimap transform, or color/style constants used by the layer change;
  - resource layout changes in a way that affects cached resource marks.
- Keep the current dynamic draw order recognizable. Terrain cache should draw first; dynamic entity
  blips, fog, visible resource treatment, viewport outline, and pings should still appear with the
  same semantics as before.
- Preserve minimap input transforms and hit testing. Rendering cache changes must not change click,
  drag, right-click, camera-jump, or command-target behavior.
- Add explicit teardown for any new canvas, GPU, or listener resources if the cache owns resources
  outside the existing minimap canvas.
- Keep implementation localized to the minimap area unless a small helper module is clearer and
  passes the client architecture checker.
- Record before/after evidence from the Matt/Alex replay and vehicle-wall stress workloads in the
  phase handoff. The key expected movement is a drop in `match.minimap` average/p95 and removal of
  terrain as a per-frame subphase cost.

## Expected Touch Points

- `client/src/minimap.js`
- possible new `client/src/minimap_static_layer.js` or similar UI-local helper
- `tests/minimap_input_contracts.mjs`
- `tests/client_contracts.mjs` if pure cache invalidation helpers are added
- `docs/perf-tracing.md` only if the operator-facing minimap interpretation changes

## Implementation Checklist

- [ ] Add a static minimap layer cache for terrain and safe static marks.
- [ ] Rebuild the cache on map/size/transform/style invalidation only.
- [ ] Keep dynamic minimap overlays live and visually ordered.
- [ ] Preserve minimap pointer and command interactions.
- [ ] Add focused tests or contract coverage for cache invalidation where practical.
- [ ] Run before/after browser perf harness workloads and save artifact paths in the handoff.
- [ ] Run verification and record exact results.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/minimap_input_contracts.mjs`
- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node scripts/client-perf-harness.mjs --workload matt-alex-replay --seconds 10`
- `node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 10`
- `git diff --check`

If docs change, also run:

```bash
node scripts/check-docs-health.mjs
```

## Manual Test Focus

Open a normal local match, a replay, and the vehicle-wall dev scenario. Confirm minimap terrain,
resource marks, unit/enemy blips, fog, pings, viewport outline, camera jumping, drag behavior, and
right-click minimap orders still behave normally.

## Handoff Expectations

Report the before/after `frame.work`, `match.minimap`, and `match.renderer` rows for each harness
workload. State exactly when the static layer is invalidated, whether any visual ordering changed,
and what minimap fog work remains for Phase 2.
