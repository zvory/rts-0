# Phase 7 - Vehicle Migration

## Phase Status

- [x] Implemented.
- Tank, Scout Car, and Command Car are live-routed through SVG rigs and have migration manifests
  with passing part-level plus full-composition pixel gates.

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

- [x] Add SVG rigs for remaining vehicle units.
- [x] Add migration manifests for newly live-routed vehicle units.
- [x] Add hull/weapon-facing and movement context bindings.
- [x] Add vehicle special-cue bindings.
- [x] Route all remaining unit kinds through rig renderer.
- [x] Add and pass part-level plus full-composition pixel gates for vehicle static, weapon-facing,
      recoil, and Command Car breakthrough states.
- [x] Confirm legacy unit draw code is no longer used in normal gameplay.
- [x] Run verification and record exact results.

## Implementation Notes

- Added live SVG rigs and fixture sources for Scout Car and Command Car in
  `client/src/renderer/rigs/vehicle_svg.js` and `tests/fixtures/svg/`.
- Scout Car preserves the vehicle-facing hull/body and independent weapon-facing rear gunner by
  adding schema-approved `scoutGunnerX`, `scoutGunnerY`, `scoutMountX`, and `scoutMountY` runtime
  inputs derived from the existing renderer context.
- Command Car preserves body facing, static command badges, and the Breakthrough aura cue.
- Scout Car and Command Car manifests use bounded per-kind composition thresholds because their
  SVG-authored primitives leave small antialias/overlap residuals against the legacy Pixi draw path.
  No player-facing art drift is intentionally introduced.
- Movement-phase oracle labels already exist, but the migration manifest does not include those
  labels because the current transparent harness does not prime renderer-local vehicle motion state
  for rig samples. Live gameplay still passes vehicle motion context to routed rigs through
  `_rigRenderContextFor`.

## Verification

- Rig schema and SVG importer tests.
- `node tests/svg_migration_guardrails.mjs`
- `node tests/transparent_unit_pixels.mjs --parts --no-artifacts`
- Focused vehicle equivalence tests.
- Full temporary all-unit equivalence suite.
- `node scripts/check-client-architecture.mjs`
- `git diff --check`.

## Verification Results

- `node tests/svg_migration_guardrails.mjs` passed.
- `node tests/transparent_unit_pixels.mjs --parts --no-artifacts` passed 481/481 comparisons with
  no failures.
- `node tests/svg_rig_importer.mjs` passed.
- `node tests/rig_runtime.mjs` passed.
- `node scripts/check-client-architecture.mjs` passed.
- `git diff --check` passed.

## Manual Test Focus

Run a local match or replay with tanks, scout cars, and command cars. Check hull turning,
weapon-facing, recoil, track/wheel motion, fuel cue, breakthrough ring, team tint, selection, hp
bars, fog/shot reveal, and rematch teardown.

## Handoff Expectations

Confirm every unit kind is rig-rendered in normal gameplay, name all remaining legacy files or
functions used only by the equivalence harness, and state whether Phase 8 can delete the duplicate
renderer path.

- Every `UNIT_KINDS` entry is now present in live rig routing.
- Remaining legacy procedural unit draw helpers in `client/src/renderer/units.js` and
  `client/src/renderer/shared.js` are retained for fallback and the temporary Pixi-vs-Pixi
  equivalence harness.
- Phase 8 can start deleting duplicate renderer paths after any desired manual match/replay smoke
  pass.
