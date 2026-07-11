# Phase 12 - Instanced Vegetation

## Phase Status

- [ ] Not started.

## Depends On

- Phase 11.5 merged with stable benchmarks, instance policy, pools, and provisional budgets.

## Objective

Add deterministic vegetation through the measured instance path before shadows complicate the
scene. Drive wind from one scene-owned visual-clock uniform with correct instance world matrices
and no per-plant JavaScript animation. Measure the locked tier densities, fog policy, capture, and
cleanup in a stable scenario.

## Work

- Add stable benchmark scenario `vegetation` with Phase 0 map/seed/count/camera/fog/viewport/DPR and
  `off|low|medium|high` tier toggles. Extend schema v1 only with optional additive fields; otherwise
  create v2 under the Phase 11 schema policy.
- Implement shared source mesh/material vegetation through the Phase 11.5 hardware/thin instance
  route. Deterministic placement is presentation-only and cannot change gameplay, fog, selection,
  pathing, or command authority.
- Sample one renderer/scene-owned time uniform from the injected visual clock. No plant owns a
  timer, listener, callback, or per-frame JavaScript animation update.
- Apply instance world matrices before wind/world transforms in vertex code. Add non-identity
  translation, rotation, and scale contracts that fail if only the source mesh animates correctly.
- Apply locked density factors `off=0`, `low=.30`, `medium=.60`, `high=1.00`; record deterministic
  admitted counts and shared resource keys by tier. Shadow participation remains disabled until
  Phase 12.5.
- Define vegetation fog policy explicitly from Phase 9 categories. Placement may be static, but it
  cannot create a hidden-entity-derived marker, diagnostic, or fitted bound.
- Measure tier deltas for draws, instances, triangles, materials/textures, timing, and registry
  resources, and compare against Phase 11.5 reports using the stable comparison command.
- Exercise camera pan/dolly, resize/DPR, fixed/event capture, fog edge, view generation, reset,
  rematch, and destroy with automated resource baselines.
- Use `lab-interact` to capture normal-distance high-tier vegetation in the exact scenario and
  inspect one PNG once.

## Expected Touch Points

- Babylon vegetation instance/material/shader modules
- Phase 11 benchmark scenario/schema/diagnostics
- `tests/client_contracts/babylon_vegetation_contracts.mjs` (create it in this phase)
- dedicated Babylon browser vegetation/lifecycle coverage wired into the authoritative runner
- `tests/browser_babylon_vegetation.mjs`
- durable rendering docs/parity/budget ledger
- `plans/render3d/phase-12.md` status update in the implementation commit

## Quality Requirements

- Vegetation has zero per-instance JavaScript animation updates.
- Shader tests prove non-identity instance world matrices participate in final position.
- Tier counts follow the locked deterministic factors and shared keys remain bounded.
- View/fog generation changes cannot leave hidden-derived vegetation diagnostics or stale resources.

## Explicit Exclusions

- No shadow light/map/caster/proxy work; Phase 12.5 owns it.
- No finished environment art, representative unit GLB, faction work, WebGPU-only path, default
  switch, or universal FPS gate.

## Implementation Checklist

- [ ] Add the stable vegetation scenario and compatible schema fields.
- [ ] Implement instanced vegetation with one shared visual-clock uniform and matrix correctness.
- [ ] Apply locked tier densities and fog/view-generation policy.
- [ ] Measure/compare tier deltas and automated lifecycle baselines.
- [ ] Inspect one Lab Interact PNG and update durable docs/ledger/budgets.
- [ ] Mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/babylon_vegetation_contracts.mjs
    node tests/client_contracts/babylon_performance_contracts.mjs
    node scripts/rendering-benchmark.mjs --backend babylon --scenario vegetation --repeat 3 --output target/rendering-benchmarks/phase-12.json
    node scripts/compare-rendering-benchmarks.mjs --baseline target/rendering-benchmarks/phase-11.5.json --candidate target/rendering-benchmarks/phase-12.json
    node tests/browser_babylon_vegetation.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

At normal gameplay distance, compare locked vegetation tiers while panning/dollying and resizing.
Inspect wind across translated/rotated/scaled instances, fog-edge behavior, deterministic capture,
and readability; rely on automated counters and lifecycle assertions for correctness.

## Handoff Expectations

Report scenario/report hash, shader matrix evidence, tier counts/deltas, fog policy, resource/lifecycle
results, preview URL/command, and inspected PNG. Name Phase 12.5 as next and identify locked shadow
starting caps, caster/proxy admission, camera fit, stale-map clearing, tier degradation, and benchmark
integration.
