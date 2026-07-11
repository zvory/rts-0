# Phase 9 - Authoritative Fog and Reveal Secrecy

## Phase Status

- [ ] Not started.

## Depends On

- Phase 8 merged with proven shared-resource ownership and lifecycle diagnostics.

## Objective

Prove the highest-risk gameplay invariant before interaction overlays or content: Babylon cannot
reveal an entity, position, target, event, memory update, or diagnostic the recipient was not
authorized to receive. Implement the semantic terrain/fog/remembered/reveal layer spine using
registry-owned resources. Validate live, replay, spectator, and Lab timelines with programmatic
no-leak assertions and one deterministic fog-edge capture.

## Work

- Define semantic layer categories in `RendererFrame`: static ground, persistent ground marks,
  ordinary fog-gated world presentation, authoritative current fog, explored/remembered
  presentation, explicit below/above-fog intel/reveals, tactical feedback, and screen overlays.
  Do not copy Pixi container names as the cross-backend contract.
- Render map/terrain boundary and current visible/explored fog from Phase 3 grids/revisions using
  Phase 8 shared textures/materials. Bound uploads/allocations by revision and make resource
  ownership/readiness visible.
- Render remembered buildings only from the explicit received memory model. They are visually
  distinct and contain no current hidden HP, queue, target, animation, effect, or movement data;
  reconciliation/expiry follows current client semantics.
- Preserve explicit reveal policies: `visionOnly`/legacy intel remains at its documented fog layer,
  while shot/event reveals appear above fog only when the recipient's `RendererFrame` includes the
  explicit reveal and only for its normalized lifetime. Never resolve a hidden source id.
- Gate every backend surface, not just mesh visibility: geometry, selection candidates, diagnostics,
  capture metadata, labels, particles, lights, future shadow admission, bounds, and resource keys.
- Add contrasting recipient/timeline fixtures and programmatic assertions that hidden ids/positions
  never enter Babylon renderer inputs or diagnostic output. Prefer real server-projected fixtures
  where existing test infrastructure permits.
- Cover replay seek/vision perspective, spectator union view, Lab vision/reset, resize, detached
  fixed capture, freeze, rematch, and resource teardown. A generation change cannot retain old fog
  texture/event/memory state.
- Remove Phase 6's controlled-Lab-only runtime gate only after the no-leak contracts pass. Enable
  namespaced experimental live/replay/spectator routes deliberately and keep failure before join if
  fog initialization/readiness cannot provide the required semantics.
- Use `lab-interact` to arrange visible, explored, unseen, remembered, below-fog intel, and explicit
  reveal cases at a fog edge. Capture deterministically and inspect one PNG once.
- Update `docs/design/client-rendering.md` and `docs/design/rendering-parity.md` with the layer/fog
  contract, evidence, and remaining overlay work.

## Expected Touch Points

- Phase 3 frame/layer descriptors
- `client/src/renderer_babylon/terrain.js`
- `client/src/renderer_babylon/fog.js`
- remembered/reveal presentation modules and resource diagnostics
- replay/Lab reset and capture-readiness hooks
- `tests/client_contracts/babylon_fog_contracts.mjs`
- `tests/client_contracts/babylon_visibility_contracts.mjs`
- browser fog/capture smoke coverage
- durable rendering docs/parity ledger
- `plans/render3d/phase-9.md` status update in the implementation commit

## Security Requirements

- Babylon receives only least-privilege renderer data; no `GameState`, transport, full snapshot, or
  authoritative fog-source subview reaches the module.
- Invisible objects cannot cast, pick, label, light, emit, diagnose, or retain a hidden position.
- Remembered presentation is historical received data, never a live hidden entity view.
- Above-fog presentation requires an explicit normalized reveal semantic.
- No-leak failures block the phase regardless of visual quality.

## Explicit Exclusions

- No generic all-kind entities, selection/HP, placement/order overlays, or real Babylon effect;
  Phase 10 owns interaction presentation.
- No batching/benchmark optimization, shadows, vegetation, representative GLB, or full Pixi parity.
- No renderer-local visibility stamping or protocol change.

## Implementation Checklist

- [ ] Define semantic layers and implement revisioned terrain/current/explored fog.
- [ ] Implement remembered buildings and explicit below/above-fog reveal policies.
- [ ] Gate diagnostics/capture/picking/future caster/effect surfaces against hidden data.
- [ ] Add contrasting recipient/timeline no-leak contracts.
- [ ] Cover replay/spectator/Lab/reset/capture/rematch resources and inspect one fog-edge PNG.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/babylon_fog_contracts.mjs
    node tests/client_contracts/babylon_visibility_contracts.mjs
    node tests/client_contracts/presentation_frame_contracts.mjs
    node tests/lab_interact_fixed_capture_contracts.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Inspect fog edges while a received entity/reveal enters and leaves vision, a remembered building,
legacy intel, replay/spectator perspectives, Lab reset, resize, fixed capture, freeze, and rematch.
Look specifically for hidden geometry, labels, picking hits, resource keys, diagnostics, capture
metadata, or stale memory, not only visible silhouettes.

## Handoff Expectations

Report semantic layer order, fog update/upload policy, remembered/reveal semantics, no-leak fixtures
and results, resource baselines, exact preview command/URL, and inspected artifact. Name Phase 10 as
next and identify instance-compatible generic fallbacks, entity targeting, selection/HP,
placement/order/tactical overlays, real finite effect, Lab/observer overlay, and perspective input.
