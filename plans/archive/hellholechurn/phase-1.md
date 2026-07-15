# Phase 1 - Replay-Safe Scripted Lab Actions

## Phase Status

- [x] Done.

## Objective

Generalize bundled Lab scenario automation so a scenario driver can emit ordinary Lab mutations as
well as player commands at the existing pre-tick boundary. Apply and record those mutations through
the same replay/timeline invariants as operator-issued Lab work. Preserve the current Hellhole setup,
god-mode roster, 900-tick shuttle cadence, and all generated assets in this phase.

## Work

- Replace the command-only scenario-driver output with a small typed action representation that can
  carry either:
  - a player command plus `LabCommandOptions`, or
  - a replay-serializable `LabOp` such as `SpawnEntities`.
- Keep scenario actions bounded, ordered, and failure-isolated. A rejected scripted action should
  log a stable warning and leave the room alive rather than panic or partially record the action.
- Refactor the room's Lab mutation application enough to let trusted scenario actions reuse its
  timeline-cap handling, future truncation, replay serialization, operation ordering, and logging
  without pretending the action came from an untrusted client payload.
- Preserve current operator authorization and client Lab controls. Scenario automation must not
  expose a new arbitrary mutation surface to browsers or change who can operate a Lab room.
- Ensure the direct `Game` harness path can apply the same typed action sequence without depending
  on room, WebSocket, or timeline machinery.
- Keep all scripted actions before `Game::tick()`. Do not introduce an after-tick hook.
- Keep `LabReplayOperation::SpawnEntities` as the durable representation for resolved spawn batches
  unless a focused test demonstrates a real incompatibility.
- Make driver seek synchronization robust enough that a reconstructed tick does not enqueue a
  command or mutation already present in the retained timeline.
- Add focused room-task/timeline tests that exercise at least one scripted spawn batch and prove:
  - it applies once at the intended tick
  - its replay entry uses the existing spawn-batch operation
  - seeking to that tick reconstructs the same authoritative state
  - a second driver pass at the reconstructed tick does not duplicate it
  - replay artifact export/import remains valid
- Update `docs/design/server-sim.md` if the scenario-driver or `Game` API seam description changes.
- Mark this phase done in this file in the implementation commit.

## Expected Touch Points

- `server/src/lobby/lab_scenario_driver.rs`
- `server/src/lobby/room_task/lab/replay.rs`
- `server/src/lobby/room_task/lab.rs`
- `server/src/lobby/lab_timeline.rs`
- `server/src/lobby/lab_replay_operations.rs`
- `server/src/tools/hellhole_snapshot_stream.rs`
- `server/src/tools/hellhole_perf_harness.rs`
- `server/src/lobby/room_task/tests/lab_scenario_driver.rs`
- focused Lab replay/timeline tests
- `docs/design/server-sim.md` if its scenario automation description needs refresh

## Verification

- Focused Rust tests for `lobby::lab_scenario_driver`, room-task Lab scenario actions, and Lab
  timeline/replay behavior.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if the
  public `Game` seam or sim-module relationships change.
- `node scripts/check-lobby-architecture.mjs` for room-task ownership or mutation-boundary changes.
- `git diff --check`.

## Manual Test Focus

Open the current `supply-300-hellhole` Lab scenario, let it cross a scripted command boundary, seek
back to that boundary, and resume. Confirm the room remains alive, scripted actions are not doubled,
and the visible scenario behavior is unchanged from the pre-phase workload.

## Handoff Expectations

Describe the typed action and apply/record seam, the exact replay ordering rules preserved, and any
remaining assumption phase 2 must obey when it creates spawn batches. Name the focused tests run and
the core seek/export behavior a human should check after the complete workload is available.
