# Phase 8 - Enforcement and Harness Removal

## Phase Status

- [ ] Not implemented.

## Objective

Remove the temporary migration system and enforce the SVG-authored rig pipeline as the only unit
visual path.

## Work

- Delete legacy procedural unit draw branches and shared helpers that are no longer used.
- Delete temporary legacy baselines, pixel-diff fixtures, side-by-side comparison entrypoints,
  comparison flags/pools, update-only oracle paths, and migration-only test data created by Phases
  1 and 4-7.
- Keep permanent coverage for:
  rig schema validation, SVG importer failure modes, required anchors, animation sampler behavior,
  renderer teardown, architecture boundaries, and a small smoke test that rigged units render.
- Add or tighten architecture checks so new unit visuals cannot bypass the rig pipeline with
  ad hoc `PIXI.Graphics` draw branches in `units.js`.
- Add an enforcement check or focused test preventing future unit-specific procedural draw branches
  in `units.js` and direct unit-art `PIXI.Graphics` work outside `client/src/renderer/rigs/`.
- Update `docs/design/client-ui.md` and any unit-lab docs to describe the SVG authoring workflow,
  required metadata, preview steps, and permanent verification commands.
- Remove feature gates or fallback routing that are no longer needed.

## Expected Touch Points

- `client/src/renderer/units.js`
- `client/src/renderer/shared.js`
- `client/src/renderer/rigs/`
- Temporary migration fixtures/tests from earlier phases.
- Permanent rig/schema/importer tests.
- `scripts/check-client-architecture.mjs`
- `docs/design/client-ui.md`
- `plans/svg/phase-8.md`

## Implementation Checklist

- [ ] Delete legacy procedural unit renderer code.
- [ ] Delete temporary comparison flags, pools, baseline fixtures, update-only oracle paths, and
      equivalence fixtures.
- [ ] Keep permanent schema/importer/animation/render smoke coverage.
- [ ] Enforce rig-only unit visual boundary in architecture checks or focused tests.
- [ ] Update docs for future SVG-authored unit work.
- [ ] Remove stale feature gates and fallback routing.
- [ ] Run verification and record exact results.

## Verification

- Permanent rig schema tests.
- Permanent SVG importer tests.
- Permanent animation sampler tests.
- Permanent renderer smoke for rigged units.
- `node scripts/check-client-architecture.mjs`
- `git diff --check`.

## Manual Test Focus

Run a local match or replay with every unit family visible: worker, infantry, support weapons,
vehicles, and Ekat if present. Check selection, health bars, fog/shot reveal, recoil, setup/deploy,
vehicle motion, team tint, rematch teardown, and unit-lab preview workflow if documented.

## Handoff Expectations

Confirm the temporary equivalence system has been removed, list the permanent tests that replace
it, summarize the new SVG authoring workflow, and name any follow-up art-quality work that is now
possible outside this migration.
