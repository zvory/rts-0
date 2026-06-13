# Phase 3 - Server Analysis Contract

Status: Done.

## Objective

Add authoritative, seek-safe replay analysis state for data that cannot be derived correctly from
the browser's current projected snapshot. This phase defines and implements the server/protocol
contract for production, unit inventory, units lost, and resources lost analysis.

## Contract Shape

Prefer a replay-only reliable or snapshot-adjacent message rather than expanding normal player
snapshots by default. The payload should include the replay tick it describes and enough per-player
records for the client to render tabs immediately after playback, vision changes, and seeks.

Candidate semantic shape:

```text
replayAnalysis {
  tick,
  players: [
    {
      id,
      units: [{ kind, count, steelValue, oilValue }],
      production: [{ buildingId, buildingKind, itemKind, itemType, progress, queueDepth }],
      unitsLost: [{ kind, count, steelValue, oilValue }],
      resourcesLost: { steel, oil }
    }
  ]
}
```

The final design may split this by tab if payload size or update frequency argues for separate
messages. Document the selected shape in `docs/design/protocol.md`.

## Scope

- Define the replay analysis protocol in:
  - `server/crates/protocol/src/lib.rs`
  - `server/src/protocol.rs` if adapter changes are needed
  - `client/src/protocol.js`
  - `docs/design/protocol.md`
- Add server-side analysis state owned by replay playback, not by normal active client trust.
- Ensure analysis state is rebuilt consistently when `ReplaySession::rebuild_to()` restores a
  keyframe and fast-forwards to the target tick.
- Decide whether analysis keyframes should be part of `ReplayKeyframe` or recomputed from the
  cloned `Game` at the rebuilt tick.
- Track production from authoritative building state, including item identity, progress, and queue
  depth.
- Track current unit composition and value from authoritative entities.
- Track units lost and value lost from authoritative death/removal paths.
- Define `resourcesLost` precisely before implementation. Recommended first definition: spent
  steel/oil value of units that died, not all harvested/spent economy. If the desired definition is
  broader, explicitly model spending/cancellation/refund semantics before coding.
- Keep payloads bounded by player count, known unit/building kinds, and reasonable queue summaries.

## Expected Touch Points

- `docs/design/protocol.md`
- `docs/context/protocol.md` if section pointers shift
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `server/src/lobby/room_task.rs`
- `server/crates/sim/src/game/` analysis helpers or score/death tracking locations
- `client/src/protocol.js`
- protocol parity and replay/session tests

## Verification

- Rust protocol serialization/deserialization tests for the new message or fields.
- JS protocol decode/build tests for the mirror.
- Replay session tests proving analysis state:
  - matches current authoritative game state at normal playback ticks
  - is correct immediately after `seekReplayTo`
  - does not double-count losses during seek fast-forward
  - is scoped to replay/spectator contexts only
- Run focused protocol and replay tests. Exact commands depend on final touched crates, but should
  include targeted Rust tests and the JS protocol parity suite used by this repo.

## Manual Testing Focus

Open a replay, let it run, seek to several points before and after major fights, and inspect a
temporary debug dump or minimal UI to confirm production, units, and losses match visible replay
state. Change replay vision and confirm the server behavior matches the documented analysis
visibility policy.

## Handoff Expectations

The handoff must include the final protocol shape, visibility policy, resource-loss definition,
seek/keyframe strategy, and exact tests run. The next agent should use this payload for UI only and
avoid adding client-side history reconstruction.

## Player-Facing Outcome

No complete new tab UI is required in this phase, but the replay system now has authoritative data
that remains correct across arbitrary seeking.
