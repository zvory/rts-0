# Phase 10 - Core Interaction and Overlay Spine

## Phase Status

- [ ] Not started.

## Depends On

- Phase 9 merged with authoritative fog/reveal secrecy and semantic layer categories.

## Objective

Prove that the shared camera, selection, frame, event, resource, and fog contracts support real RTS
interaction in Babylon without porting the long-tail Pixi catalog. Add truthful generic entity
fallbacks and one representative path in each core overlay category. Exercise real perspective
entity targeting and ground interaction across normal, replay, spectator, and Lab lifecycles.

## Work

- Render every received current entity kind through a truthful generic fallback route when no
  validated GLB exists. From the start, use shared template meshes/materials and ordinary instances
  or another Phase 11-compatible instance boundary; do not create one mesh source/material/texture
  per entity that the next phase must rewrite.
- Preserve team identity, facing, setup/construction/remembered distinctions, and bounded missing-
  asset readiness. Fallbacks reflect only received presentation data and are ledgered as
  placeholders, not parity.
- Implement semantic selection indication plus HP/progress for current selectable entities without
  deriving picking bounds from meshes/assets.
- Implement one complete placement/invalid-placement path, move/order/target feedback path, and
  range/rally or equivalent tactical ground-overlay path through Phase 3 frame data.
- Implement one real finite Phase 4 event effect through Phase 8 scopes/pool interface and Phase 5
  deterministic capture. It must obey Phase 9 fog/layer policy and never loop for demonstration.
- Implement one Lab or observer world overlay without importing Lab UI/transport, plus the
  backend-neutral screen marquee.
- Exercise all Phase 2 projected entity-targeting paths on the real Babylon camera: selection,
  right-click attack/gather/repair, hover/command preview, armed entity-target ability, and Lab
  entity click. Exercise nullable ground move/placement/tool paths separately.
- Validate minimap viewport polygon/recenter, spatial audio listener, control-group focus/viewport
  selection, resize/DPR, replay/spectator, Lab reset/focus/capture, freeze, and rematch.
- Update each parity row with completed/representative/placeholder/missing/deferred plus automated
  and visual evidence. Do not absorb unit-specific overlays or finished effects/art.
- Use `lab-interact` for a small authoritative perspective scene containing selection/HP,
  placement/order/tactical feedback, the real finite effect, fog/reveal, and Lab/observer overlay;
  deterministic capture and inspect one PNG once.

## Expected Touch Points

- Babylon generic entity/template/instance modules
- Babylon core world/screen overlay and finite-effect modules
- presentation-frame descriptors only where a representative category is absent
- semantic camera/minimap/audio integration through existing contracts
- `tests/client_contracts/babylon_overlay_contracts.mjs`
- `tests/client_contracts/babylon_interaction_contracts.mjs`
- real browser interaction/capture smoke coverage
- durable rendering docs/parity ledger
- `plans/render3d/phase-10.md` status update in the implementation commit

## Requirements

- Generic fallbacks share source geometry/materials and remain compatible with Phase 11 batching.
- Mesh/asset geometry never changes selection or entity-targeting results.
- All current targeting paths use the same projected proxy policy; ground rays serve only ground targets.
- Effects come from normalized real events and scoped resources.
- Fog secrecy applies to fallbacks, overlays, effects, picking, diagnostics, and capture.

## Explicit Exclusions

- No full overlay/effect catalog, unit-specific animation parity, finished terrain, or faction art.
- No dense-scale budget claim, thin-instance conversion, tuned pool capacity, vegetation, or shadows.
- No default switch or Pixi removal.

## Implementation Checklist

- [ ] Add instance-compatible truthful generic fallbacks for all received entity kinds.
- [ ] Add selection/HP and representative placement/order/tactical/screen overlay paths.
- [ ] Add one real finite scoped effect and one Lab/observer overlay.
- [ ] Validate every projected entity-target and nullable ground-target path in Babylon.
- [ ] Validate minimap/audio/control groups/replay/spectator/Lab/resize/capture/rematch.
- [ ] Update parity evidence and inspect one Lab Interact PNG.
- [ ] Mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/babylon_overlay_contracts.mjs
    node tests/client_contracts/babylon_interaction_contracts.mjs
    node tests/client_contracts/selection_projection_contracts.mjs
    node tests/minimap_input_contracts.mjs
    node tests/lab_interact_fixed_capture_contracts.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

At near/far perspective positions, test selection, overlapping targets, marquee, every entity-target
classification, ground move/placement, control groups, minimap, audio, core overlays, real effect,
fog/reveals, replay/spectator, Lab reset/focus/capture, DPR/resize, freeze, and rematch. Confirm
missing art stays truthful and selectable without a hidden or mesh-dependent hit.

## Handoff Expectations

Report shared fallback template/material/instance design, completed versus ledgered overlays,
entity/ground targeting results, effect scope/capture, subsystem/lifecycle checks, exact preview URL/
command, and inspected artifact. Name Phase 11 as next and identify scenario ids, benchmark command/
schema, counter reset, shared/unique instance categories, pool capacity/reset, and provisional budgets.
