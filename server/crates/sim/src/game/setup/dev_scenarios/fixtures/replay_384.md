# Replay 384: Alex's Tank 245 at the forest edge

Source: beta match 384, Soupman vs Alex, Wald des Todes, started
2026-09-24 00:31:53 UTC. Recorded and reproduced on `9fbfb3c60fc2`, seed
1583372308, duration 25421 ticks. Alex is player 2. Tank 245 has 292/292 HP and
two kills. At tick 9776 it has an active Move order but no movement.

The tank reaches (2026.033203125, 1292.9278564453125) at tick 9760 and remains
there through tick 10200, including all 26 follow-up move/formation orders from
9812 through 10022. This is a reproduction of the failure, not a movement fix.

## Extraction and reduction

The source artifact was read from `match_replays` for match 384 and replayed
using `ReplayArtifactV1::restore_start_game`; each logged command was enqueued
before its applied tick. The source JSON and database credentials are not
committed. Fixtures contain only this unit and its relevant orders/positions:

- `replay_384_tick_9707_tank.json`: complete serialized Tank 245 at tick 9707,
  before the formation order at 9708.
- `replay_384_tank_orders.json`: 30 orders from ticks 9708–10022. Original
  single-unit Move and FormationMove commands are unchanged. Seven group orders
  are replaced with the recorded destination actually assigned to Tank 245;
  `sourceCommand` retains the original group input. This removes Tank 258
  without changing Tank 245's assigned destination. The scenario driver enqueues
  at source tick minus one because its scheduler runs before `Game::tick()`.
- `replay_384_tank_positions.json`: authoritative position and facing for every
  source tick 9707–10200, independently extracted from the complete match.

The scene retains the original terrain/road and forest metadata in tiles
x=55–75, y=35–58, and nearby original tree doodads. Outside that patch the map is
empty grass. All other entities, resources and base sites are removed. Roads
must remain: their movement bonus is required to reproduce the approach.

A seven-second idle inspection lead-in precedes the original tick 9707 state;
recorded order ticks retain their original numbers. No position is teleported,
no immobility flag is set, and movement/collision logic is unchanged. The focused
regression compares each reproduced position to the full replay and checks that
HP and kill count remain unchanged.

## Review

Open `/dev/scenarios?id=replay_384_tank_forest_lock&unit=tank&count=1`, or use
Interact's `dev-scenario open` with that id, unit and count. Select entity 245
and frame its approach plus the forest edge. The normal diagnostic paths show
how accepted orders change while the tank stays still.

Run the focused test with:

```sh
cargo test --release --manifest-path server/Cargo.toml -p rts-sim replay_384
```

The scenario covers the first escape-command burst. The later HoldPosition at
10113 and orders after tick 20683 are outside its command schedule; the final
move remains active to make the lock observable indefinitely.
