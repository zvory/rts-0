# Phase 5 - Lab Timeline Recording

## Phase Status

- [x] Done.

## Objective

Create the room-local lab timeline model needed for rewind without enabling seek yet.

## Work

- Introduce a focused lab timeline structure owned by the room task or a new lobby-local module. It
  should not live inside `Game` and should not depend on replay artifact storage.
- Record an initial baseline keyframe for every lab room after the authoritative `Game` is created.
- Record periodic `Game::clone_for_replay_keyframe()` lab keyframes at a documented interval, with a
  documented cap or retention policy so long-running labs cannot grow memory without bound.
- Record accepted privileged lab operations in tick order with enough typed data to replay them from
  a keyframe. Include request id, operator id, operation kind, and the operation payload actually
  applied by the server.
- Record lab issue-as commands in the same timeline stream before or when they are queued, preserving
  the real player id and command payload used for authoritative command validation.
- Treat scenario import as a timeline reset. Once an import restores a new world, clear previous
  timeline history, create a new baseline, and continue recording from the restored state.
- Broadcast or send lab `roomTimeState` with current tick, duration/current maximum tick, keyframe
  ticks, speed, pause state, and controller id as appropriate. Do not advertise seek/timeline
  controls until Phase 6.
- Keep the existing human-readable lab operation count and dirty state working.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/lab_timeline.rs` or another focused new lobby-local module
- `server/src/lobby/mod.rs`
- `server/src/lobby/live_tick.rs` if keyframe recording hooks belong near live tick completion
- `server/crates/sim/src/game/mod.rs` only if a public clone/keyframe helper needs clearer naming or
  comments
- `server/crates/protocol/src/lib.rs` / `server/crates/contract/src/lib.rs` only if `RoomTimeState`
  docs or defaults need adjustment
- `docs/design/server-sim.md`
- `docs/design/protocol.md`

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lab_timeline`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim replay_keyframe`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if sim
  public helpers or architecture budgets are touched
- `git diff --check`

If a proposed filter runs zero tests, replace it with the exact test names added by this phase and
report that in the handoff.

## Manual Test Focus

No browser smoke is required unless visible room-time state changes. A cheap manual check may open a
lab, perform several lab operations and issue-as commands, pause, and confirm server logs or test
instrumentation show keyframes and timeline entries being recorded.

## Handoff Expectations

State the keyframe interval, retention/cap behavior, import reset semantics, and exact typed entries
the timeline records. Also state whether Phase 6 can rebuild arbitrary ticks or only ticks at or
after recorded keyframes.
