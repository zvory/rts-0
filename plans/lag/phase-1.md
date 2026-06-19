# Phase 1 - Effective-Tick Protocol and Diagnostics

## Phase Status

- [ ] Planned.

## Objective

Add the protocol and diagnostic contract for scheduled command execution while preserving current
live behavior. This phase should make command timing observable end to end before any server
scheduling or broader prediction depends on it.

## Scope

- Extend live gameplay command messages with an intended `executeTick` or equivalent compact field.
- Keep `clientSeq` as the command identity and preserve the existing sim-consumption ACK semantics.
- Add owner-only command result metadata keyed by `clientSeq` for:
  - requested execute tick
  - accepted execute tick
  - applied tick, when known
  - late-by tick count
  - accepted/rejected/no-op status where the server can state it safely
  - stable reason codes
- Expose the current per-player server command lead recommendation in owner-only snapshot net
  status or a similarly compact owner-only payload.
- Keep current command execution timing unchanged in this phase; this is a contract and
  observability phase.

## Expected Touch Points

- `server/crates/protocol/src/lib.rs`
- `server/crates/contract/src/lib.rs`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/snapshot_fanout.rs`
- `client/src/prediction_controller.js`
- `client/src/match.js`
- `docs/design/protocol.md`
- `tests/protocol_parity.mjs`
- `tests/prediction_controller.mjs`
- `tests/tri_state/scenarios/*command*`

## Verification

- Add protocol parity coverage for the new command and result fields.
- Add Rust protocol DTO tests for defaults and compact decode/encode behavior.
- Add prediction-controller tests proving receipt/result metadata is diagnostic until later phases
  consume it for reconciliation.
- Add or update a tri-state scenario that issues a command and records requested/accepted/applied
  tick metadata without changing visible behavior.
- Run:
  - `node tests/protocol_parity.mjs`
  - `node tests/prediction_controller.mjs`
  - focused Rust protocol tests
  - one focused tri-state command metadata scenario

## Manual Testing Focus

Start a local match with Movement prediction on and off. Issue move, train, and rally commands and
confirm normal gameplay behavior is unchanged while `window.__rtsPredictionDebug` exposes the new
command timing metadata.

## Handoff Expectations

The handoff must name the final field names, the stable reason codes, the default behavior for old
or missing execute ticks if any remains, and which later phase should start consuming the metadata.
