# Phase 2 - Remove Entity-Linear Rig and Frame-View Waste

## Phase Status

- [ ] Pending.

## Objective

Reduce CPU, allocation, and Pixi display-tree cost that grows directly with army size without
changing gameplay, art appearance, prediction, interpolation, fog authority, or input semantics.
This phase has two bounded lanes: construct and sample only the rig work actually needed, then
consolidate per-frame entity derivation so presentation, selection, and Pixi do not repeatedly copy
the same entity.

Phase 1's explicit-present timing and exact 200/300 workloads must be merged first. Use its artifacts
as the before measurement and do not change the workload while comparing this phase.

## Scope

### Route-specific rigs

- Construct SVG `UnitRigInstance` children only for the requested route parts and PNG children only
  for sprites intersecting the route.
- Compute one immutable `RigAnimationSample` per entity draw and share it across SVG, PNG, fallback,
  shadow, overlay, and effect routes.
- Replace the Tank atlas's transparent track suppressor sprites with explicit suppression/coverage
  metadata. Update the generator so future atlas-module regeneration preserves the metadata; do not
  commit regenerated image bytes.
- Cache atlas and palette subtextures at renderer/backend-root scope. Instances own sprites and
  containers, while shared textures are destroyed exactly once by renderer teardown.
- Preserve partial-atlas fallback, visual-profile overrides, frame-strip routing, shot reveal,
  recoil, setup transitions, muzzle anchors, pool eviction, failed-asset fallback, fixed capture,
  and rematch teardown.

### Consolidated frame entities

- Replace the three full `GameState.entitiesInterpolated()` passes used by
  `buildFrameEntityViews()` with one GameState-owned source pass.
- Preserve distinct visual, current, and authoritative meanings: predicted/interpolated render
  pose; latest predicted HUD/minimap pose; and latest non-predicted fog/observer pose.
- Derive selected entities from the same frame records and selection IDs without applying display
  overlays again.
- Keep `PresentationFrameV1` detached, recursively frozen, fog-filtered, and backend-neutral.
- Build `SelectionSceneV1` from already detached visible presentation entities. Reuse the frozen
  record or a compact allowlisted interaction subset instead of recursively copying the full entity
  again.
- Add only normalized motion/recoil fields that Pixi genuinely needs to the presentation contract,
  then remove per-entity adapter reads of `_curById`, `_prevById`, `weaponRecoil`, and
  `weaponRecoilPhase`.
- Encode visible/intel/reveal classification directly so the adapter does not spread-clone entities
  merely to restore marker booleans.
- Keep backend-local indexes that are genuinely useful. Deleting the entire compatibility adapter
  or rewriting unrelated legacy Pixi helpers is not required.

## Non-Goals

- No supply-cap, balance, protocol, server, simulation, or fog-rule change.
- No viewport culling or LOD; whole-map zoom remains the intended worst case.
- No Tank repaint, new asset, animation retuning, or visual-profile redesign.
- No mutable GameState or snapshot reference may cross the presentation boundary.
- Do not change standalone `GameState.entitiesInterpolated()` semantics for non-frame callers.
- No fog-grid, minimap, health-bar, selection-geometry, or trench caching; Phase 3 owns those.
- If full adapter deletion expands into unrelated feedback/building/effect conversion, retain a
  minimal facade and document it. Do not defer removal of pose/recoil rereads, intel/reveal spread
  clones, or the second selection deep copy.

## Structural Acceptance Targets

Pin these counts with deterministic inspection factories or diagnostics before modifying production
code. If current definitions have changed, document the new arithmetic and preserve the underlying
invariant: only route-requested resources exist and sampling occurs once.

| Case | Current baseline | Required after |
| --- | ---: | ---: |
| Tank rig-owned display objects including containers | 114 | no more than 14 |
| Tank animation samples per entity/frame | 4 | 1 |
| Tank part-state constructions per entity/frame | 140 | no more than 35 |
| Tank binding evaluations per entity/frame | 520 | no more than 130 |
| Tank atlas texture wrappers for 150 Tanks | 750 | 3 shared subtextures |
| Anti-Tank Gun display objects | 57 | no more than 9 |
| Mortar Team display objects | 61 | no more than 11 |
| Artillery display objects | 110 | no more than 56 |
| Mixed-alpha Match frame entity source work | 3 interpolation calls plus 1 selected call | 1 source pass |
| Selection interaction detachment | 1 recursive copy per selectable entity | 0 second recursive copies |
| Pixi legacy pose/recoil reads | per entity | 0 |

## Expected Touch Points

Rig lane:

- `client/src/renderer/rigs/runtime.js`
- `client/src/renderer/rigs/png_runtime.js`
- `client/src/renderer/rigs/animation.js`
- `client/src/renderer/rigs/live_routing.js`
- `client/src/renderer/rigs/png_routing.js`
- `client/src/renderer/rigs/tank_png_atlas.js`
- `client/src/renderer/units.js`
- `client/src/renderer/index.js`
- `scripts/art/tank-raster-pipeline.mjs`
- `tests/rig_runtime.mjs` and focused renderer lifecycle contracts

Frame lane:

