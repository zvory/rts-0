# Phase 3 - SVG Importer and Authoring Fixtures

## Phase Status

- [ ] Not implemented.

## Objective

Turn SVG-authored unit files into validated normalized rig definitions.

## Work

- Implement `compileSvgRig(svgText, metadata)` as a pure importer that parses SVG text and emits
  normalized rig definitions.
- Support only Phase-0-approved SVG features. Reject unsupported filters, external images,
  scripts, foreign objects, CSS dependencies, non-finite transforms, and ambiguous inherited
  styles.
- Map SVG groups and attributes into rig concepts:
  part ids, local transforms, pivots, anchors, team tint slots, static fills/strokes, semantic
  bounds, draw order, and optional animation binding names.
- Add a small set of checked-in authored SVG fixtures for representative unit anatomy:
  worker-like body, infantry weapon, crew-served weapon, and vehicle hull/turret.
- Add tests proving fixture SVGs compile to normalized rig definitions and invalid SVG fails
  closed with useful errors.
- Optionally connect `/dev/unit-lab` to preview compiled fixtures if that can be done without
  expanding the phase.

## Expected Touch Points

- Rig importer modules under the Phase-0-approved location.
- Checked-in SVG fixture directory chosen in Phase 0.
- Focused SVG importer tests.
- `client/unit-lab.js` only if the preview hook is kept small.
- `docs/design/client-ui.md`
- `plans/svg/phase-3.md`

## Implementation Checklist

- [ ] Add pure SVG-to-rig importer.
- [ ] Add supported/unsupported SVG feature tests.
- [ ] Add representative SVG fixtures.
- [ ] Add metadata and anchor extraction tests.
- [ ] Keep importer output independent from Pixi.
- [ ] Run verification and record exact results.

## Verification

- Focused SVG importer tests.
- Rig schema tests from Phase 2.
- `node scripts/check-client-architecture.mjs`
- `node --check` for new JS files.
- `git diff --check`.

## Manual Test Focus

If unit-lab preview is touched, open `/dev/unit-lab` and confirm existing generation browsing still
works and SVG fixture previews do not require a server restart beyond the normal dev loop.

## Handoff Expectations

List supported SVG elements/attributes, fixture files, importer command or test entrypoint, and
any SVG features explicitly rejected for future art authors.
