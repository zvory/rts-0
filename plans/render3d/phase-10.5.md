# Phase 10.5 - Overlay, Effect, and Route Gate

## Phase Status

- [ ] Not started.

## Depends On

- Phase 10 merged with truthful generic entities, selection/HP, and real perspective targeting.

## Objective

Complete the representative overlay/effect spine and then decide mechanically whether ordinary
experimental Babylon routes can be enabled. Add one complete path per core category without
absorbing the long-tail Pixi catalog. Preserve default Pixi and role-appropriate command authority.

## Work

- Implement the locked building placement footprint with valid/invalid state, move order line/
  destination plus entity-target marker, and selected-unit range ring from Phase 3 data.
- Implement the backend-neutral screen marquee and locked Lab tool area preview without importing
  Lab UI or transport into the renderer.
- Implement the normalized 240 ms attack/muzzle event as one real finite Babylon effect through
  Phase 8 scopes/pool interface and Phase 5 deterministic capture. It obeys Phase 9 fog/layer
  policy, never loops, preserves seed/payload/pose, and survives later recreation.
- Exercise placement/order/tactical/marquee/Lab-observer/effect presentation with selection/HP,
  fog/reveals, minimap, audio, control groups, resize/DPR, fixed capture, reset, freeze, and rematch.
- Add a route-gate function with explicit reasons. It permits only an explicit
  `rtsRenderer=babylon` after runtime/capability preflight; missing/invalid/default remains Pixi or a
  pre-join error as Phase 6 defines.
- Enable ordinary experimental live, replay, and spectator routes only after all Phase 9/9.5/10/10.5
  contracts pass. Live players retain existing command policy, replay/spectator remain command-free,
  and Lab retains metadata-driven policy.
- Distinguish pre-join from post-`START` failure. Dependency/capability/route failures stop before
  networking; map/fog/scene construction failure after the synchronous `START` payload fails closed,
  detaches match handlers, destroys the partial backend once, returns to a bounded lobby/error state,
  and never silently swaps renderer or processes later events through a partial scene.
- Keep all unmatched Pixi presentation rows explicit as `missing` or `deferred`; route enablement is
  experimental foundation coverage, not default-cutover parity.
- Use `lab-interact` for a small authoritative perspective scene containing the representative
  overlay categories, real finite effect, fog/reveal, and Lab/observer overlay. Capture at fixed
  offsets and inspect one PNG once.

## Expected Touch Points

- Babylon core world/screen overlay and finite-effect modules
- route/capability gate and app post-`START` failure cleanup
- presentation descriptors only where a representative category is absent
- Lab Interact backend launch/capture coverage
- `tests/client_contracts/babylon_overlay_contracts.mjs` (create it in this phase)
- `tests/client_contracts/babylon_route_gate_contracts.mjs` (create it in this phase)
- browser live/replay/spectator/Lab route smoke coverage wired into the authoritative runner
- `tests/browser_babylon_routes.mjs`
- durable rendering docs/parity ledger
- `plans/render3d/phase-10.5.md` status update in the implementation commit

## Route Gate Requirements

- Default/unset renderer remains Pixi and loads no Babylon bytes.
- Explicit live player, replay, spectator, and Lab routes receive only their existing authority and
  visibility policy.
- Pre-join failure opens no socket; post-`START` scene failure detaches and tears down exactly once.
- No route renders through a partial backend or falls back mid-match.
- Long-tail visual gaps remain visible in the parity ledger and do not masquerade as parity.

## Explicit Exclusions

- No full overlay/effect library, unit-specific animation parity, finished terrain, faction art, or default switch.
- No batching/pool tuning, benchmark budget claim, vegetation, shadows, or representative GLB.

## Implementation Checklist

- [ ] Add one complete placement, order/target, tactical, screen, and Lab/observer overlay path.
- [ ] Add the finite normalized attack/muzzle effect and deterministic capture.
- [ ] Add pre-join gate and fail-closed post-`START` cleanup contracts.
- [ ] Prove live/replay/spectator/Lab role and visibility policy.
- [ ] Wire browser route evidence into the authoritative runner and inspect one Lab Interact PNG.
- [ ] Update parity evidence and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/babylon_overlay_contracts.mjs
    node tests/client_contracts/babylon_route_gate_contracts.mjs
    node tests/client_contracts/babylon_interaction_contracts.mjs
    node tests/client_contracts/babylon_visibility_contracts.mjs
    node tests/client_contracts/presentation_capture_contracts.mjs
    node tests/lab_interact_fixed_capture_contracts.mjs
    node tests/browser_babylon_routes.mjs
    node scripts/check-client-architecture.mjs
    node tests/select-suites.mjs --verify
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

In controlled Lab, verify placement/order/tactical/marquee/Lab-observer paths and the real attack
effect at fixed offsets. Then explicitly launch one live player, replay, and spectator Babylon
route, confirm role policy and fog, force pre-join and post-`START` failures, and leave/re-enter;
default Pixi must remain unchanged and Babylon-free.

## Handoff Expectations

Report completed/deferred overlay rows, effect scope/capture offsets, route-gate reasons, role tests,
post-`START` cleanup, lifecycle counts, exact preview URLs/commands, and inspected PNG. Name Phase 11
as next and identify stable scenario ids, committed schema, launcher metadata, counter reset, and
unoptimized baselines.
