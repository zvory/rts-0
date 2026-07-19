# Phase 3 - Local Reproduction Harness

## Phase Status

- [x] Archived as stale and not completed on 2026-07-18.

This harness phase depended on evidence that Phase 2 never collected. Do not implement it from this
document; define any future reproduction signature and tooling from a fresh incident investigation.

## Objective

Create a local automated reproduction path that can drive the command-density jitter signature found
in phase 2 without requiring the user to manually open the game and issue commands. The harness
should be usable by later fixing agents as a repeatable before/after validation loop.

## Work

- Use phase 2 evidence to define the target signature before implementing the harness. At minimum,
  define expected command density, snapshot jitter/gap/burst behavior, frame-gap behavior,
  prediction-health behavior, and server-health non-signals.
- Choose the smallest local reproduction mechanism that can observe browser behavior:
  - a live local server plus Playwright/Chrome driving the real client
  - a Node integration flow only if browser frame/prediction behavior is not required
  - a purpose-built synthetic WebSocket/client harness only if it can faithfully reproduce the phase
    2 signature
- Drive controlled scenarios:
  - idle baseline
  - steady normal command cadence
  - high-density repeated move-command burst
  - optional network or reliable-message delay injection only if phase 2 evidence justifies it
- Capture the same parser-compatible logs and client diagnostics used in phase 2.
- Add a small analyzer or assertion layer that classifies whether the local run matches the preserved
  beta signature. Prefer "matches / does not match / inconclusive" over brittle absolute thresholds.
- Save generated artifacts under ignored or `/tmp` paths by default. Commit only harness code, docs,
  and stable fixtures, not large generated logs unless they are small intentional examples.
- Do not implement the fix in this phase. The harness should make fixing possible later.

## Expected Touch Points

- `tests/` or `scripts/` for the local harness entry point
- `scripts/parse-net-report-logs.mjs` only if a small reusable analyzer extension is needed
- `docs/network-incident-examples/<phase-2-dir>/` for references to the target signature
- `docs/context/testing.md` or an adjacent runbook if the harness becomes an operator workflow
- no gameplay behavior changes except optional test-only instrumentation gates

## Implementation Checklist

- [ ] Extract the phase 2 target signature into a concise harness requirement section.
- [ ] Implement a local automated command-density driver.
- [ ] Capture parser-compatible logs and client prediction/frame diagnostics.
- [ ] Add a comparator/classifier for idle, normal-command, and high-command runs.
- [ ] Document how later agents run the harness before and after candidate fixes.
- [ ] Add focused tests for any reusable analyzer code.
- [ ] Mark this phase as done in this file in the implementation commit.

## Verification

- Run the new harness command on a local server.
- Run `node scripts/parse-net-report-logs.mjs` against the generated local logs if the harness emits
  Fly-compatible JSONL or tracing text.
- Run focused tests for any added analyzer modules.
- `node scripts/check-client-architecture.mjs` if browser/client modules are touched.
- `git diff --check`

## Manual Test Focus

Manual testing should be limited to sanity-checking that the harness actually opens or drives the
game, issues the expected command densities, and saves artifacts where documented. The user should
not need to manually reproduce the stutter for this phase to pass.

## Handoff Expectations

Report the exact harness command, where artifacts are written, and whether the local harness matches
the phase 2 beta signature. If it cannot reproduce the issue locally, state which phase 2 signals are
missing and what extra local simulation, browser, or transport control a follow-up plan would need.
Do not propose a fix until the local reproduction status is clear.
