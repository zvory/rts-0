# Phase 0 - Contract, Inventory, and Baselines

## Phase Status

- [ ] Not started.

## Objective

Create the durable source of truth and evidence ledger that later executor phases can follow
without recovering the intentionally deleted PoC implementation or reconstructing decisions from
chat. Record the production rendering boundary, camera/projection semantics, presentation-frame policy, ownership
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
  from application/UI/input reads that Phases 1, 1.5, and 1.75 must remove.
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
  Babylon budgets until Phases 11/11.5 and 12/12.5 measure the production backend.
- Treat the unverified PoC observations copied into `plan.md` as non-binding leads. The
  implementation branch was deleted intentionally: do not recover it from history, reflogs,
  caches, old worktrees, PR patches, artifacts, or another clone, and do not present its
  one-machine counts as current-main
  baselines.

## Expected Touch Points

- `docs/design/client-rendering.md`
- `docs/design/client-ui.md`
- `docs/context/client-ui.md`
- `docs/context/README.md`
- `docs/doc-map.json` if the new source-of-truth document needs routing
- `docs/design/rendering-parity.md`
- `plans/render3d/phase-0.md` status update in the implementation commit

## Decision Freeze Exit Gate

Phase 0 may not be marked done while a later executor still needs an unspecified product/design
choice. Freeze the following in `docs/design/client-rendering.md` and the ledger:

- Exact semantic camera/projection API names/shapes, including `{x,y,heightPx}`, CSS-pixel units,
  nullability, snapshot/restore/versioning, the perspective audio reference-distance formula, and
  the legacy view-restore compatibility policy.
- Plain-data selection proxy ownership, presentation-height sources, deterministic hit ordering,
  and screen-marquee semantics.
- Static map versus per-rAF ownership and the revisioned immutable `GridSnapshot` accessor/copy
  mechanism; do not leave a borrowed-array policy for Phase 3 to invent.
- The locked semantic layer ids/order from `plan.md` and the exact descriptor fields later phases
  may extend compatibly.
- Event identity/seed/lifetime, the 240 ms attack/muzzle fixture, 256-event/10-second history, and
  frozen-presentation capture rules.
- Backend factory lifecycle, `rtsRenderer` selector, pre-join versus post-`START` failure, late-load,
  freeze, resize, reset, and destroy semantics.
- Resource scope hierarchy and the rule that a child never disposes a shared dependency.
- Exact parity status/evidence fields, content-expansion gate, and default-cutover gate.
- Exact benchmark scenario ids plus authoritative setup/map/seed, entity/effect counts, camera,
  viewport/DPR, tier, warmup/sample/repetition policy, counter definitions, and budget formula.
- The locked deterministic repository-authored tracked-vehicle fixture specification from `plan.md`.

An intentionally deferred numeric implementation value such as Phase 7 scene scale must name the
owning later phase and a deterministic decision rule. Otherwise Phase 0 returns `blocked` and does
not mark its status done.

## Explicit Exclusions

- No production or experimental renderer code.
- No dependency download, vendor asset, GLB, shader, particle, fog, or camera behavior change.
- No PoC implementation recovery, code archaeology, or asset reuse.
- No performance target invented from the PoC count or a single unrecorded local run.
- No claim that the initial inventory is permanent; later phases update the durable document when
  implementation evidence improves it.

## Implementation Checklist

- [ ] Add and route the durable rendering design document.
- [ ] Record all cross-phase invariants and semantic contracts needed by Phase 1.
- [ ] Create the parity/evidence ledger with both migration gates.
- [ ] Inventory raw camera consumers, Pixi presentation capabilities, and lifecycle resources.
- [ ] Define reproducible scenario contracts and required diagnostics.
- [ ] Record the plan-copied PoC leads as unverified/non-binding without recovering implementation.
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

Name the exact durable design and ledger paths, confirm the decision-freeze gate has no unresolved
later-phase choice, and identify the raw camera consumers owned by Phases 1/1.5/1.75. State that
runtime code was unchanged and name Phase 1 as next, with the semantic core/fake adapters as its
focus.
