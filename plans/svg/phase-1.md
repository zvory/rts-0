# Phase 1 - Temporary Legacy Visual Oracle

## Phase Status

Status: Done.

- [x] Done.

## Objective

Create an automatically verifiable legacy-rendering oracle before the rig renderer exists.

## Work

- Add a test-only renderer fixture that can draw a single legacy unit kind in isolation with
  deterministic Pixi setup, fixed camera scale, fixed team color, and fake entity state.
- Capture semantic measurements from the legacy renderer:
  visible bounds, shadow bounds, selection ring radius, health bar anchor, muzzle anchor, facing
  tick/barrel direction, setup/deploy part positions, and movement-phase part offsets.
- Add bounded pixel-diff support for cases where semantic measurements are insufficient.
- Sample animation states for every current unit kind:
  multiple `facing` values, `weaponFacing` offsets, recoil progress, setup progress, movement
  phase, worker busy state, low/oil-starved cue, breakthrough ring, and shot-reveal alpha.
- Store baselines in a temporary migration-test fixture area with clear naming and comments that
  the data must be deleted in Phase 8.
- Add a script or test entrypoint that fails when the deterministic legacy oracle cannot render.

## Expected Touch Points

- `tests/` or `scripts/` for a focused renderer-equivalence test.
- `client/src/renderer/` only through test seams if needed.
- `plans/svg/phase-1.md`
- Temporary fixture directory chosen in Phase 0.

## Implementation Checklist

- [x] Add deterministic single-unit render fixture.
- [x] Add semantic measurement extraction.
- [x] Add bounded pixel-diff helper with documented thresholds.
- [x] Generate baselines for all unit kinds and required animation samples.
- [x] Mark all new oracle artifacts as temporary migration scaffolding.
- [x] Run focused verification and record exact results.

## Verification

- Focused renderer oracle test or script added by this phase.
- `node scripts/check-client-architecture.mjs` if client modules are touched.
- `git diff --check`.

Results:

- `node tests/legacy_unit_visual_oracle.mjs` - passed, 170 samples.
- `node scripts/check-client-architecture.mjs` - passed.
- `git diff --check` - passed.

## Manual Test Focus

Open a local match or replay only if test seams required renderer changes. Confirm normal units
still display with the legacy renderer.

## Handoff Expectations

List the oracle command, fixture location, sampled states, thresholds, and any known unstable
measurements that later rig comparisons should avoid or replace.
