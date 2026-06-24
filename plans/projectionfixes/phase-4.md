# Phase 4 - Union Vision Semantics

Status: done.

## Goal

Define and implement clean projection semantics for live spectator union, replay vision, and lab
team vision. This phase should fix only the cases with a clear projection-model mismatch, not churn
observer behavior for cosmetic purity.

## Scope

- Audit `SpectatorUnion`, `ReplayVision`, and lab `Team`/`Teams` projections after Phase 3.
- Replace live spectator `full_vision_events` with event projection that follows the same visible
  player set as the spectator snapshot, unless a specific event is intentionally global.
- Filter or classify owner-only notices so normal spectators do not receive player-private command
  failure/economy toasts unrelated to their projected view.
- Implement remembered-building projection for union views:
  - one-player replay vision shows that player's memory only
  - all-player/team union vision includes the selected players' memories
  - contradictory memories are preserved deliberately or tagged/deduped by a documented rule
- Decide and implement lab team `playerResources` behavior. Preferred rule: team vision should not
  expose unrelated teams' resources unless explicitly in full-world/all-player mode.
- Add focused tests for replay vision switching, lab team vision resources, spectator private
  notices, and union remembered buildings.

## Expected Touch Points

- `server/src/lobby/projection.rs`
- `server/src/lobby/live_tick.rs`
- `server/src/lobby/replay_session.rs`
- `server/src/lobby/room_task/lab.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/sim/src/game/building_memory.rs`
- `server/src/lobby/room_task/tests/live.rs`
- `server/src/lobby/room_task/tests/replay.rs`
- `server/src/lobby/room_task/tests/lab_timeline.rs`
- `docs/design/protocol.md`

## Constraints

- Do not erase a player's memory just because a spectator previously used all vision and then
  switches to one-player replay vision. The server should project the newly selected player's
  memory; the browser should not retain stale memory from the prior mode.
- Do not invent a "single truth" for contradictory memories unless the design explicitly chooses a
  conflict rule. It is acceptable for union vision to show multiple remembered facts if that is the
  honest projection.
- Preserve current live player fog privacy. This phase is about observer/lab/replay projections.
- Keep global artillery firing globally visible.
- Avoid large protocol changes unless remembered-memory source tagging proves necessary.

## Verification

- Run focused Rust tests around lobby replay/lab/live projection:

```bash
cargo test --manifest-path server/Cargo.toml lobby::room_task::tests::replay
cargo test --manifest-path server/Cargo.toml lobby::room_task::tests::lab
cargo test --manifest-path server/Cargo.toml lobby::room_task::tests::live
```

Adjust exact filters to the final module names.

## Manual Testing Focus

Watch a live match as a spectator and verify player-private command failures do not appear as
spectator toasts. In replay, switch from all vision to one player and confirm memory/resources match
that player's perspective. In lab team vision, verify entities and resources are scoped to the
selected team policy.

## Player-Facing Outcome

Spectator, replay, and lab team views become internally consistent: events, memory, and resources
match the selected vision model instead of leaking or dropping facts through implementation details.

## Handoff

After implementation, summarize the final union-memory rule, resource visibility rule, and any
events that intentionally bypass ordinary vision projection.
