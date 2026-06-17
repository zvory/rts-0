# Phase 6 - Infantry and Support Weapon Migration

## Phase Status

- [x] Implemented.

## Objective

Migrate strict top-down infantry and crew-served weapon visuals to SVG-authored rigs after the
Phase 5.x mechanical visual gates are in place.

## Work

- Author and validate SVG rigs for Rifleman, Machine Gunner, Anti-Tank Gun, Mortar Team,
  Artillery, and Ekat if Ekat still uses the generic fallback art at implementation time.
- Add or update migration manifests for every newly live-routed unit kind.
- Migrate in family gates: Rifleman and Machine Gunner first to prove shared infantry bindings,
  then Anti-Tank Gun, Mortar Team, and Artillery. Keep all previously migrated unit equivalence
  tests passing before adding the next family.
- Keep Ekat explicitly deferred unless its active-ability visual contract is stable at the start of
  this phase.
- Encode common infantry body parts as shared authoring patterns or reusable rig fragments only if
  Phase 2/3 APIs already support that cleanly. Do not add an abstraction just to remove minor SVG
  duplication.
- Implement weapon-facing separation for rifles, machine guns, AT guns, mortar tubes, artillery
  barrels, and any Ekat weapon/ability-facing visuals.
- Implement setup/deploy animation bindings for Machine Gunner, Anti-Tank Gun, Mortar Team, and
  Artillery.
- Implement recoil and muzzle anchors so existing muzzle flash and tracer feedback attach to rig
  parts rather than duplicated geometry.
- Extend equivalence coverage for all migrated unit kinds across static, recoil, setup, deploy,
  weapon-facing, and shot-reveal samples.
- Require named part-level plus full-composition pixel gates for each migrated kind before enabling
  live routing; keeping the old legacy oracle passing is not enough by itself.
- Before adding a kind to live routing, add its entry to
  `tests/fixtures/svg/unit_migration_manifests.mjs`, run
  `node tests/svg_migration_guardrails.mjs`, then run
  `node tests/transparent_unit_pixels.mjs --parts --no-artifacts`.

## Expected Touch Points

- SVG rig source files for infantry and support units.
- Rig metadata for muzzle anchors, setup pivots, and semantic bounds.
- Rig animation bindings.
- `client/src/renderer/units.js`
- `client/src/renderer/feedback.js` only if muzzle/anchor lookup needs a narrow API.
- Temporary equivalence tests and fixtures.
- `plans/svg/phase-6.md`

## Implementation Checklist

- [x] Add SVG rigs for infantry/support units.
- [x] Add migration manifests for newly live-routed infantry/support units.
- [x] Add weapon-facing and setup/deploy bindings.
- [x] Route migrated kinds through the rig renderer.
- [x] Move feedback anchor lookup to the rig API if needed; existing anchor lookup stayed sufficient.
- [x] Add and pass part-level plus full-composition pixel gates for each migrated kind.
- [x] Keep legacy code only for comparison and unmigrated vehicles.
- [x] Run verification and record exact results.

## Verification

- `node tests/visual_pixel_compare_test.mjs` passed.
- `node tests/rig_schema.mjs` passed.
- `node tests/svg_rig_importer.mjs` passed.
- `node tests/rig_runtime.mjs` passed.
- `node tests/svg_migration_guardrails.mjs` passed.
- `node tests/transparent_unit_pixels.mjs --parts --no-artifacts` passed 481/481 with no failures
  after the remaining Scout Car, Command Car, and Ekat manifests were added.
- `node scripts/check-client-architecture.mjs` passed.
- `git diff --check` passed.

## Manual Test Focus

Run a local match or replay with infantry and support weapons. Check facing, weapon-facing,
setup/deploy transitions, recoil, muzzle flashes, selection, health bars, and shot-revealed units.

Manual review was completed from a local server at `http://127.0.0.1:38106/`; lobby and unit
visuals looked correct.

## Handoff Expectations

List each migrated kind, remaining legacy-only unit kinds, equivalence results, any feedback API
changes, and manual visual issues that should be watched during vehicle migration.

- Migrated live-routed Phase 6 unit kinds: Rifleman, Machine Gunner, Anti-Tank Gun, Mortar Team,
  and Artillery.
- Previously live-routed SVG unit kinds still covered by guardrails: Worker and Tank.
- Ekat has now been migrated from the generic fallback/procedural unit art to a live SVG rig that
  preserves the fallback pentagon body plus facing tick.
- No unit kind remains intentionally legacy-routed in normal gameplay; legacy draw code remains for
  the temporary equivalence harness until Phase 8 removes it.
- Part-level plus full-composition transparent pixel gates passed for all migration manifests.
- No feedback API change was required; muzzle anchors remain available through the existing rig
  routing path.
