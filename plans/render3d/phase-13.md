# Phase 13 - Representative GLB and Foundation Gate

## Phase Status

- [ ] Not started.

## Depends On

- Phase 12 merged with measured batching/pooling, vegetation, shadows, quality tiers, and secrecy.

## Objective

Validate exactly one production-representative neutral asset through every foundation contract, then
decide whether broad incremental content work is safe. Run the full scenario/lifecycle evidence,
update durable ledgers, and issue `go`, `revise`, or `stop` with exact blockers. Babylon remains
opt-in and Pixi remains default regardless of the recommendation.

## Work

- Choose exactly one neutral articulated vehicle or structure fixture, not a faction wave. Check in
  its reproducible source/provenance/license, GLB, and Phase 7 manifest within recorded budgets.
- Validate runtime scale/pivot/axes, visual bounds/anchors, independent turret/weapon or equivalent
  part, team-color material, declared clip/part transform, muzzle/effect anchor, visible mesh,
  shadow proxy, fallback, and capture readiness. Gameplay selection continues to use Phase 2 data.
- Load/instantiate/share/remove/recreate the asset through Phase 8 scopes and Phase 11 route. Report
  incremental draw/material/texture/triangle/memory/caster/registry/timing deltas versus its generic
  fallback in the same scene/device/tier.
- Drive one real authoritative event through its declared anchor and deterministic capture. Prove
  fog/reveal secrecy, effect lifetime, pool/resource ownership, and later-effect survival.
- Run every stable Phase 11/12 scenario with recorded environment metadata and generated JSON under
  `target/rendering-benchmarks/`. Do not commit reports or screenshots.
- Run at least ten full Babylon enter/leave or equivalent root-destroy cycles including asset load,
  entity/effect/fog/shadow/vegetation allocation, capture, reset, resize, and rematch. Registry,
  canvas, rAF, listener, context, pending load, pool, and shadow counts return to documented baseline.
- Audit `docs/design/client-rendering.md` and `docs/design/rendering-parity.md`. Every row has current
  status/evidence plus content-expansion versus default-cutover requirement; placeholder is not parity.
- Evaluate the content-expansion gate from `plan.md` and report `go`, `revise`, or `stop` first.
  `Revise` names bounded remediation phases; `stop` names the failed premise/evidence. Neither
  recommendation authorizes default rollout, faction conversion, or Pixi retirement.
- Use `lab-interact` to capture the representative asset at normal gameplay distance in a real
  fog/core-overlay/event/shadow scene, inspect one PNG once, and report its absolute path.

## Expected Touch Points

- one neutral representative source/GLB/manifest/provenance asset directory
- Babylon asset/entity/articulation/team/effect/shadow integration
- full benchmark/lifecycle launcher and diagnostics
- `tests/client_contracts/babylon_foundation_contracts.mjs`
- asset validator/resource/fog/interaction/performance/shadow/capture contracts
- `docs/design/client-rendering.md`
- `docs/design/rendering-parity.md`
- `plans/render3d/phase-13.md` status update in the implementation commit

## Content-Expansion Gate

A `go` requires current evidence that:

- shared consumers no longer depend on raw orthographic camera representation;
- real perspective entity targeting, marquee, ground commands, minimap, listener, and framing work;
- default Pixi loads no Babylon code/bytes and Match is the sole rAF owner;
- renderer frames/events are least-privilege and fog/event secrecy is proven;
- deterministic short-effect capture works from a detached revision without extended TTL;
- coordinates, asset validation/fallback, and resource ownership contracts pass;
- mass placeholders/effects/vegetation/shadows have bounded sharing/pooling/tier policies;
- counters are per-frame, scenario reports are reproducible, and provisional budgets are current;
- the representative GLB validates articulation/anchors/team/shadow/effect/resource/budget behavior;
- ten lifecycle cycles return every owned count to baseline; and
- remaining work is explicit and classified for content expansion versus default cutover.

## Explicit Exclusions

- No second representative asset, faction conversion, broad content wave, or finished-art requirement.
- No Babylon default/cohort, Pixi freeze/deletion, rollback retirement, or final browser/device matrix.
- No public deploy requirement, universal FPS promise, WebGPU requirement, elevation/physics, or unapproved scope expansion.

## Implementation Checklist

- [ ] Validate/check in exactly one representative neutral asset and manifest/provenance.
- [ ] Integrate articulation/team/anchor/effect/shadow/fallback without asset-driven selection.
- [ ] Measure asset-versus-placeholder deltas in the same scenario/device/tier.
- [ ] Run all scenarios and ten-cycle root lifecycle with baseline counts.
- [ ] Audit/update durable contract, parity evidence, gates, and remaining risks.
- [ ] Capture/inspect one final Lab Interact PNG.
- [ ] Record `go`, `revise`, or `stop` and mark this phase done in the implementation commit.

## Verification

    node scripts/validate-rendering-assets.mjs --all
    node tests/client_contracts/babylon_foundation_contracts.mjs
    node tests/client_contracts/babylon_resource_contracts.mjs
    node tests/client_contracts/babylon_visibility_contracts.mjs
    node tests/client_contracts/babylon_interaction_contracts.mjs
    node tests/client_contracts/babylon_performance_contracts.mjs
    node tests/client_contracts/babylon_shadow_contracts.mjs
    node tests/lab_interact_fixed_capture_contracts.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser
    git diff --check

Run every benchmark scenario through `scripts/rendering-benchmark.mjs` and record generated report
paths in the handoff. GitHub's `Main test gate` remains the authoritative repository-wide suite.

## Manual Test Focus

At normal gameplay distance, test representative articulation/facing/team color/anchors, selection,
ground/entity targeting, fog/reveals, real effect capture, shadow proxy, quality tiers, resize/DPR,
replay/spectator/Lab, fallback, and repeated rematches. Review every scenario report and ten-cycle
resource baseline before accepting the recommendation.

## Handoff Expectations

Lead with `go`, `revise`, or `stop`. Include asset/provenance/manifest paths, scenario commands and
report paths, same-device placeholder/asset deltas, provisional budgets, ten-cycle counts, parity
rows/gates, exact preview URL/command, inspected PNG, remaining risks, and core manual checks. State
explicitly that Babylon remains opt-in, Pixi remains default, and broad content/default-cutover/
retirement each require a separately reviewed future plan after the manual final review.
