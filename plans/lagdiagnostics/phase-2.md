# Phase 2 - Command Lifecycle Diagnostics

## Phase Status

- [ ] Not started.

## Objective

Split command response delay into bounded lifecycle stages that an agent can interpret from one
incident digest. This phase should make `command_upload_delay`, `command_server_queue`, and
`command_response_delay` explainable without raw per-command log streams or command payload capture.

## Work

- Add server-side lifecycle timing around command ingress:
  - browser issue time already known to the client
  - WebSocket frame receive and deserialize time in `handle_connection`
  - `RoomEvent::Command` queued for the room
  - room actor starts handling the command
  - command receipt queued for the writer
  - command accepted into sim pending queue
  - sim consumes the client sequence in a snapshot net-status acknowledgement
- Summarize lifecycle data as report-window max/p95/latest/count buckets keyed by player and
  `matchRunId`.
- Preserve at most bounded top-N command exemplars by client sequence and stable command family, not
  raw command details, unit ids, positions, or targets.
- Add room-event queue delay fields so agents can distinguish network/upload delay from room actor
  backlog.
- Add receipt-send age or reliable-queue timing fields if the current writer counters cannot
  distinguish receipt delivery delay.
- Extend `ClientNetReport`, structured logs, parser classifications, and docs with the new lifecycle
  phases.
- Keep command acceptance, validation, prediction, and gameplay semantics unchanged.

## Expected Touch Points

- `server/src/main.rs`
- `server/src/lobby/mod.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/room_task/live.rs`
- `server/src/lobby/live_tick.rs`
- `server/src/lobby/connection.rs`
- `server/src/structured_log.rs`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `client/src/prediction_controller.js`
- `client/src/match_net_reporter.js`
- `client/src/protocol.js`
- `scripts/parse-net-report-logs.mjs`
- `docs/design/protocol.md`
- `docs/perf-tracing.md`

## Agent-Readable Output Requirements

- Parser output should show a command lifecycle waterfall by player: client/send, ingress, room
  queue, room handling, receipt delivery, sim ack, and ack snapshot apply.
- The digest should name the dominant command lifecycle stage for each bad window when evidence is
  available.
- If a command stage cannot be isolated, the digest should say exactly which stages are still
  combined.
- Command family labels must be stable and low-cardinality, such as `move`, `build`, `train`,
  `attackMove`, and `other`.
- Top-N exemplars must be bounded and should include time, player, family, stage maxima, and
  `clientSeq`, not command payloads.

## Implementation Checklist

- [ ] Define lifecycle stage names, units, and reset behavior.
- [ ] Add server ingress and room-queue timestamps without blocking the room task.
- [ ] Add bounded per-window command lifecycle aggregation.
- [ ] Add client/report DTO fields and serde/default compatibility.
- [ ] Extend structured logging and parser summaries.
- [ ] Update docs with field semantics and caveats.
- [ ] Add focused Rust and JS tests for defaults, aggregation, and parser output.
- [ ] Mark this phase as done in this file in the implementation commit.

## Verification

- `node tests/protocol_parity.mjs`
- `cargo test --manifest-path server/Cargo.toml -p rts-server connection_sink`
- `cargo test --manifest-path server/Cargo.toml -p rts-server structured_log`
- focused Rust tests for `ClientNetReport` serde defaults and structured-log classification
- focused JS tests for command report aggregation reset behavior
- focused parser fixture tests
- `node scripts/check-client-architecture.mjs`
- `git diff --check`

## Manual Test Focus

Run one local match long enough to emit a net report, issue a few normal commands, and confirm the
parser can show lifecycle fields without exposing command payloads. Confirm prediction-off and
prediction-on command sending still works and command sequence numbers remain monotonic.

## Handoff Expectations

List each lifecycle field, unit, window behavior, and whether it is client-sourced or server-sourced.
Call out any stages that remain combined because the implementation could not isolate them cheaply.
Tell the next phase how to correlate command lifecycle windows with snapshot lifecycle windows.
