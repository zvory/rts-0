# Phase 5.3 - Worker Re-Migration Through Pixel Gates

## Phase Status

- [ ] Not implemented.

## Objective

Rework the live Worker SVG rig until it passes the new part-level and full-composition pixel gates,
then keep Worker live routing enabled only because the mechanical gates prove it matches legacy.

## Why This Phase Exists

The current Worker rig should not be treated as accepted art. It is useful only as evidence that
the live routing seam can work and that the previous verification was too weak. This phase uses
Worker as the calibration case for the new harness: the expected end state is the old static
outlined pentagon body, rotating facing tick, and busy indicator behavior proven by part and
composition pixel gates.

## Work

- Treat the current Phase 5 Worker mismatch as the acceptance target: legacy Worker body is a
  static pentagon with a dark outline, and only the facing tick changes with `facing`.
- Update the Worker authored SVG/source and animation bindings so:
  body fill and black outline match legacy,
  body geometry does not rotate across facings,
  facing tick rotates across facings,
  busy indicator appears only for mining/building,
  shadow, HP anchor, and selection metadata remain within approved thresholds.
- Run Worker part-level gates first. Do not adjust full-unit thresholds to hide part failures.
- Run Worker full-composition gates after parts pass. Any remaining full-unit failures should be
  explained as stacking, parent transform, alpha blending, or draw-order defects and fixed directly.
- Keep production live routing Worker-only and keep the legacy path available as the temporary
  comparison/rollback path until Phase 8 removes migration scaffolding.
- Record any intentional visible drift explicitly. The expected outcome is no intentional drift for
  Worker unless the phase discovers an old renderer bug and the user approves changing the art.

## Expected Touch Points

- Worker SVG source and test fixture.
- Worker part mapping or thresholds from Phase 5.2.
- Live rig routing if Worker needs temporary gating while tests are fixed.
- Worker visual comparison tests.
- `plans/svg/phase-5.3.md`.

## Implementation Checklist

- [ ] Fix Worker body geometry, outline, and non-rotating body behavior.
- [ ] Fix Worker facing tick and busy indicator bindings.
- [ ] Pass Worker part-level visual comparisons.
- [ ] Pass Worker full-composition visual comparisons.
- [ ] Keep live Worker routing independent from the temporary comparison seam.
- [ ] Run verification and record exact results.

## Verification

- Worker part-level visual comparison command.
- Worker full-composition visual comparison command.
- `node tests/rig_schema.mjs`.
- `node tests/svg_rig_importer.mjs`.
- `node tests/rig_runtime.mjs`.
- `node scripts/check-client-architecture.mjs`.
- `git diff --check`.

## Manual Test Focus

Run a local match with Workers visible only as a sanity check that the routed Worker appears in
gameplay. Acceptance must come from the pixel gates, not from manual judgment. Check that rematch
teardown still does not leak rig instances if the implementation touched routing or pools.

## Handoff Expectations

Report the Worker part-gate and composition-gate results, the exact thresholds, and any failure
artifacts that were generated and then resolved. State whether Worker is ready to remain live and
whether Phase 6 can use the same gates for infantry/support units.
