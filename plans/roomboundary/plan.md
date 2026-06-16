# Room Boundary Refactor Plan

## Purpose

Refactor `server/src/lobby/room_task.rs` by extracting stable lobby-owned helpers while preserving
the current runtime model. The room task remains the single owner of room state and `Game`, but
replay, fanout, live ticking, branch staging, and lifecycle bookkeeping should stop being one
porous module.

## Overall Constraints

- Keep `RoomTask::run` as the single event/tick owner; do not add locks around `Game`.
- Keep AI orchestration, transport, connection sinks, room registry policy, DB writes, and Tokio
  coordination out of `rts-sim`.
- Preserve the `Game` API seam and do not change protocol shapes unless a phase explicitly updates
  Rust protocol, JS mirror, parity tests, and docs.
- Treat normal live matches, spectators, replay viewers, replay branches, dev self-play watch,
  observer analysis, post-match replay, and empty-room reset as first-class paths.
- After each phase, provide a handoff naming verification results, remaining risks, and the core
  lifecycle paths that should be manually tested.
- Implement, commit, merge to `main`, and push each phase before starting the next phase.

## Phase Summaries

### [Phase 1 - Baseline Characterization](phase-1.md)

Record current `room_task.rs` responsibilities and add focused coverage for under-specified room
lifecycle behavior. This phase should identify which current behavior is protected by tests and
which remains manual. Production movement should be avoided except for harmless test helpers.

### [Phase 2 - Replay Runtime Extraction](phase-2.md)

Move replay session state and seek/keyframe logic into a lobby-local replay runtime module. The
room task should still own phase transitions and connection sends. Replay playback should become a
small API rather than a large in-file subsystem.

### [Phase 3 - Snapshot Fanout And Observer Delivery](phase-3.md)

Extract repeated live, replay, branch, and dev snapshot fanout plumbing into a lobby-local helper.
The helper should centralize compacting, net-status metadata, perf accounting, and projection-mode
selection. Fog differences between player, spectator, replay vision, branch-live, and dev full-world
views must remain explicit.

### [Phase 4 - Live Tick Driver And AI Adapter](phase-4.md)

Split live match ticking into a small driver that sequences AI command enqueue, `Game` ticking,
fanout, observer analysis, defeat checks, and panic replay capture. AI controllers should remain
outside `Game` and feed ordinary `SimCommand`s through the existing seam. This phase should make
the server/sim boundary easier to audit.

### [Phase 5 - Replay Branch Module](phase-5.md)

Move branch staging state, seat claim/release policy, branch messages, and branch launch
preparation into a lobby-local branch module. `RoomTask` should still own membership, host identity,
connection sends, and final phase transitions. Original replay player mapping must remain intact
for branch commands, snapshots, and outcomes.

### [Phase 6 - Lifecycle Cleanup And Docs](phase-6.md)

Consolidate start/end/reset/drain bookkeeping after the cohesive subsystems have been extracted.
Update server simulation docs to describe the new lobby module boundaries. Add a small guardrail
only if the earlier phases reveal a repeatable boundary failure.

## Non-Goals

- Do not move room transport or connection ownership into lower crates.
- Do not change replay artifact formats, branch protocol shapes, or match-history semantics unless
  a phase explicitly says so.
- Do not combine lifecycle cleanup with replay or fanout extraction in a single large phase.

## Handoff Rules

Each phase file has an implementation checklist. Handoffs must include exact verification commands,
manual testing focus, and a note about whether the next phase can proceed or a discovered gap should
be fixed first.
