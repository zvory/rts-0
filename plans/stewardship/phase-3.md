# Phase 3 - Refresh Source-Size Guardrails

Status: Incomplete.

## Objective

Make the source-size inventory accurately describe the current tree without turning ordinary file
shrinkage into recurring CI bookkeeping. Add CSS coverage and refresh obsolete exceptions only; do
not split or refactor runtime files in this phase.

## Work

- Include production CSS in the source-size inventory and freeze the existing large stylesheet at
  its current intentional size with a reviewable reason.
- Remove exceptions whose files are gone or now at or below the ordinary 1,500-line cap.
- Lower the checked-in above-cap exception values to the current intentional sizes as a one-time
  baseline refresh.
- Keep above-cap shrinkage advisory after that refresh. CI should continue to fail on growth beyond
  the baseline, a missing justification, or an obsolete exception whose file is now at or below the
  cap, but not on every small above-cap reduction.
- Keep generated, vendored, dependency, and build-output exclusions explicit.
- Update focused checker tests so growth, obsolete exceptions, CSS inclusion, and advisory shrinkage
  are each proven.

## Non-goals

- Do not split `client/styles.css` or any oversized Rust, JavaScript, or test file.
- Do not restyle the client or change pixels.
- Do not add public API, prototype-graft, fan-out, or command-policy checks; later phases own those
  boundaries.

## Expected Touch Points

- `scripts/check-source-file-sizes.mjs`
- `scripts/source-file-size-baseline.json`
- focused source-size checker tests
- testing or architecture documentation only where the enforcement policy changes

## Verification

- `node scripts/check-source-file-sizes.mjs`
- Focused negative fixtures proving a new oversized CSS file and growth above an exception fail.
- Focused fixtures proving above-cap shrinkage remains non-failing while an exception at or below the
  ordinary cap fails until removed.
- `git diff --check`

## Manual Test Focus

No gameplay test is expected. Inspect checker output once and confirm failures explain the path,
current size, allowed size, and intended remedy without treating beneficial shrinkage as an error.

## Handoff

Mark this phase done in its implementation commit. Report the CSS inventory rule, removed or lowered
exceptions, final failure policy, and focused negative evidence. Tell the Phase 4 agent which size
metrics are now owned here so it does not duplicate them in architecture seam checks.
