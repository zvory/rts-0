# Phase 7 - Make Reconstruction Commit on Success

Status: Incomplete.

## Objective

Make replay and Lab reconstruction failures recoverable without building a reconstruction
framework. Ordinary replay seeking must stop mutating the active session before success, while the
already-candidate-based Lab paths gain the same small panic-to-error boundary.

## Work

- Change ordinary replay seeking so it reconstructs candidate game/session fields from a keyframe
  and commits them only after all command replay and ticks succeed.
- Preserve the active replay game's tick, next-command cursor, keyframes, viewer state, controller
  state, and seek timing after either a normal error or a panic.
- Add one small reusable helper that catches a reconstruction panic and converts it, along with
  ordinary errors, into a structured internal failure suitable for room logging and user-facing
  error reporting.
- Apply that helper to ordinary replay seeking, Lab timeline seeking, and Lab replay import. Keep
  each workflow's existing candidate type and commit logic rather than forcing them behind one
  shared reconstruction object or trait.
- Commit Lab seek/import candidates only after the wrapper returns success, preserving their current
  behavior.
- Add focused injected-error and injected-panic tests proving prior authoritative state remains
  usable, plus success tests at the requested tick.
- Update the server-simulation design source of truth to describe commit-on-success reconstruction
  and panic containment.

## Non-goals

- Do not change replay or Lab artifact formats, command semantics, keyframe intervals, seek
  cooldowns, room capabilities, fog, or projection.
- Do not decompose `RoomTask` or create a general transaction/reconstruction framework.
- Do not revisit typed identities or command limits completed in Phases 5 and 6.

## Likely Touch Points

- `server/src/lobby/replay_session.rs`
- `server/src/lobby/lab_timeline.rs`
- `server/src/lobby/room_task/replay.rs`
- `server/src/lobby/room_task/lab/replay.rs`
- a small adjacent reconstruction failure helper
- focused room-task/replay/Lab tests
- `docs/design/server-sim.md`

## Verification

- Focused tests proving replay error and panic leave the entire previous session state intact.
- Focused tests proving Lab seek/import error and panic leave the previous authoritative game and
  timeline intact.
- Focused success tests proving all three callers still commit at the requested tick.
- `cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default -p rts-server`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `git diff --check`

## Manual Test Focus

Exercise an ordinary replay seek, a Lab rewind/seek, and a Lab replay import. Confirm a rejected or
malformed reconstruction reports an error and leaves the current room usable.

## Handoff

Mark this phase done in its implementation commit. Report the candidate fields now committed
atomically, each caller behind the panic wrapper, and the failure-state evidence. Tell the Phase 8
agent that server command and replay behavior is stable and the remaining work is client-only.
