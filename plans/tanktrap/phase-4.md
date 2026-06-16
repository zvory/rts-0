# Phase 4 - Client Mirror, Rendering, and Preview Policy

Status: Done.

## Goal

Teach the client to understand, render, and preview Tank Traps without yet exposing the final line
placement command surface.

## Scope

- Confirm client `KIND.TANK_TRAP`, compact kind decode, and `STATS` mirror from Phase 1 are current.
- Add a simple readable Czech hedgehog renderer in `client/src/renderer/buildings.js` or a focused
  helper chosen by the existing renderer structure.
- Ensure Tank Trap construction progress, health, selection, hover, minimap, and remembered/scouted
  display remain consistent with other small buildings.
- Update advisory placement validation for Tank Trap to follow server blocker semantics:
  - terrain and map bounds still matter
  - ordinary buildings/resources and vehicle-body units should block placement
  - infantry overlap should not make a Tank Trap site invalid if the server policy allows infantry
    to stand on the same tile
  - existing preview remains advisory and server arrival-time validation remains authoritative
- Create a client placement policy helper, for example `placementPolicyForBuilding(kind)`, so
  ordinary structures and Tank Trap preview share terrain/bounds/resource/building checks while
  differing on unit overlap. Vehicle-body classification must mirror the server movement-body class;
  do not embed a Tank Trap-only unit list in input code.
- Keep Tank Trap hidden from normal worker build-card exposure until Phase 5.
- Add focused client tests for metadata, rendering-adjacent descriptors if available, and advisory
  placement behavior.

## Expected Deliverables

- Snapshots containing Tank Trap render without errors.
- The placeholder visual is identifiable at normal zoom and distinct from Supply Depot and
  resource nodes.
- Client placement preview uses the same high-level blocker policy as the server for Tank Trap.
- Worker build-card UI still does not expose Tank Trap.

## Out of Scope

- Line-placement drag UX.
- Build-card button exposure.
- Protocol changes beyond existing kind support.
- Sound effects.

## Verification

- Run focused Node tests for client metadata, placement preview, renderer contracts, and protocol
  parity touched by this phase.
- Run `node scripts/check-client-architecture.mjs` if new client modules or cross-area imports are
  added.

## Manual Testing Focus

Use a dev/debug snapshot or scenario containing Tank Traps. Confirm they render, selection/health
works, construction progress is readable, and the placement ghost treats infantry overlap
appropriately.

## Handoff Expectations

The handoff must describe the renderer approach, placement-preview policy, tests run, and the exact
client seams Phase 5 should use for line placement and build-card exposure.
