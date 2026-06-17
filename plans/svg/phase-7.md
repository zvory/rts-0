# Phase 7 - Vehicle Migration

## Phase Status

- [ ] Not implemented.
- Partial current state: Tank is already live-routed through an SVG rig and has a Phase 5.4
  migration manifest with passing part-level plus full-composition pixel gates. Scout Car and
  Command Car remain legacy-routed.

## Objective

Migrate the remaining vehicle-body units to SVG-authored rigs and make the rig renderer cover
every unit kind after the Phase 5.x mechanical visual gates are in place.

## Work

- Author and validate SVG rigs for Scout Car and Command Car. Tank should remain live-routed and
  its manifest/gates must keep passing.
- Add or update migration manifests for every newly live-routed vehicle kind.
- Implement hull-facing vs weapon-facing separation for vehicles.
- Implement track or wheel phase animation bindings from the renderer-local movement visual state.
- Implement recoil bindings for Tank and Scout Car weapons.
- Preserve vehicle-specific details:
  Tank fuel cue, Command Car breakthrough ring attachment, Scout Car gunner/weapon readability,
  vehicle shadow shape, selection bounds, and hp bar position.
- Add equivalence samples for multiple facings, weapon-facing offsets, movement phase deltas,
  recoil values, low/oil-starved states, breakthrough ticks, and shot-reveal alpha.
- Require named part-level plus full-composition pixel gates for each migrated vehicle kind before
  enabling live routing.
- Before adding a kind to live routing, add its entry to
  `tests/fixtures/svg/unit_migration_manifests.mjs`, run
  `node tests/svg_migration_guardrails.mjs`, then run
  `node tests/transparent_unit_pixels.mjs --parts --no-artifacts`.
- Confirm there are no remaining live unit kinds routed through legacy procedural drawing.

## Expected Touch Points

- SVG rig source files for vehicle units.
- Rig animation bindings for hull, turret/weapon, tracks/wheels, and special cues.
- `client/src/renderer/units.js`
- `client/src/renderer/shared.js`
- Temporary equivalence tests and fixtures.
- `plans/svg/phase-7.md`

## Implementation Checklist

- [ ] Add SVG rigs for remaining vehicle units.
- [ ] Add migration manifests for newly live-routed vehicle units.
- [ ] Add hull/weapon-facing and movement-phase bindings.
- [ ] Add vehicle special-cue bindings.
- [ ] Route all remaining unit kinds through rig renderer.
- [ ] Add and pass part-level plus full-composition pixel gates for vehicle static and animation states.
- [ ] Confirm legacy unit draw code is no longer used in normal gameplay.
- [ ] Run verification and record exact results.

## Verification

- Rig schema and SVG importer tests.
- `node tests/svg_migration_guardrails.mjs`
- `node tests/transparent_unit_pixels.mjs --parts --no-artifacts`
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
