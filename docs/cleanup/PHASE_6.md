# Phase 6 - Game and Lobby Seam Cleanup

Goal: make `server/src/game/mod.rs` and `server/src/lobby.rs` easier to maintain without changing
the public `Game` seam or lobby behavior.

## Target Components

For `server/src/game/mod.rs`:

- `game/mod.rs`: public API surface, module declarations, and re-exports.
- `game/setup.rs`: player initialization, base resource spawning, and start payload construction.
- `game/commands.rs`: enqueue/drain behavior and command validation delegation if it belongs at
  the `Game` boundary.
- `game/snapshot.rs`: per-player snapshot projection, resource deltas, event filtering, and replay
  full-world snapshot support.
- `game/scoring.rs`: entity score values and final player score construction.
- `game/tests.rs`: `Game` seam tests that do not belong to a lower-level service.

For `server/src/lobby.rs`:

- `lobby/mod.rs`: lobby handle/API, room lookup, and module declarations.
- `lobby/connection.rs`: connection sink/writer, latest snapshot slot, and send helpers.
- `lobby/room_task.rs`: room phase machine and match loop orchestration.
- `lobby/snapshots.rs`: compact snapshot preparation and broadcast behavior.
- `lobby/dev_replay.rs`: dev self-play room parsing, replay artifact loading, and replay controls.
- `lobby/crash_replay.rs`: crash replay dumping and panic reason formatting.
- `lobby/tests.rs`: lobby/room behavior tests.

## Design Notes

The `Game` API is a project invariant. Keep `lobby.rs` and `main.rs` using the documented public
methods instead of newly exposed internals. If a method signature changes, update `DESIGN.md` in the
same implementation change.

For lobby cleanup, keep room ownership single-threaded and avoid introducing locks around `Game`.
Extraction should reveal the existing phase machine; it should not create a new concurrency model.

## Tests

- Run `cargo test` in `server/`.
- Run Node integration scripts when lobby, snapshots, dev replay, or WebSocket behavior moves.

## Done

- `Game` module root shows the public seam clearly.
- Lobby room lifecycle is separated from connection and dev tooling details.
- No protocol, snapshot, or room lifecycle behavior changed unless explicitly documented.

