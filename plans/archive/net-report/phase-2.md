# Phase 2 - Command Timing And Correlation

## Phase Status

- [x] Done.

## Objective

Make command-response lag diagnosable as distinct milestones: local issue, browser socket send,
server receipt, sim consumption, snapshot receipt, browser apply, and next rendered frame. This phase
should answer whether a reported command delay was upload/Wi-Fi, server room/tick scheduling,
downstream snapshot delivery, browser processing, or rendering.

## Work

- Add `matchRunId` correlation to client reports so `client_net_report` rows can be joined directly
  with `match_started`, `match_ended`, server perf, snapshot, and writer timing logs.
- Choose the smallest server-receipt diagnostic that cleanly splits command upload from sim
  consumption:
  - preferred shape is a tiny reliable receipt keyed by `clientSeq` and server tick/time, or an
    equivalently precise aggregate if the wire message is judged too chatty;
  - do not include command payloads, unit ids, target ids, positions, or player-entered text.
- Extend client command diagnostics with report-window aggregates:
  - commands issued, socket-send accepted, server-received, sim-acknowledged, and rejected counts;
  - issue-to-server-receipt latest/max/p95;
  - server-receipt-to-sim-ack latest/max/p95 where available;
  - issue-to-sim-ack latest/max/p95;
  - ack-snapshot-received-to-applied latest/max/p95 if Phase 1 exposes apply timing;
  - oldest pending command age and max pending command count.
- Preserve the existing distinction that socket/room receipt is diagnostics-only and not the
  reconciliation acknowledgement. Prediction should continue dropping pending commands only after
  sim-consumption acknowledgement.
- Update Rust and JavaScript protocol mirrors, structured logging, `docs/design/protocol.md`, and
  tests for any new message or report fields.
- Update report classification so command milestone failures do not collapse into generic
  `network_rtt` or cumulative prediction-disable labels.

## Expected Touch Points

- `server/src/main.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/live_tick.rs`
- `server/src/lobby/launch.rs` if `matchRunId` is included in start payloads
- `server/crates/protocol/src/lib.rs`
- `server/crates/contract/src/lib.rs` if a shared DTO changes
- `client/src/net.js`
- `client/src/match.js`
- `client/src/prediction_controller.js`
- `client/src/protocol.js`
- `server/src/structured_log.rs`
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- `tests/client_contracts.mjs`
- `tests/protocol_parity.mjs`
- `tests/server_integration.mjs`
- `tests/tri_state/` scenarios if command milestone behavior changes or gains browser-lane coverage

## Implementation Checklist

- [x] Add or expose `matchRunId` in a way available to `ClientNetReport`.
- [x] Design and implement the server-receipt diagnostic with no raw command payload leakage.
- [x] Track command milestone aggregates in the client report window.
- [x] Keep sim-consumption acknowledgement as the only prediction reconciliation ack.
- [x] Extend Rust/JS protocol mirrors and structured logging.
- [x] Update protocol/perf docs and command-lifecycle interpretation notes.
- [x] Add focused integration/client/tri-state tests.
- [x] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- `node tests/server_integration.mjs`
- relevant `tests/tri_state/run.mjs` scenarios for command acknowledgement/receipt behavior
- `cargo test --manifest-path server/Cargo.toml -p rts-server command`
- `cargo test --manifest-path server/Cargo.toml -p rts-protocol client_net_report`
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If a filter is too broad, document the exact narrower tests run and why they cover the changed command
milestone surface.

## Manual Test Focus

Run a local two-player or one-player sandbox match and issue move, attack-move, build, train, rally,
and stop commands. Confirm command responsiveness is unchanged and the report aggregates show command
issue/receipt/sim-ack counts without exposing command payload details. If possible, use a browser
network throttle or artificial delay harness and confirm the milestone fields move in the expected
direction.

## Handoff Expectations

Explain the selected receipt design and why it is low-spam. List every command timing field and how to
interpret it during a lag report. Include one example diagnosis path that separates upload delay from
server tick delay, downstream snapshot delay, and browser apply/render delay.
