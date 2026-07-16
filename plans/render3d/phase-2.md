# Phase 2 - Crude Gameplay Feedback

## Phase Status

- [ ] Not started.

## Depends On

- Phase 1 merged with detached static-map delivery and a readable primitive battlefield.

## Objective

Make every existing normal-player action and gameplay-significant world state understandable in
Babylon without recreating Pixi styling or adding new effects.

## Work

- Render trenches, smoke, and ability objects with crude ground shapes or volumes. Preserve their
  existing fog/layer policy and never reconstruct hidden state.
- Render rallies, queued orders, ranges and minimum ranges, firing arcs, support-weapon setup,
  placement validity, resource targeting, ability targeting, and Lab tool previews using generic
  lines, rings, wedges, fills, and labels. Preserve existing ownership/control, selection,
  setup/deployed, settings, and fog/layer gates from `feedbackContext` and the presentation layer.
- Render consequential entity-backed status cues already shown by Pixi, including active ability
  auras, loaded/active weapon state, and tank low-oil or oil-starved feedback. Extend the detached
  shared presentation data only for an existing cue that cannot be expressed from current records;
  never read mutable economy state from Babylon.
- Consume the existing smoke-canister, mortar, artillery, panzerfaust, muzzle-flash, miss, command,
  and impact feedback records. Reuse an existing checked-in effect/decal image or SVG on a flat
  plane/billboard when it is directly suitable; otherwise map the record to a tiny reusable
  vocabulary such as launch line, projectile marker, target marker, impact flash, and short text.
  Use the shared visual clock and the record's existing lifetime rather than adding a backend loop
  or new event schema.
- Verify production/rally, mining, construction, movement, attack, targeted abilities, support
  weapons, and combat outcomes are visually explainable at ordinary play distance.
- Update the rendering parity ledger and focused contracts, then capture and inspect one
  authoritative fogged scenario covering movement, building, combat, and a targeted ability.

## Expected Touch Points

- `client/src/renderer/babylon/` gameplay-object and feedback presentation
- existing `PresentationFrameV1` records; extend shared data only if an existing Pixi behavior is
  otherwise impossible to represent safely
- Babylon feedback, lifetime, fog, architecture, and browser contracts
- `docs/design/client-rendering.md` only for an actual shared-contract change
- `docs/design/rendering-parity.md` and this phase status

## Acceptance

- Every existing normal-player gameplay feedback category and entity-backed status cue supplied to
  Pixi has a truthful, correctly gated generic Babylon representation.
- Current commands and abilities do not require blind clicks, and consequential world changes do
  not happen without a visible explanation.
- No GLB, bespoke weapon effect, new gameplay feature, asset-conversion pipeline, generalized pool,
  or backend-owned clock is introduced.

## Verification and Manual Test

Run focused Babylon feedback/fog/lifetime contracts, the client architecture check, and the
selected browser smoke. In Interact, exercise construction and rallying, movement and queued
orders, smoke or an ability object, a setup/target preview, and combat feedback across a fog edge.

## Handoff

Report the generic visual vocabulary, covered record categories, any shared-data change, checks,
and inspected capture. Tell the next agent exactly which live-match flows to play and identify only
remaining issues that could prevent normal live development.
