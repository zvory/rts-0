# Room Task Split Plan

## Purpose

Make `server/src/lobby/room_task.rs` small enough to open, review, and reason about without losing
the current room-runtime model. The room task must remain the single Tokio owner of a room and its
`Game`, but mode-specific handlers, tests, and state helpers should move behind named room-local
modules. This is a behavior-preserving cleanup plan: no gameplay, protocol, balance, fog, replay
artifact, or match-history semantic changes are intended.

## Target Shape

The final shape should keep `room_task.rs` as the readable actor shell:

- `server/src/lobby/room_task.rs` - `RoomTask`, `run`, `handle_event`, phase/policy accessors, and
  very small shared send helpers.
- `server/src/lobby/room_task/types.rs` - room-owned data types, constants, and small constructors
  that other lobby modules need through stable re-exports.
- `server/src/lobby/room_task/lobby.rs` - ordinary lobby joins, leaves, readiness, host fallback,
  teams, factions, AI seats, quickstart, map selection, and public lobby summaries.
- `server/src/lobby/room_task/live.rs` - live-match commands, command receipts, pause/give-up,
  late spectator attach, and live start glue around `live_tick.rs` and `launch.rs`.
- `server/src/lobby/room_task/lab.rs` - `LabSession`, lab joins, lab request authorization, lab
  mutation routing, lab results, lab state broadcast, and lab scenario export/import.
- `server/src/lobby/room_task/dev.rs` - dev-watch joins, dev scenario launch, scripted dev tick
  driver glue, dev room-time controls, and dev start payload sends.
- `server/src/lobby/room_task/replay.rs` - replay joins, replay-room prompts, replay start payload
  resends, replay room-time speed/step/seek, replay vision, observer analysis, and return-to-lobby.
- `server/src/lobby/room_task/branch.rs` - branch-staging room joins, branch seat claim/release,
  branch launch glue, branch live attach, and branch staging broadcasts around `replay_branch.rs`.
- `server/src/lobby/room_task/lifecycle.rs` - start/end/reset/drain/match-history bookkeeping,
  post-match replay transition, empty-room disposal, and performance tick logging.
- `server/src/lobby/room_task/tests/` - focused room-task tests split by mode or behavior family.

## Phase Summaries

### [Phase 1 - Test Split And Baseline](phase-1.md)

Split the in-file `room_task.rs` test module into focused child modules under
`server/src/lobby/room_task/tests/`. Keep the assertions and helper behavior the same, but group
tests by lobby, live, replay, lab, branch, dev, and lifecycle responsibilities. Record the current
runtime line-count baseline and the room paths that still need manual smoke testing before
production handler movement begins.

### [Phase 2 - Module Skeleton And Shared Types](phase-2.md)

Create the `server/src/lobby/room_task/` production module skeleton and move only low-risk shared
types, constants, and pure helper constructors behind stable root re-exports. Keep
`RoomTask::run`, `handle_event`, phase transitions, and all behavior handlers in the root file for
this phase. This establishes the privacy and import pattern that later phases must follow without
combining it with mode behavior moves.

### [Phase 3 - Lobby Control Split](phase-3.md)

Move ordinary lobby control handlers into `room_task/lobby.rs`: public summary construction, normal
joins and leaves, readiness, host reassignment, team/faction/AI/map/spectator controls, quickstart,
and lobby broadcasts. Keep the root event dispatcher as the only place that maps `RoomEvent` variants
to room behavior, and expose moved methods only as `pub(super)` where the root dispatcher needs them.
This should make lobby admission and roster policy reviewable without loading replay, lab, live tick,
or match-history code.

### [Phase 4 - Lab And Dev Mode Split](phase-4.md)

Move lab-specific state and handlers into `room_task/lab.rs`, including `LabSession`, lab role
metadata, lab operation conversion, authorization, mutation routing, result delivery, and state
broadcast. Move dev-watch/scenario glue into `room_task/dev.rs`, leaving the existing
`dev_replay.rs`, `tick_control.rs`, `projection.rs`, and `launch.rs` helpers in place. This phase
keeps lab mutation centralized in room-task ownership while removing two specialized mode bodies from
the actor shell.

### [Phase 5 - Replay And Room-Time Split](phase-5.md)

Move replay viewer joins, replay-room prompts, replay start payload sends, replay vision, seek,
speed, step, room-time state, observer analysis, and return-to-lobby replay behavior into
`room_task/replay.rs`. Continue using `replay_session.rs` for replay playback state and keyframe
logic; this phase only moves room-owned event handling and send orchestration. Preserve replay
capabilities, per-viewer vision, seek cooldown behavior, and replay prompt semantics exactly.

