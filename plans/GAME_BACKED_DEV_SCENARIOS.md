# Game-Backed Dev Scenario Viewer Plan

## Goal

Add a dev-only scenario viewer that lets a human inspect authored simulation situations in the
normal game renderer. The first target is the ignored scout-car snaking corridor timing scenario,
but the same system should support combat, economy, production, traffic, and AI-adjacent scenarios.

This is not a plan to fix `scout_car_snaking_corridor_clear_times`. The goal is to make the
behavior visible and reproducible so fixes can be investigated against actual server behavior.

## Why Game-Backed

Scenarios should use `Game` wherever possible:

- the renderer receives the same `start` and `snapshot` messages it already understands;
- combat events, death events, target tracers, resource deltas, sounds, fog-off spectator views, and
  interpolation all work through the existing client path;
- scenario behavior stays aligned with the public simulation seam instead of growing a parallel
  visualization simulator;
- recorded command logs and snapshot artifacts can be added later without changing the viewer model.

Direct service-level runners should exist only as an escape hatch for tests that intentionally
exercise internals below the `Game` seam and cannot be honestly represented by a custom `Game`.

## High-Level Shape

Add a local-only route:

```text
/dev/scenario?id=<scenario_id>&variant=<variant>
```

The route redirects to the existing client with a dev-watch query, similar to `/dev/selfplay`.
The client auto-joins a reserved scenario room:

```text
__dev_scenario__:<scenario_id>:<variant>
```

The lobby room task recognizes scenario rooms and starts a scenario session immediately for any
viewer. Viewers are always spectators and receive no-fog snapshots.

## Server Architecture

Introduce a small scenario module, likely under:

```text
server/src/game/scenario/
```

Core traits and structs:

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

Most scenarios should be implemented with a `GameBackedScenario` helper:

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

## Scenario Setup API

Add test/dev-only construction helpers for `Game` so scenarios can author exact worlds without
turning those powers into normal gameplay APIs.

Desired capabilities:

- create a `Game` from a custom `Map`;
- choose explicit `PlayerInit` records;
- disable normal starting bases/resources when a scenario wants an empty map;
- spawn units, buildings, and resource nodes at exact world positions;
- set unit facing and initial orders;
- recompute spatial index, supply, and fog after setup;
- expose enough setup failure context to make scenario registration errors obvious at startup.

Keep these helpers `pub(crate)` and only reachable from server-side scenario/test code. They must
not be exposed through client commands or general WebSocket messages.

## First Scenario: Scout-Car Snaking Corridor

Scenario id:

```text
scout_car_snaking_corridor
```

Variants:

```text
cars=1
cars=4
```

The scenario should reuse the existing fixture shape from
`server/src/game/services/movement/tests.rs`:

- flat map;
- blocked stone band;
- carved snaking three-tile-wide corridor;
- scout cars spawned below the exit, facing north;
- one scripted move order toward the existing goal;
- no combat, economy, or win condition.

The existing ignored test can continue to print timing results. Once the scenario exists, the test
should share fixture constants/builders with the scenario instead of owning a private copy.

Useful first-view behavior:

- camera starts centered on the corridor entrance;
- no fog;
- speed controls are available;
- reset restarts the scenario from tick 0;
- the scenario keeps running after timeout so the human can inspect long stalls.

Optional later overlays:

- goal point;
- exit-clear threshold line;
- recent trail per scout car;
- next waypoint;
- current path goal;
- stuck/no-progress counters;
- static-blocked/repath/reverse-recovery counters.

Keep overlays dev-only. Do not add them to normal gameplay snapshots unless the client explicitly
enters a dev scenario/debug mode.

## Combat Scenario Examples

Game-backed scenarios should make combat visualization straightforward. Initial candidates:

- `tank_vs_at_gun`: verify setup, range, facing, projectile/audio feedback, and death events;
- `riflemen_focus_fire`: verify target acquisition and concentrated damage;
- `scout_car_kiting_infantry`: verify firing while moving and target tracers;
- `factory_chokepoint_traffic`: combine production spawn exits, traffic, and combat pressure;
- `under_attack_notice`: verify notices, minimap pings, and spatial alert audio.

Combat scenarios should use normal `Game::tick` so event delivery and snapshot projection match real
gameplay.

## Client Changes

Generalize the current self-play dev-watch bootstrap into a scenario-aware dev watch config.

Suggested client behavior:

- `/dev/scenario?...` shows a banner like `local dev scenario no fog <id>`;
- the client auto-joins the reserved scenario room as spectator;
- replay speed controls appear for scenario rooms, not only self-play replay rooms;
- reset sends an existing or new dev-only control message to rebuild the scenario;
- normal command card and give-up UI stay hidden for spectators.

Avoid building a separate frontend. The value comes from using the real Pixi renderer and client
state model.

## Room Task Changes

Add a room mode alongside normal and self-play:

```rust
RoomMode::DevScenario(DevScenarioConfig)
```

The room task should:

- start the requested scenario on first viewer join;
- send a spectator `StartPayload`;
- on each tick, advance the scenario and send its snapshot to all viewers;
- support speed changes and reset;
- reject ordinary gameplay commands in scenario rooms;
- keep the room alive and restartable while viewers are connected.

Scenario room names should be parsed from safe ids and safe variant values only. Invalid ids should
produce a visible server error rather than falling back to a normal lobby room.

## Artifacts Later

After live viewing works, add optional artifact recording:

```text
server/target/scenario-artifacts/<scenario_id>/<variant>/start.json
server/target/scenario-artifacts/<scenario_id>/<variant>/frames.json
server/target/scenario-artifacts/<scenario_id>/<variant>/summary.log
```

Artifacts are useful when a failing local run needs to be shared or replayed without rerunning the
simulation. They should record client-facing frames first; command-log artifacts are useful only for
Game-backed scenarios whose setup can also be reconstructed.

## Implementation Order

1. Add scenario routing and room-mode plumbing with one hardcoded placeholder scenario.
2. Add `Game` scenario setup helpers behind `pub(crate)` server-side APIs.
3. Port the scout-car snaking corridor fixture into a Game-backed scenario.
4. Share fixture constants/builders between the ignored timing test and the scenario.
5. Generalize client dev-watch labels and speed/reset controls for scenario rooms.
6. Add one simple combat scenario to prove the abstraction handles events and death feedback.
7. Document the workflow in `docs/context/testing.md` after the first scenario is usable.
8. Add optional frame artifact recording only after live viewing is stable.

## Verification

For the initial plan implementation, no runtime tests are required because this file is only a plan.

For the eventual feature:

- `cd server && cargo test`;
- `node tests/server_integration.mjs`;
- `node tests/regression.mjs`;
- `node tests/ai_integration.mjs`;
- `cd tests && npm install && node client_smoke.mjs` when client dev-watch UI changes;
- manually open the scenario with a local server and macOS `open`, following the existing self-play
  replay inspection convention.

## Non-Goals

- Do not fix scout-car movement as part of the viewer.
- Do not expose arbitrary map/unit spawning to clients.
- Do not create a second renderer or a standalone canvas UI for the main path.
- Do not change normal match protocol semantics for non-dev rooms.
- Do not make Bazel edits; if generated build metadata is ever needed, use the repo's Gazelle flow.
