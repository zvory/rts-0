# Phase 6 - Infantry and Support Weapon Migration

## Phase Status

- [ ] Not implemented.

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

## Expected Touch Points

- SVG rig source files for infantry and support units.
- Rig metadata for muzzle anchors, setup pivots, and semantic bounds.
- Rig animation bindings.
- `client/src/renderer/units.js`
- `client/src/renderer/feedback.js` only if muzzle/anchor lookup needs a narrow API.
- Temporary equivalence tests and fixtures.
- `plans/svg/phase-6.md`

## Implementation Checklist

- [ ] Add SVG rigs for infantry/support units.
- [ ] Add migration manifests for newly live-routed infantry/support units.
- [ ] Add weapon-facing and setup/deploy bindings.
- [ ] Route migrated kinds through the rig renderer.
- [ ] Move feedback anchor lookup to the rig API if needed.
- [ ] Add and pass part-level plus full-composition pixel gates for each migrated kind.
- [ ] Keep legacy code only for comparison and unmigrated vehicles.
- [ ] Run verification and record exact results.

## Verification

- Rig schema and SVG importer tests.
- Focused infantry/support equivalence tests.
- Muzzle/feedback anchor tests if touched.
- `node scripts/check-client-architecture.mjs`
- `git diff --check`.

## Manual Test Focus

Run a local match or replay with infantry and support weapons. Check facing, weapon-facing,
setup/deploy transitions, recoil, muzzle flashes, selection, health bars, and shot-revealed units.

## Handoff Expectations

List each migrated kind, remaining legacy-only unit kinds, equivalence results, any feedback API
changes, and manual visual issues that should be watched during vehicle migration.
