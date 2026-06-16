# Phase 1 - Checker Recovery And Source Of Truth

## Phase Status

- [ ] Not implemented.

## Objective

Restore the faction assumption checker and define which active docs own faction boundary facts.

## Work

- Fix the `plans/faction` versus `plans/archive/faction` failure mode in
  `scripts/check-faction-assumptions.mjs`.
- Decide whether archived faction plan files are historical evidence only or intentionally active
  checker inputs.
- Move active lifecycle policy out of archived plan files. Active faction lifecycle policy should
  live in `docs/design/faction-architecture-inventory.md` or a new active design doc such as
  `docs/design/faction-lifecycle.md`; scripts must not read `plans/archive/faction/*` except for
  historical archive-policy checks.
- Improve checker errors for missing expected docs or anchors.
- Record the current status of `kriegsia`, `ekat`, and `phase2_empty_fixture`.
- Resolve active-doc contradictions for `kriegsia`, `ekat`, and `phase2_empty_fixture` before
  adding stronger anchors in Phase 2.
- Remove stale hard-coded compact-version and moved-plan-path assumptions from the checker before
  converting them into ratchets.

## Expected Touch Points

- `scripts/check-faction-assumptions.mjs`
- `docs/design/faction-architecture-inventory.md`
- Possibly a new active lifecycle section or doc if archive files should not be read
- `plans/factionguardrails/*`

## Implementation Checklist

- [ ] Remove or document the checker's dependency on moved plan paths.
- [ ] Move active lifecycle policy out of archived plan files and make archived files
      historical-only unless explicitly named.
- [ ] Define active source-of-truth docs for faction boundaries.
- [ ] Resolve all active-doc contradictions for `kriegsia`, `ekat`, and `phase2_empty_fixture`.
- [ ] Add clearer checker diagnostics.
- [ ] Remove stale hard-coded compact version and moved-plan path assumptions from the checker.
- [ ] Record current catalog id statuses.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `node scripts/check-faction-assumptions.mjs`
- `node scripts/check-faction-catalog-parity.mjs`
- `git diff --check`

## Manual Test Focus

No gameplay test expected. Human review should confirm the boundary wording matches product intent.

## Handoff Expectations

State whether archived faction files are checker inputs and state the decided status of `kriegsia`,
`ekat`, and `phase2_empty_fixture`.
