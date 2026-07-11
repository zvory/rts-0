# Phase 11 - Benchmark Harness and Counter Semantics

## Phase Status

- [ ] Not started.

## Depends On

- Phase 10.5 merged with generic entities, representative overlays/effect, and experimental routes.

## Objective

Make performance evidence reproducible before changing batching or pool policy. Create the stable
scenarios, launcher, committed schema, current-frame counter semantics, comparison command, and
teardown checks. Record unoptimized current-main baselines without turning them into permanent
budgets.

## Work

- Materialize the exact Phase 0 scenario contracts as `quiet`, `dense-placeholders`,
  `active-effects`, `fog-overlays`, and `lifecycle`, including authoritative setup/map/seed, counts,
  camera, perspective, viewport/DPR, quality flags, and expected readiness assertions.
- Add the stable launcher:

      node scripts/rendering-benchmark.mjs --backend babylon --scenario <id|all> \
        --repeat 3 --output target/rendering-benchmarks/<run>.json

  It owns a private server/browser, warmup/sample policy frozen in Phase 0, metadata, schema
  validation, teardown, and nonzero failure when Chrome/Babylon/readiness is unavailable.
- Commit the report schema at `scripts/rendering-benchmark.schema-v1.json`. Reports cite its id and
  version and live only under ignored `target/rendering-benchmarks/`; add the specific ignore rule.
- Report commit, scenario/map/seed, browser/version, GPU/backend/runtime, viewport/DPR, quality,
  warmup/sample/repetition, median/p95 total frame and `scene.render`, current-frame draws, meshes,
  hardware/thin instances, triangles, materials, textures/estimated memory, particles, and registry
  live/pending counts.
- Reset/advance Babylon internal instrumentation exactly once per authoritative Match frame. Test a
  static scene across multiple frames so current-frame draws remain stable while explicitly named
  cumulative counters increase.
- Add `scripts/compare-rendering-benchmarks.mjs --baseline <report> --candidate <report>` that
  rejects schema/environment/scenario mismatches and emits structural/timing deltas. Timing
  comparisons are evidence only and never a shared-runner absolute gate.
- Record SHA-256 plus summarized counters for every ignored report in the handoff/PR body so review
  evidence survives worktree cleanup.
- Add an automated same-page lifecycle scenario covering repeated Match construct/destroy and exact
  canvas/rAF/listener/context/registry/pending-load baselines; Phase 13.5 extends it to ten cycles.
- Use `lab-interact` to capture the unoptimized dense/effect scene once and inspect the artifact.
- Update durable methodology and ledger with counter definitions and unoptimized baselines.

## Expected Touch Points

- `scripts/rendering-benchmark.mjs`
- `scripts/compare-rendering-benchmarks.mjs`
- `scripts/rendering-benchmark.schema-v1.json`
- authoritative benchmark scenario definitions
- Babylon frame profiler/backend diagnostics
- `.gitignore`
- `tests/client_contracts/babylon_performance_contracts.mjs` (create it in this phase)
- `tests/client_contracts/rendering_benchmark_contracts.mjs` (create it in this phase)
- dedicated Babylon browser benchmark/lifecycle coverage wired into `tests/run-all.sh`/CI
- durable rendering docs/parity ledger
- `plans/render3d/phase-11.md` status update in the implementation commit

## Measurement Requirements

- PoC counts are historical leads, never a target or baseline.
- Every number includes scenario, tier, viewport/DPR, counter definition, warmup/sample/repetition,
  environment, and schema version.
- Optional additive schema fields remain v1; renamed, removed, type-changed, or newly required
  fields require v2 and an explicit reader compatibility policy.
- Generated reports never enter Git, but hashes/summaries enter the durable handoff.
- Browser/backend unavailability is a failed required check, not a successful skip.

## Explicit Exclusions

- No batching/content-route rewrite, effect-pool tuning, or provisional optimized budget; Phase 11.5 owns them.
- No vegetation, shadows, representative GLB, faction content, default switch, or universal FPS promise.

## Implementation Checklist

- [ ] Add five exact scenarios, all/repeat launcher, committed schema, metadata, and teardown.
- [ ] Reset/test current-frame versus cumulative Babylon counters.
- [ ] Add comparable-report validation and delta output.
- [ ] Run three repetitions, hash/summarize reports, and record unoptimized baselines.
- [ ] Automate repeated same-page lifecycle baselines and inspect one Lab PNG.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/babylon_performance_contracts.mjs
    node tests/client_contracts/rendering_benchmark_contracts.mjs
    node scripts/rendering-benchmark.mjs --backend babylon --scenario all --repeat 3 --output target/rendering-benchmarks/phase-11.json
    node scripts/compare-rendering-benchmarks.mjs --baseline target/rendering-benchmarks/phase-11.json --candidate target/rendering-benchmarks/phase-11.json
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Inspect the dense/effect capture for truthful presentation and review report metadata/counter names,
not aesthetic parity. Confirm static current-frame draws do not accumulate and automated lifecycle
counts return to baseline; batching/pool changes remain Phase 11.5.

## Handoff Expectations

Report scenario definitions, commands/schema, environment, report paths/hashes/summaries,
current-frame reset proof, comparison output, lifecycle counts, exact preview command/URL, and
inspected PNG. Name Phase 11.5 as next and identify content routes, shared keys, pool capacity/reset,
optimized deltas, and provisional budget formula.
