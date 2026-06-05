# Phase 1: Minimal Pathfinding Viewer

Status: done

## Objective

Build the smallest game-backed viewer that makes the scout-car snaking corridor bug visible in the
normal Pixi client.

This phase should bias hard toward shipping a useful debugging loop. It is acceptable for scenario
selection and setup to be narrow and hardcoded.

## Shape

Add a local-only route:

```text
/dev/scenario?id=scout_car_snaking_corridor&cars=1
/dev/scenario?id=scout_car_snaking_corridor&cars=4
```

The route redirects to the existing client with a dev-watch query, similar to `/dev/selfplay`.
The client auto-joins a reserved scenario room, for example:

```text
__dev_scenario__:scout_car_snaking_corridor:cars=4
```

The room task starts the scenario immediately for any viewer. Viewers are spectators and receive
no-fog snapshots.

## Reuse Existing Plumbing

Reuse the current dev self-play machinery rather than introducing a new abstraction layer:

- `RoomMode` already separates normal rooms from dev-watch rooms;
- the dev path already owns a `Game`, enqueues scripted commands, ticks the game, and broadcasts
  `snapshot_full_for`;
- the client already has a dev-watch bootstrap and banner;
- the client already hides normal command/give-up UI for spectators;
- replay speed controls already exist and can be shown for scenario rooms too.

## Server Work

- Add `RoomMode::DevScenario(DevScenarioConfig)` or equivalent minimal mode.
- Parse only the supported scenario id and `cars=1|4` variants; reject anything else with a visible
  server error.
- Add one dev driver variant for the scout-car corridor.
- Build a `Game` with the corridor map, one player, scout cars, no normal economy, and no win
  condition.
- On the first scenario tick, enqueue one group move order toward the existing corridor goal.
- Keep the scenario running after any timing threshold so stalls can be inspected.
- If the scenario panics on the tick path, use the existing crash replay/error behavior where
  practical instead of killing the room task.

## Client Work

- Generalize `devWatchConfig()` enough to understand `/dev/scenario`.
- Show a banner like `local dev scenario no fog scout_car_snaking_corridor cars=4`.
- Auto-join the reserved scenario room as spectator.
- Show speed controls for scenario rooms; reset can be deferred if wiring it cleanly is more than a
  small reuse of existing seek behavior.

## Fixture

Reuse the existing fixture shape from `server/src/game/services/movement/tests.rs`:

- flat map;
- blocked stone band;
- carved snaking three-tile-wide corridor;
- scout cars spawned below the exit, facing north;
- one scripted move order toward the existing goal;
- no combat, economy, or win condition.

The ignored timing test may keep its private copy during the first implementation if extraction
would slow delivery. Sharing fixture constants/builders can happen in Phase 2.

## Done

- A developer can run the local server and open the scenario in the normal client.
- `cars=1` and `cars=4` both show scout cars attempting the snaking corridor route.
- The view uses real server snapshots and real client rendering, not a standalone visualizer.
- No generic scenario registry, artifact recording, or overlay protocol has been added.

## Verification

- `cd server && cargo test`
- Manually run the server and open the scenario with macOS `open`, following the existing dev
  self-play inspection convention.
- Run Node integration tests only if room handling or WebSocket behavior changes beyond the dev
  scenario path.
