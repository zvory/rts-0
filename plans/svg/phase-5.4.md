# Phase 5.4 - Future Unit Migration Guardrails

## Phase Status

- [ ] Not implemented.

## Objective

Make the pixel and part-level gates mandatory for future SVG unit migrations, and add lightweight
tooling that helps executors mechanically draft or inspect rig parts from legacy draw output.

## Why This Phase Exists

Once Worker is fixed, the important lesson should become a guardrail rather than tribal knowledge.
Future conversions should not rely on a person repeatedly checking whether a Rifleman, mortar, tank,
or shot-reveal looks close enough. This phase makes the migration process LLM-friendly by requiring
per-unit manifests, named part mappings, and reusable commands that produce objective failures and
legacy geometry/style clues.

## Work

- Add a reusable migration manifest format for each unit kind that names:
  legacy part names,
  rig part ids,
  required animation samples,
  per-part thresholds,
  full-composition thresholds,
  and any approved intentional drift.
- Add guardrails so a unit kind cannot be added to live rig routing unless it has a migration
  manifest and passing part-level plus full-composition visual gates.
- Add a small debug/export tool that can dump legacy captured part geometry/style metadata for a
  selected unit sample. The tool does not need to perfectly author SVG, but it should make future
  conversion more mechanical by showing polygon points, line styles, fills, strokes, alpha, draw
  order, and transforms per part.
- Update the SVG plan docs so Phase 6 and Phase 7 explicitly use the new manifest and gates before
  enabling any additional unit kind.
- Keep failure artifacts local/ignored and keep deterministic baseline metadata reviewed in git.

## Expected Touch Points

- Visual migration manifest files or fixtures under `tests/fixtures/svg/`.
- Visual comparison harness and routing tests.
- `client/src/renderer/rigs/live_routing.js` or focused guard helper.
- `scripts/check-client-architecture.mjs` or a new focused checker if live routing needs manifest
  enforcement outside the visual test.
- `plans/svg/plan.md`, `phase-6.md`, `phase-7.md`, and `phase-5.4.md`.

## Implementation Checklist

- [ ] Add per-unit migration manifest format and Worker manifest.
- [ ] Require passing visual gates for live-routed rig kinds.
- [ ] Add legacy part metadata dump/debug tool for future conversions.
- [ ] Update Phase 6 and Phase 7 docs to require manifests and visual gates.
- [ ] Preserve no-build-step client constraints.
- [ ] Run verification and record exact results.

## Verification

- Manifest validation or guardrail test.
- Worker part-level and full-composition visual comparison commands.
- Existing rig schema/importer/runtime tests if routing or rig code changes.
- `node scripts/check-client-architecture.mjs`.
- `git diff --check`.

## Manual Test Focus

No gameplay manual test is required unless live routing code changes. If routing changes, run a
local Worker match smoke only to confirm the live gate still routes Worker and rejects unmigrated
unit kinds.

## Handoff Expectations

Report the manifest format, the guardrail command, and the legacy metadata dump command. State the
exact command future Phase 6/7 executors should run before enabling a new rigged unit kind.
