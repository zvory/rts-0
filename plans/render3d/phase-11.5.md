# Phase 11.5 - Batching, Pools, and Provisional Budgets

## Phase Status

- [ ] Not started.

## Depends On

- Phase 11 merged with stable scenarios, report schema/comparison, counter reset, and baselines.

## Objective

Apply deliberate content routing and bounded effect pooling against measured baselines. Preserve
selection ids, event semantics, resource ownership, and truthful fallbacks while reducing structural
cost. Produce provisional optimized budgets from the frozen formula rather than executor judgment.

## Work

- Classify each content path as unique hierarchy, hardware instance, thin instance, merged static
  mesh, or pool. Enforce shared source mesh/material/texture keys and category diagnostics; route
  generic dense fallbacks without changing selection proxies or ids.
- Implement bounded effect-pool capacity and overflow fallback/drop policy through Phase 8 ownership.
  Reset pose, owner, visibility, clock, callback, seed, event id/payload, fog/view generation, and
  diagnostics on every return; pool return is not dependency disposal.
- Add pure fake and browser tests for shared key counts, route choice, overflow, double return,
  late event completion, and every reset field.
- Run all five scenarios three times before/after on the same environment. Reject incomparable
  metadata and publish category deltas plus report hashes/summaries.
- Set each deterministic structural ceiling to
  `maxObserved + max(1, ceil(maxObserved * 0.10))` across the three optimized repetitions. Record
  timing medians/p95 and flag same-device regressions over 20%, but do not fail CI on timing alone.
- Derive pool capacity from the Phase 0 active-effects scenario's declared maximum simultaneous
  events; overflow behavior is deterministic and tested rather than sized from an incidental run.
- Re-run the same-page lifecycle benchmark and confirm shared keys, pool, registry, canvas, rAF,
  listener, context, and pending-load counts return to baseline.
- Use `lab-interact` to capture the optimized dense/active-effect scene and inspect one PNG.
- Update durable route policy, provisional structural budgets, timing evidence, and ledger status.

## Expected Touch Points

- Babylon instance/material/template routing and effect-pool modules
- Phase 11 benchmark comparison/report summaries
- performance/pool/resource contracts
- dedicated Babylon browser performance/lifecycle coverage
- durable rendering docs/parity budget ledger
- `plans/render3d/phase-11.5.md` status update in the implementation commit

## Budget Requirements

- Structural ceilings use the exact formula above and include scenario/tier/viewport/DPR/counter definition.
- Timing is a same-device warning/evidence field until a later target-device rollout plan.
- New content reports category deltas and cannot silently create per-entity materials/textures/draw paths.
- Pool reset and lifecycle baselines are automated correctness gates, not manual observations.

## Explicit Exclusions

- No vegetation, shadows, representative GLB, faction content, final target-device matrix, or default switch.

## Implementation Checklist

- [ ] Define/enforce unique/instance/thin/merged/pool routes and shared keys.
- [ ] Add bounded pool capacity/overflow and complete reset diagnostics.
- [ ] Compare three before/after repetitions and populate formula-based budgets.
- [ ] Re-run lifecycle baselines and inspect one optimized Lab PNG.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/babylon_performance_contracts.mjs
    node tests/client_contracts/babylon_resource_contracts.mjs
    node scripts/rendering-benchmark.mjs --backend babylon --scenario all --repeat 3 --output target/rendering-benchmarks/phase-11.5.json
    node scripts/compare-rendering-benchmarks.mjs --baseline target/rendering-benchmarks/phase-11.json --candidate target/rendering-benchmarks/phase-11.5.json
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Inspect dense fallback selection/readability and repeated pooled effects. Review automated route,
shared-key, pool-reset, budget, comparison, and lifecycle output; do not manually estimate
performance from the capture.

## Handoff Expectations

Report category routing, shared keys, pool capacity/overflow/reset, before/after report paths and
hashes, formula-based ceilings, timing warnings, lifecycle counts, preview command/URL, and inspected
PNG. Name Phase 12 as next and identify vegetation scenario, instance matrices, shared time, tier
density, fog policy, capture, and cleanup.
