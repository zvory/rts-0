# Phase 6: Blockage Discovery And Order Response

Status: planned

## Goal

Make units react coherently when perspective pathing sends them into an unseen live building that
blocks progress.

## Scope

- Detect when a unit's planned path is blocked by live authoritative occupancy that was absent from
  the planning perspective.
- Refresh building memory if the blocker is now visible to the unit's owner.
- Apply the accepted phase 4 order behavior:
  - move orders repath or fail,
  - attack-move may attack or repath,
  - explicit attack may preserve target intent while dealing with blockers,
  - gather/build/rally orders follow their accepted policy.
- Prevent repath loops with bounded thresholds, cooldowns, or stuck counters.
- Avoid panic paths on stale blocker ids, destroyed blockers, missing memory, or invalid goals.

## Important Design Choices

- Discovery should come from live server facts, but only convert into player memory if visibility
  rules allow the player to know the blocker exists.
- Repathing should avoid thrashing when multiple units hit the same narrow blocker.
- A hidden blocker that becomes destroyed should stop blocking live movement immediately and should
  update pathing perspective when scouted or inferred by accepted rules.
- If workers are not supposed to auto-attack blockers, make that explicit in tests.

## Expected Touch Points

- `server/crates/sim/src/game/services/movement/`
- `server/crates/sim/src/game/services/move_coordinator.rs`
- `server/crates/sim/src/game/services/combat/`
- `server/crates/sim/src/game/entity/order.rs`
- Phase 1 memory module
- `docs/design/server-sim.md`
- Self-play or regression fixtures if hidden wall-off behavior needs end-to-end coverage

## Verification

- Unit tests for blocked move, attack-move, explicit attack, gather, and build transitions.
- `cd server && cargo test movement`
- `cd server && cargo test combat`
- `cd server && cargo test`
- Live suites after starting a server:
  - `node tests/server_integration.mjs`
  - `node tests/regression.mjs`
  - `node tests/ai_integration.mjs`

## Manual Testing Focus

- Move a unit toward a destination through a never-seen wall-off and confirm it is surprised, then
  repaths or fails according to the accepted design.
- Attack-move into a hidden enemy building and confirm the unit attacks or repaths according to the
  accepted design.
- Scout the wall-off first and confirm future paths avoid it.
- Destroy a blocker and confirm future movement can use the opened path once the perspective allows
  it.

## Handoff

The handoff should summarize actual order behavior, remaining edge cases, and player-facing patch
notes. It should also list any scenarios that still need playtest tuning rather than more code.
