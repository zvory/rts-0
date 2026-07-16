# Phase 1 - Crude World Readability

## Phase Status

- [ ] Not started.

## Objective

Make the authoritative map and every current entity understandable in Babylon using only simple
geometry, flat colors, and labels. Repair only the data boundary required to do that safely.

## Work

- Stop passing the shared Pixi `sources` bag into Babylon construction. Allow only the Babylon
  dependency, canvas parent, an intentionally scoped instrumentation hook if retained, and
  `StaticMapPresentationV1` through a small detached renderer-neutral seam keyed by its existing
  revision; Babylon continues to receive dynamic state only through `PresentationFrameV1`.
- Render the terrain grid with a minimal color/material taxonomy that distinguishes every
  gameplay-relevant terrain or obstacle class. Render oil, steel, and any other current resource
  site distinctly; reconcile static location/identity and dynamic availability/depletion records
  by id into one visual.
- Give every current unit, building, and resource kind a stable readable primitive taxonomy. A
  box, wedge, cylinder, flat label, or combination is sufficient; show team/relationship, body
  facing, weapon facing where relevant, setup/deployed and loaded/active state, selection, HP, and
  construction/deconstruction/production progress.
- Keep all interaction on the shared semantic selection/projection path. Missing or malformed
  presentation data produces a bounded generic marker or dropped-record diagnostic.
- Update the rendering parity ledger and focused contracts, then use Interact to capture and inspect
  one mixed authoritative scene at ordinary gameplay distance.

## Expected Touch Points

- backend construction and detached static-map delivery
- `client/src/renderer/babylon/` terrain/resource/entity presentation
- Babylon boundary, static-map, entity-readability, lifecycle, and architecture contracts
- `docs/design/client-rendering.md` only if the static-map contract shape changes
- `docs/design/rendering-parity.md` and this phase status

## Acceptance

- Babylon cannot reach mutable game state or the Pixi compatibility `sources` bag.
- A player can distinguish terrain, resources, owners, all current entity kinds, facing, and core
  entity state without relying on art assets.
- Existing fog, selection, camera, loop, and teardown behavior remains intact.

## Verification and Manual Test

Run the focused Babylon boundary/static-map/entity contracts, the client architecture check, and
the selected browser smoke. In Interact, inspect a mixed scene containing terrain, both resource
types, representative units/buildings for all current gameplay categories, construction/production,
selection, and multiple owners.

## Handoff

Report the static-map seam, removed mutable sources, primitive taxonomy, fallback behavior, checks,
and inspected capture. Tell the next agent to preserve that vocabulary while adding gameplay zones
and feedback, and name any entity state that remained ambiguous during the manual test.
