# Phase 8 - Enforcement and Harness Removal

## Phase Status

- [x] Implemented.

## Objective

Remove the temporary migration system and enforce the SVG-authored rig pipeline as the only unit
visual path.

## Work

- Delete legacy procedural unit draw branches and shared helpers that are no longer used.
- Delete temporary legacy baselines, pixel-diff fixtures, side-by-side comparison entrypoints,
  comparison flags/pools, update-only oracle paths, and migration-only test data created by Phases
  1 and 4-7.
- Explicitly audit and either delete or replace all Phase 5.x migration artifacts:
  `tests/fixtures/svg/unit_migration_manifests.mjs`, `tests/svg_migration_guardrails.mjs`,
  `tests/transparent_unit_pixels.mjs`, `scripts/dump-legacy-unit-parts.mjs`,
  `tests/fixtures/svg/legacy-unit-oracle.baseline.json`, and any remaining generated/fixture SVGs
  that exist only to compare legacy procedural output against live rigs.
- Treat the Phase 5.4 manifest/check workflow as temporary migration scaffolding. If any of its
  invariants remain useful after every unit is rig-rendered, preserve them by moving the invariant
  into permanent rig schema/importer/runtime tests or `scripts/check-client-architecture.mjs`, not by
  keeping the migration manifest as a second source of truth.
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
- Temporary migration fixtures/tests from earlier phases, including the Phase 5.x manifest, pixel
  harness, static guardrail, and legacy metadata dump tool.
- Permanent rig/schema/importer tests.
- `scripts/check-client-architecture.mjs`
- `docs/design/client-ui.md`
- `plans/svg/phase-8.md`

## Implementation Checklist

- [x] Delete legacy procedural unit renderer code.
- [x] Delete temporary comparison flags, pools, baseline fixtures, update-only oracle paths, and
      equivalence fixtures.
- [x] Remove or replace Phase 5.x artifacts:
      `unit_migration_manifests.mjs`, `svg_migration_guardrails.mjs`,
      `transparent_unit_pixels.mjs`, `dump-legacy-unit-parts.mjs`, and
      `legacy-unit-oracle.baseline.json`.
- [x] Keep permanent schema/importer/animation/render smoke coverage.
- [x] Enforce rig-only unit visual boundary in architecture checks or focused tests.
- [x] Update docs for future SVG-authored unit work.
- [x] Remove stale feature gates and fallback routing.
- [x] Run verification and record exact results.

## Completion Notes

- `client/src/renderer/units.js` now routes unit and shot-reveal visuals only through live SVG rig
  definitions. Missing definitions or routes fail through the renderer's existing missing-texture
  guard instead of drawing a procedural unit fallback.
- Removed the migration comparison seam from `renderer/rigs/runtime.js`, renderer initialization,
  teardown, and sweep logic.
- Deleted the temporary Phase 5.x migration stack: manifest, static guardrail, transparent pixel
  harness/page, legacy oracle baseline, visual pixel diff helper/test, and legacy part dump script.
- Preserved permanent coverage in schema/importer/runtime/client-contract tests, refreshed the stale
  Ekat SVG fixture to match production rig source, and added architecture checks that prevent
  procedural unit art from returning to `renderer/units.js`.
- Updated `docs/design/client-ui.md`, `tests/README.md`, `docs/doc-map.json`, and `tests/run-all.sh`
  for the post-migration SVG rig workflow.

## Verification Results

- `node tests/rig_schema.mjs` passed.
- `node tests/svg_rig_importer.mjs` passed.
- `node tests/rig_runtime.mjs` passed.
- `node scripts/check-client-architecture.mjs` passed.
- `node tests/client_contracts.mjs` passed.
- `git diff --check` passed.

## Verification

- Permanent rig schema tests.
- Permanent SVG importer tests.
- Permanent animation sampler tests.
- Permanent renderer smoke for rigged units.
- Permanent replacement checks for any Phase 5.x invariant intentionally kept after removing the
  migration manifest workflow.
- `node scripts/check-client-architecture.mjs`
- `git diff --check`.

## Manual Test Focus

Run a local match or replay with every unit family visible: worker, infantry, support weapons,
vehicles, and Ekat if present. Check selection, health bars, fog/shot reveal, recoil, setup/deploy,
vehicle motion, team tint, rematch teardown, and unit-lab preview workflow if documented.

## Handoff Expectations

Confirm the temporary equivalence system has been removed, list the permanent tests that replace
it, explicitly state what happened to the Phase 5.x manifest/pixel/dump artifacts, summarize the
new SVG authoring workflow, and name any follow-up art-quality work that is now possible outside
this migration.
