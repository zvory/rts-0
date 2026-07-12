# Phase 10 - Generic Entities and Perspective Interaction

## Phase Status

- [ ] Not started.

## Depends On

- Phase 9.5 merged with authoritative fog/reveal secrecy and semantic layer categories.

## Objective

Prove that every current received entity and every targeting path works with the real perspective
adapter before adding representative overlays/effects or enabling ordinary routes. Use truthful
shared generic fallbacks and semantic selection/HP presentation in the controlled Babylon Lab
route. Keep asset geometry and backend nodes out of selection authority.

## Work

- Render every received current entity kind through a truthful generic fallback when no validated
  GLB exists. Share template geometry/materials from the start and keep the instance boundary
  compatible with Phase 11.5; do not create one source/material/texture per entity.
- Preserve team identity, facing, setup/construction state, relationship, and bounded missing-art
  readiness using only received presentation data. Record every fallback as `placeholder`, not
  parity.
- Derive a machine-readable expected-kind set from the authoritative client protocol/config catalog
  and assert every current entity kind resolves to a truthful fallback or validated asset route;
  a hand-maintained partial list is insufficient.
- Add semantic selection indication plus HP/progress for current selectable entities without
  deriving hit bounds, ownership, or selectability from meshes/assets.
- Exercise every Phase 2 projected entity-target path on the real Babylon camera: ordinary click,
  marquee admission, right-click attack/gather/repair classification, hover/command preview
  classification, armed entity-target ability, ctrl-in-viewport, control groups, and Lab entity click.
- Exercise nullable ground move, placement target, attack-ground/ability target, and Lab ground tool
  projection separately, but do not render their full overlays yet. Misses remain armed/cancel per
  existing intent semantics and never emit stale/non-finite coordinates.
- Validate minimap viewport polygon/recenter, spatial audio listener, control-group focus/viewport
  selection, resize/DPR, fixed capture, Lab reset/focus, freeze, and rematch.
- Cover replay and spectator presentation/no-control policy through isolated route/bundle contracts
  while the ordinary route gate remains closed. They may navigate and inspect allowed presentation,
  but cannot acquire command authority.
- Use `lab-interact` for a small authoritative fogged perspective scene with overlapping near/far
  entities, all major entity categories, selection/HP, ground hits/misses, and rematch; inspect one
  PNG once.
- Update durable docs/parity rows and keep placement/order/tactical/Lab-observer/effect visuals
  assigned to Phase 10.5.

## Expected Touch Points

- Babylon generic entity/template/instance modules
- Babylon selection/HP presentation
- semantic projection/picking integration through existing input contracts
- minimap/audio/control-group adapters through established semantics
- `tests/client_contracts/babylon_entity_contracts.mjs` (create it in this phase)
- `tests/client_contracts/babylon_interaction_contracts.mjs` (create it in this phase)
- controlled browser interaction/capture smoke coverage
- `tests/browser_babylon_interaction.mjs` wired into the authoritative browser runner
- durable rendering docs/parity ledger
- `plans/render3d/phase-10.md` status update in the implementation commit

## Requirements

- Generic fallbacks share source geometry/materials and remain compatible with Phase 11.5 routing.
- Entity-kind coverage is catalog-derived and fails when a newly added kind has no route.
- Mesh/asset geometry never changes selection, command classification, or entity-targeting results.
- All entity-target paths use the same projected proxy policy; ground rays serve only ground targets.
- Fog secrecy applies to fallbacks, selection/HP, picking, diagnostics, and capture.
- Replay/spectator tests prove no command authority; normal routes remain blocked.

## Explicit Exclusions

- No placement/order/tactical world visuals, real particle effect, Lab/observer world overlay, or route unlock.
- No full overlay/effect catalog, unit-specific animation parity, finished terrain, or faction art.
- No dense-scale budget claim, tuned batching/pool capacity, vegetation, shadows, default switch, or Pixi removal.

## Implementation Checklist

- [ ] Add shared truthful generic fallbacks for all received entity kinds.
- [ ] Add semantic selection/HP without mesh-derived picking.
- [ ] Validate every projected entity-target and nullable ground-target path.
- [ ] Validate minimap/audio/control groups/Lab/resize/capture/freeze/rematch.
- [ ] Prove replay/spectator no-control policy in isolated contracts.
- [ ] Update parity evidence, inspect one Lab Interact PNG, and mark this phase done.

## Verification

    node tests/client_contracts/babylon_entity_contracts.mjs
    node tests/client_contracts/babylon_interaction_contracts.mjs
    node tests/client_contracts/selection_projection_contracts.mjs
    node tests/minimap_input_contracts.mjs
    node tests/lab_interact_fixed_capture_contracts.mjs
    node tests/browser_babylon_interaction.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

In controlled Babylon Lab, test overlapping near/far click targets, marquee, all entity-target
classifications, ground hits/misses, control groups, minimap, audio, selection/HP, fog/reveals,
resize/DPR, fixed capture, reset, freeze, and rematch. Confirm missing art stays truthful and
selectable without a hidden or mesh-dependent hit; ordinary routes must still reject before join.

## Handoff Expectations

Report shared fallback design, entity-kind coverage, selection/HP, entity/ground targeting results,
subsystem/lifecycle checks, route-gate proof, exact preview URL/command, and inspected PNG. Name
Phase 10.5 as next and identify representative overlay categories, the real finite attack effect,
post-`START` failure cleanup, role policy, and the explicit experimental route gate.
