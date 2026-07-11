# Phase 12 - Vegetation, Shadows, and Quality Tiers

## Phase Status

- [ ] Not started.

## Depends On

- Phase 11 merged with stable benchmark scenarios, per-frame counters, instance policy, and budgets.

## Objective

Add the two highest-value/highest-risk scene systems only after measurement and ownership exist.
Implement instanced shader-driven vegetation and a bounded directional-shadow manager with
deliberate quality degradation. Integrate both into the stable benchmark harness and prove visual
stability, instance correctness, resource cleanup, and expected relative counter changes.

## Work

- Add stable benchmark scenario `vegetation-shadows` with fixed vegetation/caster counts, camera
  path/view, map/seed, fog/core overlays, and quality-tier toggles. Extend schema v1 compatibly or
  version it if new fields cannot be optional.
- Implement shared source mesh/material vegetation through hardware/thin instances according to
  Phase 11 policy. Wind uses one renderer/scene-owned time uniform sampled from the injected visual
  clock, never one JavaScript update per plant.
- Make vertex shaders explicitly apply instance world matrices before wind/world transforms. Add
  contracts with non-identity translation/rotation/scale so a shader that animates only the source
  mesh cannot pass.
- Implement quality-tier vegetation density, wind enable/complexity, shadow participation, and
  disablement. Deterministic placement does not change gameplay/fog authority and resets cleanly.
- Add a directional shadow manager owning light/shadow resources through Phase 8. Define map
  resolution, caster admission/limit, proxy preference, material/alpha policy, camera-fit bounds,
  update cadence, and diagnostics.
- Gate caster admission on received/visible presentation policy so a hidden entity cannot cast or
  enter diagnostic bounds. Shadow proxies remain visual only and never affect picking.
- Define named quality tiers with deliberate reductions in shadow resolution/casters/update work
  and vegetation density/wind/shadows. Team identity, selection/HP, fog secrecy, and command
  readability cannot be disabled.
- Measure shadows off/on and vegetation tier deltas with per-frame draws, instances, triangles,
  materials/textures, caster/proxy counts, map size/updates, timings, and registry resources.
- Start with a measured camera-fitted directional strategy. Do not add cascades unless results show
  the simpler policy fails and the user approves the scope expansion.
- Exercise camera pan/dolly, resize/DPR, fixed capture, fog edge, reset, rematch, and destruction.
  Use `lab-interact` to capture normal-distance tier/shadow evidence and inspect one PNG once.

## Expected Touch Points

- Babylon vegetation instance/material/shader modules
- Babylon directional light/shadow manager and quality settings
- Phase 11 benchmark scenarios/schema/diagnostics
- fog visibility/caster admission integration through established presentation data
- `tests/client_contracts/babylon_vegetation_contracts.mjs`
- `tests/client_contracts/babylon_shadow_contracts.mjs`
- durable rendering docs/parity/budget ledger
- `plans/render3d/phase-12.md` status update in the implementation commit

## Quality Requirements

- Vegetation has zero per-instance JavaScript animation updates.
- Shader tests prove instance world matrices participate in final position.
- Shadow caster/proxy limits and update work are bounded and observable by tier.
- Hidden entities cannot cast, affect fitted bounds, or appear in shadow diagnostics.
- Tiers trade optional detail/performance only; core tactical readability and secrecy remain.

## Explicit Exclusions

- No cascades without separately approved evidence, no final target-device matrix, no WebGPU-only path.
- No finished environment art, broad terrain conversion, representative unit GLB, or faction work.
- No universal FPS gate or default switch.

## Implementation Checklist

- [ ] Add stable vegetation/shadow benchmark scenario and schema fields.
- [ ] Implement instanced vegetation with one shared visual-clock uniform and world-matrix correctness.
- [ ] Add bounded shadow manager, caster/proxy policy, camera fit, tiers, and diagnostics.
- [ ] Prove hidden-caster secrecy and expected relative tier/counter changes.
- [ ] Exercise lifecycle/fixed capture and inspect one Lab Interact PNG.
- [ ] Update durable docs/ledger/budgets and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/babylon_vegetation_contracts.mjs
    node tests/client_contracts/babylon_shadow_contracts.mjs
    node tests/client_contracts/babylon_performance_contracts.mjs
    node scripts/rendering-benchmark.mjs --backend babylon --scenario vegetation-shadows --output target/rendering-benchmarks/vegetation-shadows.json
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

At normal gameplay distance, compare tiers while panning/dollying and resizing. Inspect wind across
translated/rotated/scaled instances, shadow stability/proxy alignment, fog-edge caster secrecy,
selection/readability, fixed capture, reset, rematch, and correct registry cleanup. Confirm reported
relative density/caster/map work follows tier settings.

## Handoff Expectations

Report scenario command/report, shader instance-matrix evidence, tier table, caster/proxy/camera-fit
policy, off/on deltas, secrecy/lifecycle results, exact preview URL/command, and inspected artifact.
Name Phase 13 as next and identify the sole representative GLB, full pipeline/budget delta, ten-cycle
lifecycle, all scenario runs, ledger audit, and go/revise/stop recommendation.
