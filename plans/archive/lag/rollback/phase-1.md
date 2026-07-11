# Phase 1 - Effective-Tick and Rollback Protocol

## Phase Status

- [ ] Planned.

## Objective

Add the protocol and diagnostic contract for scheduled command execution and bounded rollback while
preserving current live behavior. This phase should make command timing, rollback eligibility, and
fallback execution observable end to end before any server scheduling or broader prediction depends
on it.

## Scope

- Extend live gameplay command messages with optional `executeTick: u32`.
  - Prediction-enabled clients stamp compatible live commands with `executeTick`.
  - Prediction-disabled clients omit `executeTick`; the server treats the command as unscheduled
    and preserves the current next-authoritative-tick behavior.
  - The server must reject `executeTick = 0`, wrapped values, and values outside the documented
    future/history bounds with stable result metadata rather than panicking.
- Keep `clientSeq` as the command identity and preserve the existing sim-consumption ACK semantics.
- Add a bounded owner-only command result payload keyed by `clientSeq`. Prefer a dedicated
  `commandResults` snapshot field or equally explicit owner-only payload over overloading the
  scalar `SnapshotNetStatus` fields. Each result entry should use stable field names for:
  - requested execute tick
  - accepted execute tick
  - applied tick, when known
  - late-by tick count
  - rollback-window eligibility
  - rollback applied/clamped/skipped/fallback status
  - rollback replay tick count and elapsed time when available
  - envelope/result status, separated into at least `received`, `scheduled`, `applied`,
    `rejected`, `noop`, and `lateFallback` where the server can state it safely
  - stable reason codes
- Expose the current per-player server command lead recommendation in owner-only snapshot net
  status or a similarly compact owner-only payload.
- Document `ROLLBACK_WINDOW_TICKS = 6` as the initial product target, exactly 200 ms at 30 Hz. Keep
  it configurable or centralized so later phases can tune it without hunting constants.
- Document the hard catch-up command fuse, initially `MAX_REPLAY_COMMANDS = 1000`, as a safety cap
  rather than a normal tuning path.
- Define initial stable reason codes in this phase, even if some are produced only by later phases:
  `invalidSeq`, `staleSeq`, `notInGame`, `notPlayer`, `notJoined`, `executeTickMissing`,
  `executeTickInvalid`, `executeTickTooOld`, `executeTickTooFarFuture`, `rollbackEligible`,
  `rollbackApplied`, `rollbackClamped`, `rollbackWindowMiss`, `rollbackCommandCapExceeded`,
  `rollbackUnsupported`, `lateDuringReplay`, `authoritativeNoop`, `rejectedOwnership`,
  `rejectedInvalidTarget`, `rejectedCost`, and `unsupportedCommand`. If a code is intentionally not
  implemented yet, document it as reserved.
- Bump `PREDICTION_PROTOCOL_VERSION` and update the compact snapshot contract only after the Rust
  and JS mirrors, protocol docs, and parity fixture agree on the new result payload shape.
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
- Add tests proving omitted `executeTick` preserves prediction-off behavior and stamped
  `executeTick` is diagnostic-only until Phase 3.
- Add tests proving malformed, too-old, and too-far-future ticks produce stable result metadata
  without advancing `lastSimConsumedClientSeq`.
- Add prediction-controller tests proving receipt/result metadata is diagnostic until later phases
  consume it for reconciliation.
- Add or update a tri-state scenario that issues a command and records requested/accepted/applied
  tick and rollback metadata without changing visible behavior.
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

The handoff must name the final field names, the rollback constants, the stable reason codes, the
default behavior for missing execute ticks, the owner-only result payload shape, the bounded result
list size/expiry policy, and which later phase should start consuming the metadata.