### [Phase 6 - Live And Branch Handler Split](phase-6.md)

Move live-match control handlers into `room_task/live.rs`, including command routing, command
receipts, pause/unpause, give-up, late spectator attach, live start payload glue, and live snapshot
notice plumbing. Move branch-staging and branch-live room handlers into `room_task/branch.rs` while
keeping reusable seat policy in `replay_branch.rs`. This phase leaves the existing `live_tick.rs`
driver and `replay_branch.rs` state helper intact, but makes their room-owned callers local to the
matching mode files.

### [Phase 7 - Lifecycle And Runtime State Cleanup](phase-7.md)

Move start/end/reset/drain/match-history bookkeeping into `room_task/lifecycle.rs` and introduce
small room-owned state structs only where they reduce illegal optional-field combinations. Candidate
state groups include live pause state, match identity/history state, room-time playback state, and
empty-room disposal bookkeeping. Do not introduce locks, trait-object mode dispatch, or a public
`Game` API change; the goal is a clearer owned-state shape, not a new runtime model.

### [Phase 8 - Guardrails And Documentation Closeout](phase-8.md)

Update the server simulation docs and context capsule to describe the final room-task module map and
which file owns each runtime concern. Add or tighten `scripts/check-lobby-architecture.mjs` guardrails
so `room_task.rs` and its child modules have explicit size and boundary budgets that prevent the root
file from growing back. Rerun hotspot analysis and record the room-runtime group tracking so future
cleanup sees the split files as one ownership area instead of losing history.

## Overall Constraints

- Start every implementation phase from fresh `origin/main` in an isolated `/tmp/rts-worktrees`
  worktree on a `zvorygin/` branch.
- Preserve unrelated dirty state, especially `playtest_notes.md`.
- Each phase must be pushed as an owned PR with auto-merge armed, then waited on until GitHub
  reports the PR merged and the phase head is reachable from `origin/main`.
- When a phase is complete, mark that phase document as done in the same implementation commit.
- Keep `RoomTask::run` as the single event/tick owner. Do not add locks around `Game`, move `Game`
  ownership to another task, or introduce shared mutable simulation state.
- Keep `handle_event` in `room_task.rs` until a later approved design says otherwise. The root file
  should remain the readable event map for the room actor.
- Child modules may define `impl RoomTask` blocks, but moved handler methods should be
  `pub(super)` only when called by the root dispatcher or another sibling through an explicit helper.
- Preserve `SessionPolicy`, `ProjectionPolicy`, `Participants`, `TickControl`, `launch.rs`,
  `live_tick.rs`, `replay_session.rs`, `replay_branch.rs`, `snapshot_fanout.rs`, and
  `connection.rs` as the existing helper boundaries unless a phase explicitly says to move a small
  caller-side wrapper.
- Do not change gameplay behavior, room lifecycle ordering, command semantics, replay artifact
  format, protocol tags/fields, balance values, fog/projection policy, match-history gating, or DB
  write behavior.
- If a phase touches message construction in a way that could affect wire shape, read
  `docs/context/protocol.md`, update the relevant protocol docs if needed, and run protocol parity.
- If a phase touches lab mutation routing, keep accepted mutation and issue-as calls centralized in
  room-task ownership and update `scripts/check-lobby-architecture.mjs` only with a documented
  boundary reason.
- Treat test splitting as moves and local helper extraction, not as permission to delete coverage or
  rewrite assertions.
- Keep the final root `room_task.rs` production file small enough for an agent to load for event
  orientation. Phase 8 should set the exact guardrail from the achieved size rather than blessing a
  large file.
- If new split paths are not already grouped by `scripts/hotspot-analysis.mjs`, update both
  `scripts/hotspot-analysis.mjs` and `docs/hotspot-analysis.md` in the phase that creates or
  finalizes those paths.
- Use focused verification during phase work. The PR gate remains the authoritative full
  `./tests/run-all.sh` check.
- Every phase handoff must state what changed, what the next agent should do, which focused commands
  passed, what uncertainty remains, and which core behavior should be manually tested.

## Suggested Execution

Run one phase at a time and wait for each PR to merge before starting the next phase:

```bash
scripts/phase-runner.sh --plan roomsplit phase-1 phase-2 phase-3 phase-4 phase-5 phase-6 phase-7 phase-8 --pr --wait
```

For a lower-risk first wave, run only the test split and module skeleton:

```bash
scripts/phase-runner.sh --plan roomsplit phase-1 phase-2 --pr --wait
```
