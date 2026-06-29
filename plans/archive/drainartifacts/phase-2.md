# Phase 2 - Bounded Match-History Write Tracking

Status: Done.

## Goal

Keep normal match-history writes detached from room transitions, but make those detached tasks
trackable so deploy shutdown can wait for replay-backed writes within a bounded timeout.

## Scope

- Add a small match-history write tracker owned by lobby/drain-level state or a closely related
  helper.
- Route existing `end_match` persistence through the tracker instead of calling `tokio::spawn`
  directly from `room_task/lifecycle.rs`.
- Preserve current behavior for normal play: the room should enqueue/spawn the write and continue to
  lobby or post-match replay without awaiting Supabase.
- Expose a wait method that shutdown can call to wait until all currently tracked writes finish or a
  caller-supplied timeout/deadline expires.
- Ensure write failures still log and do not propagate into room tasks. The tracker should observe
  task completion, not convert DB write errors into gameplay errors.
- Add enough logging to distinguish "all match-history writes completed during shutdown" from
  "shutdown write wait timed out with N writes still pending".

## Expected Touch Points

- `server/src/lobby/mod.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/room_task/lifecycle.rs`
- possible new helper module under `server/src/lobby/`
- `server/src/main.rs` drain tests, if the wait primitive is surfaced there
- focused tests under `server/src/lobby/tests.rs` or `server/src/lobby/room_task/tests/*`
- `docs/design/match-history.md` slow-write/shutdown wording

## Implementation Notes

- Keep the tracker independent enough to unit-test with synthetic futures; avoid requiring a live
  Postgres database for task-counting and timeout tests.
- Waiting should be snapshot-based or generation-based so a shutdown wait can finish for writes
  known at the time it began without racing forever against unrelated future writes.
- The tracker should use bounded state. Completed handles must be removed so long-running servers do
  not leak memory.
- Do not force-end active matches in this phase. That belongs in phase 3.

## Verification

- Focused Rust tests that:
  - a tracked write increments pending count and decrements on completion
  - shutdown wait returns when tracked writes complete
  - shutdown wait times out or reports pending writes when a write future hangs
  - normal `end_match` still returns without awaiting the write future
- `cargo fmt --manifest-path server/Cargo.toml --check` or the repo-equivalent formatting command
  for touched Rust.
- `git diff --check`.

## Manual Testing Focus

No full manual gameplay test is required for this phase unless implementation touches visible lobby
flow. If testing manually, finish a local match with recording enabled against a safe local/test DB
and confirm the room returns to post-match replay without waiting on the DB write.

## Handoff Expectations

Describe the tracker API, where write tasks are registered, and what shutdown should call in phase
3. Include any timeout/logging behavior and known limits, especially whether the wait is
snapshot-based or waits for all currently pending writes.
