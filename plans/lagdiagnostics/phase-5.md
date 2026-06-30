# Phase 5 - Client Frame and Prediction Context

## Phase Status

- [ ] Not started.

## Objective

Make client-side frame and prediction contributions interpretable from normal report windows. This
phase should bridge the gap between rich local `window.__rtsPerf` summaries and sparse uploaded net
reports without uploading raw frames or traces.

## Work

- Add bounded uploaded summaries for the top stable frame phases and renderer/minimap/HUD diagnostic
  counter groups when frame work or renderer thresholds are crossed.
- Preserve existing local-only raw/debug details; only upload stable labels, counts, buckets, and
  top-N groups.
- Add or improve report fields that explain:
  - RAF dispatch delay versus named JavaScript work
  - unattributed frame work
  - top renderer subphase
  - top render diagnostic counter group
  - prediction replay work during late snapshots
  - whether predicted owned-world coverage was present while snapshots were late
  - focus/hidden state and viewport/DPR context
- Extend parser output so a render/frame-work classification names the likely local phase and
  states when local frame work was not the sustained bottleneck.
- Keep client architecture boundaries intact by routing diagnostics through existing `Match`
  composition and report aggregation.

## Expected Touch Points

- `client/src/frame_profiler.js`
- `client/src/client_perf_report.js`
- `client/src/match_net_reporter.js`
- `client/src/match_health.js`
- `client/src/prediction_controller.js`
- `client/src/net.js`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `server/src/structured_log.rs`
- `client/src/protocol.js`
- `scripts/parse-net-report-logs.mjs`
- `docs/perf-tracing.md`
- focused client/report tests

## Agent-Readable Output Requirements

- Upload only allowlisted diagnostic labels and bounded counts.
- Parser output should distinguish `frame.rafDispatch`, named `match.*` work,
  nested `renderer.*` work, and `frame.unattributed`.
- The digest should state whether local client work coincided with command bursts or late snapshots.
- Prediction fields should say whether prediction was disabled, replaying too much work, correcting
  heavily, or simply absent because no local predicted overlay existed.
- Browser/device context should remain coarse and non-fingerprinting.

## Implementation Checklist

- [ ] Define the stable uploaded frame/render/prediction summary labels.
- [ ] Add report-window aggregation and clamping for top phase/counter groups.
- [ ] Extend protocol/report DTOs and parser output.
- [ ] Update docs with examples for interpreting local frame contribution.
- [ ] Add focused JS tests for aggregation, reset, label allowlisting, and clamping.
- [ ] Run client architecture checks for wiring changes.
- [ ] Mark this phase as done in this file in the implementation commit.

## Verification

- focused JS tests for frame/report aggregation
- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- `node scripts/check-client-architecture.mjs`
- focused parser fixture tests
- `git diff --check`

## Manual Test Focus

Run the browser client performance harness or a local match that triggers a known render-heavy
window. Confirm the uploaded summary names stable phase/counter groups and that raw recent frames,
stack traces, entity ids, and traces remain local-only. Confirm normal gameplay still sends bounded
net reports.

## Handoff Expectations

List the new uploaded client-context fields, their allowlisted labels, and their reset behavior.
Explain how to tell "client render was dominant" from "client render was incidental." Tell the next
phase what a complete evidence package should include.
