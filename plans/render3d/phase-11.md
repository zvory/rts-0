# Phase 11 - Batching, Pools, and Benchmark Harness

## Phase Status

- [ ] Not started.

## Depends On

- Phase 10 merged with instance-compatible generic entities, one finite effect, and core overlay paths.

## Objective

Measure and bound the production backend before adding vegetation, shadows, or representative art.
Create named reproducible scenarios and a stable JSON report, then implement shared mesh/material/
instance policies and capacity-bounded effect pooling. Establish provisional structural and same-
device regression budgets with correctly reset current-frame counters.

## Work

- Add stable scenario ids using authoritative Lab/dev setup where applicable:
  `quiet`, `dense-placeholders`, `active-effects`, `fog-overlays`, and `lifecycle`. Record map/seed,
  entity/effect counts, camera view, visibility perspective, and required quality flags.
- Add the stable command:

      node scripts/rendering-benchmark.mjs --backend babylon --scenario <id> \
        --output target/rendering-benchmarks/<id>.json

  The launcher owns warmup/sample duration, private server/browser setup, metadata collection, and
  teardown; generated JSON remains under `target/` and is not committed.
- Define `target/rendering-benchmarks/schema-v1.json` output fields in committed code/docs: commit,
  scenario/map/seed, browser/version, GPU/backend, viewport/DPR, quality, warmup/samples, median/p95
  total frame and `scene.render`, current-frame draw calls, meshes, hardware/thin instances,
  triangles, materials, textures/estimated memory, active/pooled particles, and registry live/
  pending counts.
- Explicitly reset/advance Babylon internal counters once per authoritative Match frame. Test that a
  static scene reports stable per-frame draws instead of accumulation and label cumulative counters
  separately if retained.
- Classify content routes: unique hierarchy, hardware instance, thin instance, merged static mesh,
  or pool. Enforce shared source mesh/material/texture keys and category-level diagnostics; convert
  generic dense fallbacks to the appropriate route without changing selection ids/proxies.
- Implement bounded effect pool capacity, overflow fallback/drop policy, complete state reset, and
  active/pooled diagnostics using Phase 8 ownership. Repeated pooled events cannot retain previous
  pose, owner, visibility, clock, callback, or seed.
- Calibrate provisional scenario budgets from same-device repeated comparisons. CI gates structural
  invariants and relative toggles, not absolute FPS/wall-clock values from shared runners.
- Add a bounded repeated lifecycle benchmark and verify canvas/rAF/listener/context/registry/pending
  load counts return to baseline. Phase 13 owns the final longer ten-cycle gate.
- Use `lab-interact` with explicit `RTS_CLIENT_DIR` to capture the dense-placeholder/active-effect
  scene once and inspect the artifact, confirming batching/pooling preserves truthful presentation.
- Update durable performance methodology in `docs/design/client-rendering.md` and budget/status
  evidence in `docs/design/rendering-parity.md`.

## Expected Touch Points

- `scripts/rendering-benchmark.mjs` and committed report schema/metadata helpers
- authoritative benchmark scenario definitions
- Babylon instance/material/template and effect-pool modules
- frame profiler/backend diagnostics
- `tests/client_contracts/babylon_performance_contracts.mjs`
- `tests/client_contracts/rendering_benchmark_contracts.mjs`
- durable rendering docs/parity ledger
- `plans/render3d/phase-11.md` status update in the implementation commit

## Budget Requirements

- PoC counts are historical observations, never target/baseline.
- Every number includes scenario, tier, viewport/DPR, counter definition, warmup/sample method, and environment.
- Structural budgets cover draws/materials/shared resources/instances/pool capacity where deterministic.
- Frame timing is same-device evidence until target-device rollout creates a later release gate.
- New content must report category deltas and cannot silently create per-entity materials/textures/draw paths.

## Explicit Exclusions

- No vegetation, shadows, shadow counters, representative GLB, faction content, or default switch.
- No universal FPS promise and no committed generated reports/screenshots.

## Implementation Checklist

- [ ] Add five stable scenarios, benchmark command, schema, metadata, and teardown.
- [ ] Reset/test current-frame Babylon counters.
- [ ] Define/implement shared unique/instance/thin/merged routing for generic content.
- [ ] Add bounded effect pool capacity/overflow/reset diagnostics.
- [ ] Calibrate provisional structural/same-device budgets and lifecycle baseline.
- [ ] Inspect one dense-placeholder/active-effect Lab Interact artifact.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/babylon_performance_contracts.mjs
    node tests/client_contracts/rendering_benchmark_contracts.mjs
    node scripts/rendering-benchmark.mjs --backend babylon --scenario quiet --output target/rendering-benchmarks/quiet.json
    node scripts/rendering-benchmark.mjs --backend babylon --scenario dense-placeholders --output target/rendering-benchmarks/dense-placeholders.json
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Run all five scenarios twice on the same device, compare metadata/counters, and inspect dense
fallback selection/readability plus repeated pooled effects. Confirm static draws do not accumulate,
pool reset is complete, shared keys stay bounded, and lifecycle counts return to baseline; report
the exact preview command/URL and inspected artifact path.

## Handoff Expectations

Report scenario ids/commands, schema version, environment, provisional budgets, per-category routing,
pool policy, current-frame reset proof, lifecycle counts, generated report paths, exact preview
command/URL, and inspected artifact path. Name Phase 12 as next and identify
instanced shader world matrices/shared time, vegetation tier density, shadow caster/proxy admission,
quality toggles, camera fitting, and benchmark integration.
