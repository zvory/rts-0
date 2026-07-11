# Phase 12.5 - Shadows and Quality Tiers

## Phase Status

- [ ] Not started.

## Depends On

- Phase 12 merged with measured instanced vegetation and tier diagnostics.

## Objective

Add a bounded camera-fitted directional-shadow manager through the resource registry. Prove
caster/proxy secrecy, immediate stale-map clearing, deliberate tier degradation, and stable
lifecycle before the representative asset uses the path. Do not add cascades or invent final
hardware promises.

## Work

- Add stable scenario `vegetation-shadows` using the Phase 0 exact setup and all locked tier toggles.
  Extend the benchmark schema under the Phase 11 compatibility policy.
- Own directional light, shadow map, caster/proxy records, and materials in the Phase 8 root/shared
  scopes. Define deterministic destroy/reset order and registry diagnostics.
- Start from locked tier caps: `off=0/0/never`, `low=512/32/every 4th frame`,
  `medium=1024/64/every 2nd frame`, and `high=2048/128/every frame` for map resolution/casters/update.
  Measurement may reduce optional caps, never increase them without a later reviewed plan.
- Admit casters only from current received visible `fogGatedWorld` presentation. Remembered,
  `visionOnly`/below-fog intel, and above-fog shot/event reveals do not cast; shadow proxies remain
  visual-only and never affect picking.
- Fit the directional map to bounded camera-visible ground/caster data without hidden ids/positions.
  Report admitted/rejected counts, proxy usage, map resolution, update decision, and fit bounds.
- Override tier cadence on any visibility removal, fog/view-generation change, reset, seek, or
  rematch: clear and render the shadow map before the next presented frame so a former caster leaves
  no stale shadow or fit-bound trace.
- Measure shadow off/on and tier deltas for draws, instances, triangles, materials/textures,
  caster/proxy counts, map work, timings, and registry resources. Core team identity, selection/HP,
  fog secrecy, and command readability never degrade with tier.
- Exercise pan/dolly, resize/DPR, fixed/event capture, fog edge, reset, rematch, context loss where
  practical, and destruction with automated baselines.
- Use `lab-interact` to capture the exact high-tier `vegetation-shadows` scenario at normal distance
  and inspect one PNG once.

## Expected Touch Points

- Babylon directional light/shadow manager and quality settings
- vegetation shadow participation and Phase 11 benchmark schema/diagnostics
- `tests/client_contracts/babylon_shadow_contracts.mjs` (create it in this phase)
- dedicated Babylon browser shadow/secrecy/lifecycle coverage wired into the authoritative runner
- `tests/browser_babylon_shadows.mjs`
- durable rendering docs/parity/budget ledger
- `plans/render3d/phase-12.5.md` status update in the implementation commit

## Quality Requirements

- Caster/proxy limits, map resolution, cadence, and fit work are bounded and observable by tier.
- Hidden, remembered, intel, and reveal presentation cannot cast or affect fit/diagnostics.
- Visibility/view-generation changes force an immediate clear/update regardless of tier cadence.
- Tiers trade optional detail only; tactical readability, selection, and secrecy remain.

## Explicit Exclusions

- No cascades, final device matrix, WebGPU-only path, finished environment art, faction work, or default switch.

## Implementation Checklist

- [ ] Add `vegetation-shadows` scenario/schema fields and locked starting tier caps.
- [ ] Implement registry-owned light/map/caster/proxy/camera-fit policy.
- [ ] Prove category secrecy and immediate stale-map clearing.
- [ ] Measure/compare off/on and tier deltas plus automated lifecycle baselines.
- [ ] Inspect one Lab Interact PNG and update durable docs/ledger/budgets.
- [ ] Mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/babylon_shadow_contracts.mjs
    node tests/client_contracts/babylon_visibility_contracts.mjs
    node tests/client_contracts/babylon_performance_contracts.mjs
    node scripts/rendering-benchmark.mjs --backend babylon --scenario vegetation-shadows --repeat 3 --output target/rendering-benchmarks/phase-12.5.json
    node scripts/compare-rendering-benchmarks.mjs --baseline target/rendering-benchmarks/phase-12.json --candidate target/rendering-benchmarks/phase-12.5.json
    node tests/browser_babylon_shadows.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Compare tiers at normal distance while panning/dollying and resizing. Inspect shadow stability,
proxy alignment, fog-edge caster removal, selection/readability, and capture; use automated checks
for stale-map clearing, secrecy, caps, and resource cleanup.

## Handoff Expectations

Report scenario/report hash, final tier table, caster/proxy/camera-fit policy, forced-clear proof,
off/on deltas, secrecy/lifecycle results, preview URL/command, and inspected PNG. Name Phase 13 as
next and identify the locked generated tracked-vehicle fixture, deterministic generation,
validation/integration, fallback comparison, event anchor, budget delta, and capture.
