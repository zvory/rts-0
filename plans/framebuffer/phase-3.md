# Phase 3 - Sample Each Rig Once and Apply Sparse Routes

## Phase Status

- [ ] Not started.

## Objective

Compile stable unit-rig route decisions once, build one animation sample/render context per entity
per frame, and apply that sample only to the parts owned by each active route. Remove repeated full
definition sampling, PNG coverage discovery, excluded-part scans, temporary sets/route objects,
and avoidable sample garbage while preserving the exact current Pixi visuals and fallback behavior.

## Evidence

After aggregating call sites, `sampleRigAnimation` is 9.5% self CPU,
`UnitRigInstance.update` is 4.8%, `pngAtlasRouteCoverage` is 3.5%, and garbage collection is 3.6%.
The renderer redraws 340 units per frame but performs about 1,742 rig redraw attempts and 13,214
route-hidden checks. `_drawUnit` rediscovers PNG coverage and constructs covered/missing route
objects per entity, while each PNG/SVG route instance independently samples the complete rig and
iterates records that the route excludes.

## Scope

### Compile immutable route plans

- Compile a bounded route plan for each loaded `(kind, definition, atlas/frame-strip availability,
  route configuration)` generation. Include ordered pool/layer ownership, PNG-covered sprite/part
  ids, SVG fallback part ids, shadow/body/overlay partitions, and the inactive pools to destroy.
- Invalidate plans only when their definition, atlas, visual override, or asset generation changes.
  Do not key a global cache by arbitrary user data or retain renderer-owned textures after destroy.
- Remove steady-state `pngAtlasRouteCoverage`, `normalizedPartSet`, route object spreads,
  `activePoolNames` construction, and repeated route membership checks from `_drawUnit`.
- Preserve partial-atlas fallback: covered parts use the PNG route, missing parts use the SVG route,
  and a missing/late texture still follows the current bounded fallback/recovery path.

### Share one animation sample per entity

- Create the render context once for each entity and sample its definition once after the route plan
  is known. Pass the same non-escaping sample into shadow, PNG, SVG fallback, overlay, and other
  applicable route instances.
- Compile stable animation bindings by part/property so the sampler does not clone every base
  transform/pivot/paint object on every call. Use renderer-owned reusable scratch or bounded pooled
  sample records that are reset before reuse and never retained by a display object.
- Make route instances own or directly iterate only their covered display records. A part excluded
  by a static route is not a per-frame hidden event and should not be visited merely to set
  `visible=false` again.
- Continue applying dynamic `visible`, transform, alpha, tint, and geometry-scale changes every
  frame where required. Retain unchanged draw-key skips and avoid redrawing vector geometry whose
  paint/scale key did not change.
- Use Phase 1's aggregated diagnostics to record animation samples, active route applications,
  actual dynamic-hidden parts, draw-key hits/misses, plan creation/invalidation, and instance
  create/reuse/destroy without restoring inner-loop observer overhead.

### Preserve lifecycle and visual contracts

- Keep pool ownership, z/layer order, team palette selection, setup/deploy animation, recoil,
  vehicle motion, trench scale, shot reveal alpha, shadows, overlays, and frame-strip behavior.
- A route/sample failure remains isolated to the affected entity and uses the existing
  missing-texture fallback. Reusable scratch must be left valid for the next entity/frame after an
  exception.
- Clear route plans, pooled samples, and instances idempotently on renderer destroy and asset/map
  generation changes.

## Expected Touch Points

- `client/src/renderer/units.js`
- `client/src/renderer/rigs/animation.js`
- `client/src/renderer/rigs/runtime.js`
- `client/src/renderer/rigs/png_runtime.js`
- rig registry/asset helpers that own definition or atlas generations
- focused rig animation, PNG/SVG fallback, route, pooling, teardown, renderer, and profiler tests
- `docs/design/client-ui.md` or `docs/design/client-rendering.md` only if an existing durable
  renderer/lifecycle boundary changes
- `docs/perf-tracing.md` for the final diagnostics and comparison method

Do not introduce a second renderer loop, Web Worker/OffscreenCanvas path, new GPU backend, wire
field, simulation change, or visual LOD/culling policy in this phase.

## Verification

Add deterministic tests proving:

- one animation sample per rendered entity per frame even when PNG, SVG fallback, shadow, and
  overlay routes are all active;
- route coverage is compiled once per stable asset generation and invalidates on a real asset or
  visual-override change;
- route instances contain/visit only owned records while dynamic hidden parts still update;
- partial atlas, late texture, missing texture, frame strip, setup weapon, recoil, vehicle motion,
  trench occupancy, shot reveal, team tint, failure recovery, and destroy/recreate behavior remain
  correct;
- reusable samples do not leak one entity's transforms, visibility, tint, or alpha into another.

Run:

```bash
node tests/client_contracts.mjs
node scripts/check-client-architecture.mjs
node scripts/client-flamegraph.mjs --preview
node scripts/client-flamegraph.mjs --workload supply-300-active --seconds 15 --preview
node scripts/check-docs-health.mjs
git diff --check
```

Retain final Hellhole and active CPU profiles, ranked summaries, and harness summaries using the
same settings and machine as the Phase 1/2 comparisons.

## Acceptance Evidence

- Animation sample count is no greater than the number of rendered unit entities in a frame.
- Stable route coverage performs no per-entity steady-state recomputation; plan creation and
  invalidation are bounded asset-generation events.
- Static route exclusions no longer account for thousands of per-frame hidden checks. Dynamic
  visibility counters still reflect actual animated hiding.
- The final ranked profile materially reduces `sampleRigAnimation`, rig-runtime update,
  `pngAtlasRouteCoverage`, and garbage-collector sampled time compared with the Phase 2 profile.
- Renderer update and complete frame average/p95 improve without regressing Pixi present cost,
  unattributed work, visual correctness, or lifecycle behavior. Record the exact machine-local
  comparison; do not add timing thresholds to CI.
- Both the unchanged client-only stream and the authoritative active-player workload pass their
  setup contracts before sampling.

## Manual Visual Test

Use the project-local `interact` skill to arrange one authoritative Pixi scene containing infantry,
a Tank, a moving vehicle, an anti-tank gun or mortar in setup transition, a building, and visible
combat feedback. Inspect the scene, capture one clean 1000x700 DPR 1 PNG at a representative
animation state, inspect it once, reject blank/stale/missing-texture output, close the session, and
include only its Tailnet Preview URL in the handoff. Manually watch motion, setup/deploy, facing,
recoil, shadows, selection/HP overlays, team colors, camera movement, and one leave/re-enter cycle.

## Measured Checkpoint, PR, and Handoff

Mark this phase Done in the implementation commit. Run
`scripts/agent-pr.sh --verification "<rig contracts, both final profiles, and visual review passed>"`,
then `scripts/wait-pr.sh <pr>` and verify the phase head is reachable from `origin/main`. The helper
should archive the completed plan in the final PR follow-up commit.

The final handoff must include compiled-plan and sample ownership, invalidation and teardown rules,
one-sample evidence, all focused checks, the Phase 1/2/final profile table, remaining top self and
inclusive functions, both final workload assertions, the Interact Preview URL, and core manual test
results. Stop after this checkpoint for user review; do not turn deferred presentation, detachment,
minimap, worker, framebuffer, or device-matrix ideas into new phases without a fresh profile.
