# Phase 7 - Defaulting, Rollout, And Cleanup

## Phase Status

- [ ] Ready for implementation after Phase 6 is merged and packet/reconstruction metrics are
      available across representative workloads.

## Objective

Decide whether stateful delta snapshots should become the normal live path. This phase should use
the measurements from Phases 1 through 6 to choose a default, keep a clear rollback path, update
diagnostic tooling, and remove only experiment scaffolding that is no longer useful.

## Decision Inputs

Collect and compare the same workloads used earlier in the plan:

- Matt/Alex replay or the current equivalent replay workload;
- AI/server perf workload;
- at least one current live/dev stress path;
- a normal local match with fog enabled;
- spectator/replay/lab smoke checks when delta mode is enabled there.

Record, at minimum:

- p50/p95/p99/max payload bytes;
- over-segment-budget rate;
- keyframe rate and forced-keyframe reasons;
- delta fallback rate by section;
- client parse/decode/reconstruct/apply p95;
- server serialize/diff p95;
- stale/unsupported/malformed/resync counts;
- code paths or modes where full-keyframe recovery was used.
- the Phase 2.6 MessagePack keep/revert state.

## Rollout Policy

Default to the conservative choice:

- Keep MessagePack full snapshots as the keyframe/full-snapshot recovery path unless Phase 2.6
  reverted MessagePack.
- Enable delta snapshots by default only if representative workloads show a meaningful p95 byte and
  over-budget-rate improvement without a material parse/decode/reconstruct/apply or server serialize
  regression.
- If savings are narrow, keep delta mode opt-in and document the next focused bottleneck instead of
  forcing default rollout.
- If recovery, replay/lab, or fog/privacy issues remain, keep delta mode disabled by default and open
  a follow-up plan before more implementation.
- Do not silently change the Phase 2.6 MessagePack keep/revert decision while defaulting deltas
  unless the measurements explicitly cover that combined mode and rollback path.

## Work

- Add or finalize runtime selection:
  - environment flag and/or negotiated capability for delta defaulting;
  - clear recovery to MessagePack full keyframes;
  - startup/version diagnostics showing which snapshot mode is active.
- Update diagnostics and tooling:
  - `ClientNetReport` and server structured logs should report active snapshot mode, keyframe/delta
    counts, keyframe reasons, resync counts, and reconstruction cost where available;
  - `scripts/parse-net-report-logs.mjs` should summarize mode, p95 bytes, over-budget rate,
    keyframe ratio, recovery reasons, and recovery counts;
  - `scripts/client-perf-harness.mjs` summaries should include the same fields for comparisons.
- Update docs:
  - `docs/design/protocol.md` should describe the final default and keyframe recovery contract;
  - `docs/perf-tracing.md` should explain how to interpret delta/keyframe metrics;
  - plan phase docs should be marked done only by the implementing phase commits, not retroactively
    in this planning pass.
- Clean up:
  - remove dead experiment-only candidates that Phase 2 or later phases definitively rejected;
  - keep MessagePack full-keyframe recovery and tests;
  - keep targeted malformed/recovery tests even if delta becomes default.

## Expected Touch Points

- `server/src/main.rs`
- Phase 3-6 snapshot codec modules
- `server/src/structured_log.rs`
- `server/crates/protocol/src/lib.rs`
- `client/src/net.js`
- `client/src/client_perf_report.js`
- `client/src/protocol.js`
- `scripts/parse-net-report-logs.mjs`
- `scripts/client-perf-harness.mjs`
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- `tests/client_contracts.mjs`
- `tests/protocol_parity.mjs`
- focused parser/harness/logging tests

## Implementation Checklist

- [ ] Gather comparison metrics from representative workloads.
- [ ] Decide default, opt-in, or defer using the rollout policy above.
- [ ] Implement runtime selection and clear recovery diagnostics.
- [ ] Update report/log/parser/harness output for final mode metrics.
- [ ] Update protocol/perf docs.
- [ ] Remove stale experiment-only code without removing MessagePack full-keyframe recovery.
- [ ] Add or keep focused malformed-frame and recovery tests.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- `node tests/net_report_log_parser.mjs` if parser output changes
- focused structured-log tests if report fields or classifications change
- focused client perf harness run for at least one representative workload when practical
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If broad workload runs are not practical locally, keep delta mode disabled by default and document the
missing evidence in the handoff.

## Manual Test Focus

Run a normal local match in the selected default mode and in the MessagePack full-keyframe mode. Confirm
commands, fog, selection, replay entry/seek, spectator view, lab/dev-watch paths, and diagnostics all
remain understandable. If delta mode is enabled by default, verify that flipping the rollback flag
returns the connection to MessagePack full snapshots without gameplay changes.

## Handoff Expectations

State the rollout decision plainly: default-on, opt-in only, or deferred. Include the comparison
table that drove the decision, the rollback flag/capability, the retained recovery tests, and any
follow-up plan needed for remaining packet-budget pressure.
