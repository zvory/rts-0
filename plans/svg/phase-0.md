# Phase 0 - Contract and Current Anatomy Inventory

## Phase Status

- [ ] Not implemented.

## Objective

Freeze the migration contract before adding schema, SVG parsing, or runtime rig code.

## Work

- Inventory every current unit visual in `client/src/renderer/units.js` and
  `client/src/renderer/shared.js`.
- For each unit kind, record the procedural subparts that must become named rig parts:
  body/hull, head/crew, rifle or machine gun, mortar tube, AT/artillery carriage, tank tracks,
  turret/barrel, wheels/running gear, fuel cue, busy indicator, muzzle anchor, shadow, and any
  special ring or affordance.
- Inventory every animation input currently used by the renderer:
  `facing`, `weaponFacing`, setup state, recoil progress, movement/tread phase, owner color, oil
  state, breakthrough ticks, busy worker state, and shot-reveal alpha.
- Define the exact temporary equivalence spec:
  semantic measurements, animation sample matrix, pixel-diff thresholds, viewport/camera settings,
  acceptable alpha drift, and when a migrated rig is allowed to differ from legacy output.
- Define the approved SVG authoring conventions:
  required root metadata, part ids, allowed SVG elements, transform rules, tint markers, anchor
  markers, bounds markers, unsupported feature handling, and coordinate origin.
- Update this phase file with the inventory results and any plan adjustments discovered.

## Expected Touch Points

- `plans/svg/phase-0.md`
- `client/src/renderer/units.js`
- `client/src/renderer/shared.js`
- `client/src/renderer/entities.js`
- `client/src/renderer/index.js`
- `client/unit-lab.js`
- `docs/design/client-ui.md`
- `scripts/check-client-architecture.mjs`

## Implementation Checklist

- [ ] Inventory unit kinds and procedural visual subparts.
- [ ] Inventory animation inputs and renderer-local visual state.
- [ ] Define SVG authoring conventions and unsupported SVG behavior.
- [ ] Define normalized rig API and ownership boundaries.
- [ ] Define equivalence sampling matrix and thresholds.
- [ ] Record any phase split or order changes needed before implementation.

## Verification

- Docs-only phase unless the inventory discovers missing plan files.
- Run `git diff --check`.

## Manual Test Focus

None required. This phase should not change runtime behavior.

## Handoff Expectations

Name the final rig API, SVG conventions, equivalence thresholds, unit-kind migration order, and any
risks that later phases must resolve before writing renderer code.
