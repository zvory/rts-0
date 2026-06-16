# Phase 7 - Vehicle Migration

## Phase Status

- [ ] Not implemented.

## Objective

Migrate all vehicle-body units to SVG-authored rigs and make the rig renderer cover every unit
kind.

## Work

- Author and validate SVG rigs for Scout Car, Command Car, and Tank.
- Implement hull-facing vs weapon-facing separation for vehicles.
- Implement track or wheel phase animation bindings from the renderer-local movement visual state.
- Implement recoil bindings for Tank and Scout Car weapons.
- Preserve vehicle-specific details:
  Tank fuel cue, Command Car breakthrough ring attachment, Scout Car gunner/weapon readability,
  vehicle shadow shape, selection bounds, and hp bar position.
- Add equivalence samples for multiple facings, weapon-facing offsets, movement phase deltas,
  recoil values, low/oil-starved states, breakthrough ticks, and shot-reveal alpha.
- Require both semantic anchor/bounds comparison and bounded pixel or command comparison for each
  migrated vehicle kind.
- Confirm there are no remaining live unit kinds routed through legacy procedural drawing.

## Expected Touch Points

- SVG rig source files for vehicle units.
- Rig animation bindings for hull, turret/weapon, tracks/wheels, and special cues.
- `client/src/renderer/units.js`
- `client/src/renderer/shared.js`
- Temporary equivalence tests and fixtures.
- `plans/svg/phase-7.md`

## Implementation Checklist

- [ ] Add SVG rigs for vehicle units.
- [ ] Add hull/weapon-facing and movement-phase bindings.
- [ ] Add vehicle special-cue bindings.
- [ ] Route all remaining unit kinds through rig renderer.
- [ ] Add equivalence samples for vehicle static and animation states.
- [ ] Confirm legacy unit draw code is no longer used in normal gameplay.
- [ ] Run verification and record exact results.

## Verification

- Rig schema and SVG importer tests.
- Focused vehicle equivalence tests.
- Full temporary all-unit equivalence suite.
- `node scripts/check-client-architecture.mjs`
- `git diff --check`.

## Manual Test Focus

Run a local match or replay with tanks, scout cars, and command cars. Check hull turning,
weapon-facing, recoil, track/wheel motion, fuel cue, breakthrough ring, team tint, selection, hp
bars, fog/shot reveal, and rematch teardown.

## Handoff Expectations

Confirm every unit kind is rig-rendered in normal gameplay, name all remaining legacy files or
functions used only by the equivalence harness, and state whether Phase 8 can delete the duplicate
renderer path.