- `client/src/state.js`
- `client/src/frame_entity_views.js`
- `client/src/frame_recovery.js`
- `client/src/presentation/frame.js`
- `client/src/input/selection_projection.js`
- `client/src/input/selection.js` only if the compact interaction contract requires it
- `client/src/renderer/pixi_compatibility_adapter.js`
- HUD, minimap, and observer consumers only where consolidated views require an update
- focused frame-entity, presentation, selection, adapter, state, input, HUD, and minimap contracts
- `docs/design/client-rendering.md` and `docs/design/client-ui.md`

## Ordered Implementation Work

1. Record exact current object, sampler, binding, texture, frame-view, selection-copy, and adapter
   counts in structural tests and capture Phase 1's 300-supply trace as the before artifact.
2. Make the route signature an immutable construction input for SVG and PNG instances. Rebuild only
   when kind, definition, atlas/resource, or route signature changes.
3. Compute one animation sample in the unit draw path and supply it to all active routes. Preserve a
   bounded fallback only for isolated tests/callers, not normal rendering.
4. Add explicit rendered/suppressed/missing atlas coverage. Regenerate only the checked-in Tank atlas
   module and prove tracks neither render nor fall back.
5. Move subtexture ownership to the renderer root. Prove instances share texture identities,
   destroying one instance cannot destroy shared resources, and renderer teardown destroys each
   shared subtexture exactly once.
6. Add a GameState-owned single-pass frame derivation API. Preserve angle interpolation, prediction
   smoothing, progress extrapolation, optimistic production/rally data, resources, newly visible
   entities, and shot-reveal/vision-only classification.
7. Resolve selection from those records and keep standalone state APIs for non-frame callers.
8. Assemble presentation records with the normalized motion/recoil/classification data Pixi needs,
   then build selection proxies from the already detached presentation records.
9. Remove adapter pose/recoil snapshotting and marker spread clones, and ratchet the exact legacy-read
   allowlist so those dependencies cannot return.
10. Update rendering design contracts, run focused and visual verification, and compare the exact
    Phase 1 workload before and after.

## Focused Automated Verification

Add tests proving route-only construction; one sample across Tank shadow/body/fuel/effects; exact
Tank, AT Gun, Mortar, and Artillery counts; absent transparent track sprites; shared subtexture
ownership; and correct teardown. Cover idle, low-oil, oil-starved, recoil, weapon facing, muzzle
anchors, shot reveal, visual overrides, and partial/failing atlas fallback.

For the frame path, cover mixed-alpha and alpha-1 views, prediction enabled/disabled, spectators,
newly visible entities, authoritative fog sources, immutable presentation, hidden-data sentinels,
compact selection interactions, resource/building/Tank Trap commands, Lab selection, failed-present
selection retention, and zero per-entity adapter pose/recoil reads.

Run:

```bash
node tests/rig_runtime.mjs
node tests/client_contracts.mjs
node tests/minimap_input_contracts.mjs
node scripts/check-client-architecture.mjs
node scripts/check-docs-health.mjs
node tests/select-suites.mjs --verify
tests/run-all.sh --only-browser-scenarios=smoke
git diff --check
```

Run the Phase 1 200/300 workload IDs without changing their scenario:

```bash
node scripts/client-perf-harness.mjs \
  --stress-matrix \
  --workload supply-200-active \
  --workload supply-300-active \
  --matrix-cpu 1,4 \
  --matrix-viewport default,large \
  --matrix-dpr 1,2 \
  --matrix-repeat 3 \
  --seconds 10 \
  --trace
```

Report full frame/update/present p95, frame-view/presentation/selection/adapter p50/p95, exact rig
counters, and allocation evidence. Structural targets and semantic equivalence are hard gates;
machine-local timing must not regress, but do not invent an FPS improvement when variance obscures
one.

## Interact Lab Manual Test

Use the project-local `interact` skill. In one small authoritative scene, inspect a non-axis-aligned
Tank at normal and zero oil, then fire it to check shadow, hull, turret, barrel recoil, muzzle flash,
fuel cue, and absence of legacy tracks. Include Anti-Tank Gun, Mortar, and Artillery states where
bounded Lab commands allow them.

Capture one clean Pixi PNG, inspect it once, reject missing textures, duplicate tracks, absent
shadow/cue, incorrect recoil, or one-frame route disappearance, close the session, and include only
the Tailnet Preview URL in the handoff.

## Player-Facing Outcome

No intended gameplay or art change. Large armies retain interpolation, prediction responsiveness,
selection/targeting, fog correctness, and visuals while using substantially fewer display objects,
animation evaluations, temporary copies, and texture wrappers.

## PR and Handoff Requirements

- Start only after Phase 1 is merged and reachable from `origin/main`; implement on a fresh
  `zvorygin/` branch and mark this phase Done in the implementation commit.
- Run `scripts/agent-pr.sh --verification "<focused checks and before/after evidence passed>"`, then
  `scripts/wait-pr.sh <pr>` and verify reachability before Phase 3.
- The handoff must provide exact before/after structural counts, frame-path timing/allocation
  evidence, retained adapter reads and why, prediction/fog/selection contract confirmation, the
  Interact Preview URL, artifact paths, and the core visual/input checks Phase 3 should repeat.
