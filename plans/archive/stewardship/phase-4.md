# Phase 4 - Make Reconstruction Commit on Success

Status: Done.

## Objective

Make replay and Lab reconstruction failures recoverable without building a reconstruction
framework. Ordinary replay seeking must stop mutating the active session before success, while the
already-candidate-based Lab paths gain the same small panic-to-error boundary.

## Work

- Change ordinary replay seeking so it reconstructs candidate game/session fields from a keyframe
  and commits them only after all command replay and ticks succeed.
- Preserve the active replay game's tick, next-command cursor, keyframes, viewer state, controller
  state, and seek timing after either a normal reconstruction error or a panic. Candidate work may
  be discarded on failure; no partially reconstructed field may escape into the active session.
- Add one small reusable helper that catches a reconstruction panic and converts it, along with
  ordinary errors, into a structured internal failure suitable for room logging and user-facing
  error reporting.
- Apply that helper to ordinary replay seeking, Lab timeline seeking, and Lab replay import. Keep
  each workflow's existing candidate type and commit logic rather than forcing them behind one
  shared reconstruction object, trait, or transaction framework.
- Keep Lab seek/import candidate state isolated and commit it only after the wrapper returns
  success, preserving the current successful-path behavior. For Lab replay import, prepare one
  small commit bundle covering the replacement game/phase, timeline, cleared Lab driver, imported
  operator vision, initial camera, clean flag, and cleared operation log. Perform every fallible or
  panic-prone step before the first assignment; applying the prepared bundle must contain only
  demonstrably infallible field replacement.
- Add focused injected-error and injected-panic tests proving prior authoritative state remains
  intact and usable, plus success tests at the requested tick.
- Update `docs/design/server-sim.md` to describe candidate reconstruction, commit-on-success, and
  the bounded panic-containment seam.

## Non-goals

- Do not change replay, Lab, or checkpoint artifact formats; command semantics; keyframe intervals;
  seek cooldowns; room capabilities; fog; projection; or successful reconstruction results.
- Do not decompose `RoomTask`, add rollback machinery around unrelated room operations, or create a
  general transaction/reconstruction framework.
- Do not revisit typed identities or command limits completed in Phase 3.

## Likely Touch Points

- `server/src/lobby/replay_session.rs`
- `server/src/lobby/lab_timeline.rs`
- `server/src/lobby/room_task/replay.rs`
- `server/src/lobby/room_task/lab/replay.rs`
- a small adjacent reconstruction-failure helper
- focused room-task/replay/Lab tests
- `docs/design/server-sim.md`

## Verification

- Focused tests proving ordinary replay reconstruction error and panic leave the entire previous
  session state intact and usable.
- Focused tests proving Lab seek/import error and panic leave the previous authoritative game,
  timeline, driver, operator vision, initial camera, dirty flag, and operation log intact and
  usable. Inject the failure before the prepared import bundle's first assignment so the test
  covers the actual commit boundary.
- Focused success tests proving ordinary replay seek and Lab timeline seek commit at the requested
  tick. Prove Lab replay import instead commits at the artifact duration and applies its prepared
  game/phase, timeline, cleared driver, operator vision, initial camera, clean flag, and cleared
  operation log.
- `cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default -p rts-server`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `git diff --check`

## Manual Test Focus

Exercise an ordinary replay seek, a Lab rewind/seek, and a Lab replay import. Confirm a rejected or
malformed reconstruction reports an error, leaves the current room usable, and does not advance any
active replay/Lab cursor or timing state.

## Handoff

Mark this phase done in its implementation commit. Report the candidate fields now committed
atomically, every caller behind the panic wrapper, and the failure-state evidence for the Lab
driver and session metadata as well as game/timeline. Tell the next phase agent that server
contract ownership, command validation, and reconstruction behavior are stable; any remaining
stewardship work should not reopen them without new evidence.
