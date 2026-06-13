# Phase 1 - Shared Observer Analysis Contract

Status: Planned.

## Objective

Turn the current replay-only analysis contract into a shared observer analysis contract while
preserving existing behavior. After this phase, replay playback should behave exactly as it does
today, but code and documentation should no longer imply that the analysis system can only ever be
used by replays.

## Scope

- Rename internal Rust and JS symbols where practical from replay-specific names toward observer
  analysis names.
- Keep the existing wire tag `replayAnalysis` unless the implementation deliberately includes a
  compatibility path for a new tag. A cosmetic tag rename is not required for this plan.
- Update comments on `ServerMessage`, `Game::replay_analysis()` or its renamed equivalent, and the
  client overlay input path.
- Update `docs/design/protocol.md` so the payload is documented as observer analysis for replay
  viewers and live spectators, with active-player exclusion called out directly.
- Refresh `docs/context/protocol.md` section labels if the protocol design section heading changes.
- Do not send analysis to live spectators yet.

## Expected Touch Points

- `server/crates/protocol/src/lib.rs`
- `server/crates/sim/src/game/analysis.rs`
- `server/src/lobby/room_task.rs`
- `client/src/protocol.js`
- `client/src/match.js`
- `client/src/replay_analysis_overlay.js` or a renamed replacement
- `docs/design/protocol.md`
- `docs/context/protocol.md` only if section names change
- `tests/protocol_parity.mjs`
- Existing focused Rust protocol serialization tests

## Verification

Run focused checks that cover protocol shape and client imports:

```bash
node tests/protocol_parity.mjs
node scripts/check-client-architecture.mjs
cd server && cargo test -p rts-protocol replay_analysis_serializes_contract_shape
```

Adjust the exact Rust test filter if the test is renamed with the contract.

## Manual Testing Focus

Open a replay and confirm the analysis overlay still appears, receives data, survives seek-driven
viewer rebuilds, and keeps the same tab behavior as before. Confirm a normal live active-player
match does not show observer analysis.

## Handoff Expectations

The handoff must list the final contract names, whether the wire tag stayed as `replayAnalysis`,
and which compatibility assumptions Phase 2 must preserve. It should also call out any remaining
user-visible replay-only copy that Phase 3 should clean up.

## Player-Facing Outcome

No intended player-facing change. This phase pays down naming and documentation debt so live
spectator support can be added cleanly.

