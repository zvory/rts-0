# Phase 2: Artillery Uses Remembered Buildings

Status: implemented

## Goal

Make artillery use remembered building intel as player-facing coordinate context, without adding
hidden building entity targeting. Artillery remains a world-point `pointFire` ability; remembered
building silhouettes let the player know where previously scouted structures were so they can aim
at those stale positions.

## Scope

- Confirm artillery does not have a hidden building-id acquisition flow: `pointFire` targets world
  coordinates, while explicit entity attacks still require live visibility.
- Expose remembered enemy buildings to the recipient client as stale, non-interactive intel:
  - Never-seen buildings are omitted.
  - Seen-then-fogged buildings are sent at their remembered position/footprint.
  - Hidden destruction remains stale until the remembered footprint is scouted again.
- Avoid using remembered records to reveal target ids, hidden live hp, build progress, or hidden
  destruction.
- Keep artillery impact/damage authoritative against live entities at impact time.

## Important Design Choices

- Artillery can already aim at arbitrary world points; remembered buildings are UI/targeting
  affordances, not server-side target locks.
- If a remembered building id no longer exists, the player can still shoot the last known position.
- Visible live buildings are projected as live entities and are not duplicated as remembered
  records.
- Any event generated for the firing player should reveal only the shot marker/impact allowed by
  current artillery event rules, not hidden building state.

## Expected Touch Points

- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/lib.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `client/src/protocol.js`
- `client/src/state.js`
- `client/src/renderer/buildings.js`
- `client/src/renderer/index.js`
- The phase 1 memory module
- Focused tests in snapshot/protocol areas

## Verification

- `cd server && cargo test snapshot`
- `cd server && cargo test artillery`
- `cd server && cargo test commands`
- `cd server && cargo test building_memory`
- `node --check client/src/protocol.js`
- `node --check client/src/state.js`
- `node --check client/src/renderer/buildings.js`
- `node --check client/src/renderer/index.js`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`

## Manual Testing Focus

- Never-scouted fogged enemy buildings should not render as stale intel.
- After scouting an enemy building and losing vision, the remembered silhouette should render under
  fog so artillery can point-fire the remembered location.
- If the enemy destroys or changes that building out of sight, artillery may waste fire on stale
  intel but must not reveal the live outcome until vision/impact rules allow it.
- Current visible artillery targeting should feel unchanged.

## Handoff

Artillery flows using memory: none as entity targets. Artillery `pointFire` remains a world-point
command. Memory is exposed as non-selectable stale building silhouettes so the player has last-seen
coordinates to aim at.

Flows still requiring live visibility: explicit `Attack` target ids and live entity projection.

Player-facing patch note: previously scouted enemy buildings now leave stale fog silhouettes until
their footprint is scouted again, giving artillery and other coordinate commands a last-known
position to aim at without revealing hidden current state.
