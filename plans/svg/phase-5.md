# Phase 5 - First Live Rigged Unit

## Phase Status

Status: Done.

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

- [x] Add selected unit SVG rig source.
- [x] Add or update metadata for required anchors and bounds.
- [x] Add production renderer routing and live rig pooling independent from the temporary
      equivalence harness.
- [x] Enable per-kind rig routing for the selected unit.
- [x] Add equivalence coverage for selected unit static and animation samples.
- [x] Confirm legacy fallback/comparison path remains test-only or explicitly gated.
- [x] Run verification and record exact results.

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

## Implementation Notes

- Migrated Worker/Engineer to live SVG rig rendering through
  `client/src/renderer/rigs/live_routing.js`; the production seam compiles the Worker SVG source
  into `_liveRigDefinitionsByKind` and falls back to legacy drawing if the definition is missing or
  rejected.
- Added live rig pools independent from `_rigComparisonEnabled`, `_rigDefinitionsByKind`,
  `_rigComparisonPool`, and the test-only `rigComparisons` layer. Worker shadow and body parts use
  separate live pools so normal unit layer ordering is preserved.
- Shot-reveal Workers use the same live rig route with shot-reveal shadow/body pools and alpha.
- Legacy procedural Worker rendering remains available as fallback and for the test comparison
  harness; the side-by-side comparison seam remains explicit and test-gated.
- Worker part and composition gate:
  `node tests/transparent_unit_pixels.mjs --parts --no-artifacts` passed 42/42 comparisons with
  0 failures and exact pixel matches (`alphaWeightedMatchingRatio=1`,
  `maxPerPixelRgbaDistance=0`, `opaqueMismatchCount=0`).
- Additional verification passed:
  `node tests/rig_runtime.mjs`,
  `node tests/svg_rig_importer.mjs`,
  `node tests/rig_schema.mjs`,
  `node scripts/check-client-architecture.mjs`,
  and `git diff --check`.
- No intentional visible drift. Player-facing gameplay behavior is unchanged; visible Worker art is
  now routed through the SVG rig that matches the previous procedural Worker output.
