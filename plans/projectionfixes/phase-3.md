# Phase 3 - Full-World Event Projection

Status: done.

## Goal

Make full-world snapshots receive full-world transient events through a shared server projection
seam. This should fix dev full-world event gaps and provide the shared foundation for the separate
lab full-world event-bucket bug without weakening normal fog privacy.

## Scope

- Replace the current `FullWorld` event attachment behavior that removes one `player_id` bucket
  with a projection-aware event source.
- Ensure dev full-world and lab full-world can receive the union of events that explain the full
  world state.
- Keep normal active-player `PlayerFog` event delivery per-player.
- Keep replay/lab team union event delivery scoped to selected vision players until Phase 4 decides
  more precise union semantics.
- Preserve intentional global events such as `ArtilleryFiring`.
- Add focused server tests for dev full-world receiving events from more than the selected view
  player.
- If the known lab full-world P2 event-bucket bug is still open when this phase starts, implement
  the shared fix in a way that covers that case too; do not duplicate a lab-only workaround.

## Expected Touch Points

- `server/src/lobby/projection.rs`
- `server/src/lobby/live_tick.rs`
- `server/src/lobby/room_task/dev.rs`
- `server/src/lobby/room_task/lab.rs`
- `server/src/lobby/snapshots.rs`
- `server/src/lobby/room_task/tests/dev.rs`
- `server/src/lobby/room_task/tests/lab.rs`
- `docs/design/protocol.md` if event projection semantics are documented there

## Constraints

- Do not send full-world event unions to normal active players.
- Do not make spectator union semantics broader in this phase except where full-world policy
  explicitly applies.
- Keep `Game::tick()` event buckets unchanged unless a narrower room projection helper is not
  sufficient.
- Deduplicate event unions deterministically, preserving stable ordering where current snapshots or
  tests rely on it.

## Verification

- Run focused Rust tests for lobby projection/dev/lab paths, for example:

```bash
cargo test --manifest-path server/Cargo.toml lobby::room_task::tests::dev
cargo test --manifest-path server/Cargo.toml lobby::room_task::tests::lab
```

Adjust exact test filters to the final module names.

## Manual Testing Focus

Open a dev scenario and verify events from all visible players appear in full-world watch. In lab
full-world, issue commands as P1 and P2 and confirm transient effects, notices, and impacts appear
for both sides when the world state is visible.

## Player-Facing Outcome

Dev and lab full-world views stop showing "silent" world changes where the entity state updates but
the visual/audio/UI event that explains it is missing.

## Handoff

After implementation, summarize the event projection helper, which modes use full event union, and
which modes intentionally remain player- or team-scoped for Phase 4.
