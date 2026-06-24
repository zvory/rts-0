# Phase 1 - Command-Cadence Diagnostics

## Phase Status

- [x] Done.

## Objective

Add targeted, bounded diagnostics that make command-density-induced jitter testable from beta logs.
This phase should expose timing and counts around command bursts, reliable command receipts, snapshot
slot replacement/send age, client frame gaps, and prediction health without changing command
semantics or attempting a fix.

## Work

- Add server-side report fields or structured log rows that can answer:
  - how many accepted command receipts were emitted per player/report window
  - how many rejected command receipts were emitted per player/report window
  - how many reliable messages were drained before snapshots
  - whether a snapshot waited behind reliable messages
  - snapshot age at send, if measurable without high-cardinality logging
  - latest-only snapshot slot stored/replaced/closed counts per report window
- Add client-side report fields that can answer:
  - command burst density, such as max commands issued in a short bucket and total commands per
    report window
  - prediction disable reasons as stable bounded counters, not just total disable count
  - WASM prediction replay max milliseconds, replay ticks, and replay-budget exceeded count per
    report window
  - whether a predicted snapshot was present while snapshots were late
  - frame gap and worst frame phase during command bursts
- Update `scripts/parse-net-report-logs.mjs` so the incident summary can correlate command density
  with snapshot jitter/gaps/bursts, reliable-message pressure, frame stalls, and prediction health.
- Document the field meanings and caveats in the relevant protocol/perf docs. State clearly that HUD
  `jit` means snapshot arrival jitter.
- Preserve backwards compatibility for existing logs. Defaults should make old reports parse as
  unknown/zero rather than failing.
- Do not change gameplay, transport scheduling, prediction behavior, or command acceptance behavior.

## Expected Touch Points

- `client/src/match_net_reporter.js`
- `client/src/prediction_controller.js`
- `client/src/sim_wasm_adapter.js`
- `client/src/match_health.js`
- `client/src/net.js`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `server/src/structured_log.rs`
- `server/src/lobby/connection.rs`
- `server/src/lobby/snapshot_fanout.rs`
- `server/src/lobby/room_task/live.rs`
- `scripts/parse-net-report-logs.mjs`
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- focused tests near the touched JS/Rust parser/protocol code

## Implementation Checklist

- [x] Select a bounded field set and document rejected/noisy candidates.
- [x] Add client aggregation for command density and prediction health.
- [x] Add server aggregation or structured rows for reliable-message/snapshot timing.
- [x] Extend protocol/report DTOs and structured logging if new report fields are required.
- [x] Update the parser summary and JSON/TSV output for the new fields.
- [x] Update docs explaining how to interpret command-density jitter evidence.
- [x] Add focused tests for report defaults, aggregation resets, parser output, and classification.
- [x] Mark this phase as done in this file in the implementation commit.

## Verification

- `node scripts/parse-net-report-logs.mjs --format markdown <sample-log.jsonl>`
- `node tests/protocol_parity.mjs` if protocol/report shape changes
- focused JS tests for new report aggregation/parser behavior
- focused Rust tests for `ClientNetReport` defaults and structured-log classification
- `node scripts/check-client-architecture.mjs` if client wiring changes
- `git diff --check`

## Manual Test Focus

Run one local match long enough to emit a net report. Issue a burst of repeated move commands and
confirm the report/log includes command density, prediction health, and snapshot cadence fields while
normal gameplay still works. Do not attempt to validate or fix the stutter in this phase.

## Handoff Expectations

List every new diagnostic field, its unit, its reset/window behavior, and where it appears in the
parser output. Include the exact beta log query the next phase should run after deployment. Call out
any important missing signal that phase 2 should compensate for during manual reproduction.

## Phase 1 Handoff

New client `ClientNetReport` fields use milliseconds for `*Ms` and simulation ticks for `*Ticks`.
Report-window fields reset after each net-report upload: `snapshotLateFrameCount`,
`predictedSnapshotLateFrameCount`, `commandBurstBucketMs`, `commandBurstMax`,
`commandBurstFrameGapMaxMs`, `commandBurstWorstFramePhase`, `commandBurstWorstFramePhaseMs`,
`predictionReplayMaxMs`, `predictionReplayMaxTicks`, and
`predictionReplayBudgetExceededCount`. Stable disable-reason buckets mirror the existing
match-lifetime `predictionDisableCount` semantics: `predictionDisableUserCount`,
`predictionDisableReplayCount`, `predictionDisableSpectatorCount`,
`predictionDisableCompatibilityCount`, `predictionDisableWasmCount`, and
`predictionDisableOtherCount`.

New server-only `client_net_report` log fields are consumed on the same per-connection report
window: `server_command_receipts_accepted`, `server_command_receipts_rejected`,
`server_reliable_drained_before_snapshot`, `server_reliable_drained_before_snapshot_max`,
`server_snapshot_waited_behind_reliable`, `server_snapshot_sent`,
`server_snapshot_send_age_latest_ms`, `server_snapshot_send_age_max_ms`,
`server_snapshot_send_age_avg_ms`, `server_snapshot_slot_stored`,
`server_snapshot_slot_replaced`, and `server_snapshot_slot_closed`.

`scripts/parse-net-report-logs.mjs` exposes the new fields in JSON/TSV summary rows, adds
`command_density` and `prediction_health` issue groups, folds server outbound pressure into the
WebSocket writer group, and adds markdown player columns for `cmds/burst` and server outbound
pressure (`reliableDrained/waited/ageMax/replaced`).

Rejected noisy candidates are intentionally not uploaded or logged: raw command payloads, unit ids,
target ids, positions, per-command timestamp arrays, raw frame records, raw phase arrays, stack
traces, raw snapshot payloads, and replay data. Phase 2 should treat the signal as window-level
correlation evidence, not exact per-command causality.

After deployment, use:

```bash
scripts/fly-logs.sh beta recent | rg 'client_net_report|performance writer timing|writer_send|performance tick summary|match_started|match_ended'
```
