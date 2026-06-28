# Phase 3 - Shutdown Abort Finalization

Status: Not started.

## Goal

When deploy drain cannot wait for every active match to finish naturally, force-finalize remaining
eligible live matches as `aborted` and capture their replay artifacts before closing WebSocket
connections.

## Scope

- Split the deploy drain budget into:
  - natural match-drain time
  - forced abort/finalization time
  - match-history write wait time
  - final WebSocket/Axum shutdown slack
- Add a lobby method that asks active room tasks to finalize for shutdown and returns when rooms ack
  or a bounded timeout expires.
- Add a room event such as `FinalizeForShutdown` or `AbortForShutdown` with an ack channel.
- In active normal live rooms that are eligible for match history, take the current `Game`, compute
  current scores, build a replay-backed `MatchRecord` with `outcome = aborted`, `winner_name = None`,
  and register the write through the phase 2 tracker.
- Mark drain tracking finished only after the active game has either been finalized or was already
  no longer active.
- Make non-eligible active authoritative sessions ack safely without writing public match-history
  rows.
- After forced finalization, wait for tracked writes within the reserved write budget, then close
  connections.

## Expected Touch Points

- `server/src/main.rs`
- `server/src/lobby/mod.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/room_task/lifecycle.rs`
- `server/src/lobby/room_task/lobby.rs`
- `server/src/lobby/room_task/live.rs` if record-building needs extraction from `end_match`
- `server/src/lobby/session_policy.rs` if shutdown-finalization eligibility belongs in policy
- focused deploy-drain and room lifecycle tests
- `docs/design/hardening.md`
- `docs/fly.md`

## Implementation Notes

- Factor match-history record construction out of `end_match` rather than duplicating replay
  capture and score serialization logic.
- Do not transition shutdown-aborted rooms into post-match replay viewer; the process is exiting.
  The persisted replay row is the replay artifact that matters.
- Do not rely on `on_leave` to create aborted rows. Connection shutdown currently sends `Leave`, and
  empty-room reset drops game state; forced finalization must happen before that path.
- It is acceptable for clients to receive only the existing shutdown warning before disconnect if
  there is not enough time to deliver a `GameOver`. The persisted Recent Matches row is the product
  requirement for this phase.
- Log enough structured context to audit the flow: active matches at natural timeout,
  rooms requested for abort, rooms acked/timed out, write wait result, and pending write count if
  any.

## Verification

- Focused Rust tests that:
  - deploy drain still waits for a naturally finishing active match
  - drain timeout triggers forced abort before connection shutdown
  - a shutdown-aborted eligible live match records `outcome = aborted`, no winner, score screen, and
    replay payload
  - room empty/reset after connection shutdown no longer discards an already-finalized active match
  - non-eligible rooms do not produce match-history rows
  - write wait is bounded when a tracked write hangs
- `cargo fmt --manifest-path server/Cargo.toml --check` or equivalent.
- `git diff --check`.

## Manual Testing Focus

Run a local server with a shortened drain timeout and recording enabled against a safe local/test DB.
Start a match, trigger shutdown before the match resolves, and confirm the server logs forced abort
finalization before connection shutdown. Confirm the stored match row has `outcome = aborted`,
`winner_name = null`, score detail, and a replay row.

## Handoff Expectations

State the final drain timing constants, what room modes are finalized versus skipped, and the exact
shutdown log lines an operator should look for. Note any remaining beta-only validation that phase 4
must perform.
