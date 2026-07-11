# Phase 0 - Contract, Inventory, and Baselines

## Phase Status

- [ ] Not started.

## Objective

Create the durable source of truth and evidence ledger that later executor phases can follow
without reading the disposable PoC branch or reconstructing decisions from chat. Record the
production rendering boundary, camera/projection semantics, presentation-frame policy, ownership
model, coordinate and asset expectations, parity categories, benchmark scenarios, and separate
content-expansion/default-cutover gates. This is a documentation-only, runtime-neutral architecture
phase: it must not add Babylon runtime code or change current Pixi behavior.

## Work

- Add `docs/design/client-rendering.md` and route it from the client context capsule/index. Keep
  `docs/design/client-ui.md` authoritative for the existing client module
  contracts, linking the new document instead of duplicating large current-Pixi descriptions.
- Record the non-negotiable boundaries: one shared client, one active backend, `Match`-owned rAF
  and visual clock, server world pixels, renderer-neutral input/selection, renderer-owned GPU
  resources, already-filtered events/fog, Babylon-free default loading, and Pixi default status.
- Inventory raw camera-representation consumers using current `main`, including match/frame logic,
  minimap, audio, input/control groups, Lab Interact, diagnostics, observer overlays, visual
  samples, capture, resize, replay, and carryover. Distinguish backend-private orthographic math
  from application/UI/input reads that Phase 1 must remove.
- Inventory the complete current Pixi presentation catalog from the documented layer order and
  implementation: terrain, decals, trenches, resources, remembered buildings, entities, rigs,
  shadows, fog, shot reveals, selection/HP, ability/smoke/effects, command and placement feedback,
  observer/Lab overlays, screen marquee, capture/readiness, diagnostics, and external DOM/minimap
  surfaces.
- Create `docs/design/rendering-parity.md` as the active parity ledger with explicit statuses such
  as `shared external`, `Pixi complete`, `Babylon complete`, `representative`, `placeholder`,
  `missing`, and `deferred`. Each Babylon
  transition must cite focused automated evidence and, where visible, an inspected artifact; do
  not equate placeholder coverage with parity.
- Mark separately which rows are required before broad content waves and which are required only
  before making Babylon the default. Fog/event secrecy, camera/input, lifecycle, capture, asset
  validation, and scale budgets belong to the first gate; long-tail visual parity and browser/device
  rollout remain default-cutover work.
- Inventory ownership and lifecycle classes: application/backend/scene, shared dependency, cached
  source asset, material/texture/shader, entity instance, effect instance, pool, shadow resource,
  listener, canvas/context, timer/rAF, and late async load. Record allowed disposer and parent scope
  for each class.
- Define named benchmark scenario contracts using existing authoritative Lab/dev setup mechanisms:
  quiet representative view, dense generic army, instanced vegetation, active finite effects,
  fog/core overlays, and repeated rematch. Record required metadata and counters, but defer actual
  Babylon budgets until Phases 11 and 12 measure the production backend.
- Treat the user-supplied PoC metrics and defects as non-binding findings. Do not fetch the PoC
  branch, copy its modules, or present its one-machine counts as current-main baselines.

## Expected Touch Points

- `docs/design/client-rendering.md`
- `docs/design/client-ui.md`
- `docs/context/client-ui.md`
- `docs/context/README.md`
- `docs/doc-map.json` if the new source-of-truth document needs routing
- `docs/design/rendering-parity.md`
- `plans/render3d/phase-0.md` status update in the implementation commit

## Required Contract Decisions

- Public semantic camera/projection operations and the legacy view-restore compatibility policy.
- Plain-data selection proxy ownership and screen-space marquee semantics.
- Static map presentation versus per-rAF presentation frame ownership.
- Event identity/seed/lifetime and fixed-capture replay rules.
- Backend factory lifecycle, failure, late-load, freeze, resize, reset, and destroy semantics.
- World/scene scale, axis, handedness, and facing convention ownership, leaving numeric choices to
  the validated Phase 7 implementation if current evidence is insufficient.
- Resource scope hierarchy and the rule that a child never disposes a shared dependency.
- Parity status definitions, evidence fields, content-expansion gate, and default-cutover gate.
- Scenario metadata and diagnostic fields required before a performance claim is accepted.

## Explicit Exclusions

- No production or experimental renderer code.
- No dependency download, vendor asset, GLB, shader, particle, fog, or camera behavior change.
- No PoC branch inspection or code archaeology.
- No performance target invented from the PoC count or a single unrecorded local run.
- No claim that the initial inventory is permanent; later phases update the durable document when
  implementation evidence improves it.

## Implementation Checklist

- [ ] Add and route the durable rendering design document.
- [ ] Record all cross-phase invariants and semantic contracts needed by Phase 1.
- [ ] Create the parity/evidence ledger with both migration gates.
- [ ] Inventory raw camera consumers, Pixi presentation capabilities, and lifecycle resources.
- [ ] Define reproducible scenario contracts and required diagnostics.
- [ ] Record supplied PoC findings as non-binding evidence without inspecting its branch.
- [ ] Run docs validation and mark this phase done in the implementation commit.

## Verification

    node scripts/check-docs-health.mjs
    node scripts/check-client-architecture.mjs
    git diff --check

Confirm every new link resolves, the context capsule remains within its size policy, and the parity
ledger distinguishes foundation coverage from default-cutover parity.

## Manual Test Focus

No runtime behavior changes. Manually review the inventory against a normal match, replay/Lab
launch, current Pixi layer order, minimap, and fixed-capture entry points to catch omitted contract
consumers; do not collect new Babylon screenshots in this phase.

## Handoff Expectations

Name the exact durable design and ledger paths, summarize unresolved contract choices, and identify
the raw camera consumers Phase 1 must migrate. State that runtime code was unchanged and name Phase
1 as the next work, with Pixi camera/navigation/minimap/audio/Lab behavior as its core manual test
focus.
