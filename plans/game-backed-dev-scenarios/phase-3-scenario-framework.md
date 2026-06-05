# Phase 3: Scenario Framework

## Objective

Generalize only after the first scenario works and at least one more scenario needs the same
machinery. This phase converts hardcoded Phase 1 plumbing into a small reusable framework.

## Server Architecture

Introduce a small scenario module, likely under:

```text
server/src/game/scenario/
```

Candidate traits and structs:

```rust
pub(crate) trait DevScenario {
    fn id(&self) -> &'static str;
    fn start_payload(&self) -> StartPayload;
    fn tick(&mut self) -> ScenarioTick;
    fn reset(&mut self);
}

pub(crate) struct ScenarioTick {
    pub snapshot: Snapshot,
    pub done: bool,
}
```

Most scenarios can be implemented with a game-backed helper:

```rust
pub(crate) struct GameBackedScenario {
    game: Game,
    viewer_player_id: u32,
    script: Box<dyn ScenarioScript>,
}

pub(crate) trait ScenarioScript {
    fn enqueue_for_tick(&mut self, game: &mut Game);
    fn is_done(&self, game: &Game) -> bool;
}
```

`GameBackedScenario::tick` should:

1. enqueue scripted commands for the next tick;
2. call `game.tick()`;
3. build `game.snapshot_full_for(viewer_player_id)`;
4. merge this tick's visible events into the snapshot;
5. return the snapshot to the room task.

## Room Task Behavior

The generalized scenario room task should:

- start the requested scenario on first viewer join;
- send a spectator `StartPayload`;
- advance the scenario and send snapshots to all viewers each tick;
- support speed changes and reset;
- reject ordinary gameplay commands in scenario rooms;
- keep the room alive and restartable while viewers are connected.

Scenario room names should be parsed from safe ids and safe variant values only. Invalid ids should
produce a visible server error rather than falling back to a normal lobby room.

## Candidate Second Scenarios

Use one simple second scenario to prove the abstraction handles more than pathfinding:

- `tank_vs_at_gun`: verify setup, range, facing, projectile/audio feedback, and death events;
- `riflemen_focus_fire`: verify target acquisition and concentrated damage;
- `scout_car_kiting_infantry`: verify firing while moving and target tracers;
- `factory_chokepoint_traffic`: combine production spawn exits, traffic, and combat pressure;
- `under_attack_notice`: verify notices, minimap pings, and spatial alert audio.

Combat scenarios should use normal `Game::tick` so event delivery and snapshot projection match real
gameplay.

## Documentation

After the framework is usable, document the workflow in `docs/context/testing.md` so future agents
can open scenarios without rediscovering route names and server requirements.

## Done

- Scenario dispatch is no longer hardcoded to scout-car corridor.
- At least two scenarios share the framework.
- Reset and speed behavior are consistent across scenario rooms.
- Normal rooms and protocol semantics are unchanged.

## Verification

- `cd server && cargo test`
- `node tests/server_integration.mjs`
- `node tests/regression.mjs`
- `node tests/ai_integration.mjs`
- `cd tests && npm install && node client_smoke.mjs` when client dev-watch UI changes
- Manually open each registered scenario with a local server.
