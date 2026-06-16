# Phase 5 - First Live Rigged Unit

## Phase Status

- [ ] Not implemented.

## Objective

Migrate one low-risk unit kind through the full SVG-authored rig pipeline and enable it in live
rendering behind a per-kind gate.

## Work

- Choose the first unit kind from the Phase 0 migration order, preferably Worker/Engineer unless
  the inventory names a safer target.
- Author the SVG source and metadata for that unit's rig.
- Compile and validate the rig through the Phase 3 importer.
- Before enabling the first live unit, introduce a production rig routing seam distinct from the
  temporary comparison seam. The seam owns live rig definitions, live rig instance pools,
  shot-reveal rig instances, and fallback routing. It must not depend on `_rigComparisonEnabled`,
  `_rigComparisonPool`, or a `rigComparisons` layer.
- Enable the rig renderer for only that unit kind behind a small per-kind routing table or feature
  gate.
- Compare the rig against the Phase 1 legacy oracle for:
  static poses, facing samples, shadow, selection ring, health bar placement, busy indicator,
  owner tint, shot reveal alpha, and movement interpolation samples.
- Keep the legacy path available for this unit only as the temporary comparison and rollback path.

## Expected Touch Points

- SVG source fixtures/assets for the selected unit.
- Rig routing table or renderer seam.
- Phase 1 temporary equivalence fixtures/baselines.
- `client/src/renderer/units.js`
- `client/src/renderer/index.js`
- Focused renderer tests.
- `plans/svg/phase-5.md`

## Implementation Checklist

- [ ] Add selected unit SVG rig source.
- [ ] Add or update metadata for required anchors and bounds.
- [ ] Add production renderer routing and live rig pooling independent from the temporary
      equivalence harness.
- [ ] Enable per-kind rig routing for the selected unit.
- [ ] Add equivalence coverage for selected unit static and animation samples.
- [ ] Confirm legacy fallback/comparison path remains test-only or explicitly gated.
- [ ] Run verification and record exact results.

## Verification

- Rig schema and SVG importer tests.
- Focused selected-unit equivalence test.
- Focused renderer smoke if available for unit rendering.
- `node scripts/check-client-architecture.mjs`
- `git diff --check`.

## Manual Test Focus

Run a local match with the selected unit visible. Check team color, facing, selection ring, health
bar position, worker busy/build/mining indicator if Worker is selected, shot reveal if applicable,
and rematch teardown.

## Handoff Expectations

Name the migrated unit kind, equivalence results and thresholds, any visible approved drift, and
whether the per-kind gate is ready for broader unit migration.
Explicitly state whether the production seam is independent from the comparison seam and whether
shot-reveal instances follow the same live routing.
