# Phase 7 - Lifecycle And Runtime State Cleanup

Status: not started.

## Goal

Move room lifecycle bookkeeping into a focused module and introduce small owned-state structs where
they make illegal optional-field combinations harder to create.

## Scope

- Move match countdown, match start, branch live start, post-match replay transition, match end,
  score/team resolution, match-history dispatch, replay artifact capture/attachment policy, drain
  accounting, empty-room reset, disposal reporting, live-match identity reset, live pause reset, and
  performance tick logging into `room_task/lifecycle.rs`.
- Consider small state structs for live pause state, match identity/history state, room-time playback
  state, drain tracking, and pending recipient notices if they reduce root-field sprawl without
  changing behavior.
- Keep `Phase` transitions explicit and auditable. If a broader `RoomRuntime` enum looks attractive,
  stop and write a follow-up design note instead of doing a large state-machine rewrite in this
  phase.
- Preserve detached match-history writes and environment/policy gating.
- Keep `RoomTask::run` and `handle_event` in the root file.

## Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/room_task/lifecycle.rs`
- `server/src/lobby/room_task/types.rs` if small state structs belong there
- Lifecycle and match-history-focused room-task tests
- `scripts/check-lobby-architecture.mjs` only if moved persistence-policy guardrails need a precise
  new target
- `plans/roomsplit/phase-7.md`

## Constraints

- Do not change match countdown timing, start payload ordering, active match accounting, game-over
  resolution, score construction, post-match replay behavior, match-history eligibility, replay
  artifact capture policy, drain behavior, or empty-room reset semantics.
- Do not block the room task on DB writes.
- Do not introduce locks, async tasks around `Game`, trait-object mode dispatch, or a public `Game`
  API change.
- Keep policy checks such as `should_persist_match_history`,
  `should_capture_post_match_replay`, and `should_attach_match_history_replay_artifact` easy to
  audit near `end_match`.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lifecycle`
- `cargo test --manifest-path server/Cargo.toml -p rts-server history`
- `cargo test --manifest-path server/Cargo.toml -p rts-server drain`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `node scripts/check-lobby-architecture.mjs`
- `git diff --check`

## Manual Testing Focus

Manually check normal lobby start through game-over to post-match replay, return-to-lobby after
replay, empty public-room reset, empty private replay/branch/lab/dev room behavior, and drain-start
behavior while a live match is active.

## Handoff

After implementation, mark this phase done and summarize the lifecycle module boundary, any new
small state structs, commands run, manual checks performed or still needed, and whether a later
explicit `RoomRuntime` state-machine design is still worth planning.
