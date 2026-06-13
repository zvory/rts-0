# Phase 4 - Faction Starting Loadouts

Status: Designed, not implemented.

## Objective

Move starting entities, starting resources, supply rules, opening upgrades, and resource-node usage
into faction loadout definitions. Prove that a fixture faction can start with a different economy
and no steel/oil mining dependency without implementing the real second faction roster.

## Scope

- Define faction loadout data in the Rust-authoritative faction catalog.
- Move current-faction City Centre, Worker ring, starting resources, and starting supply into the
  current faction loadout.
- Allow loadouts to define starting units, buildings, completed construction state, initial
  resources, initial capacities, opening upgrades/flags, and debug-mode additions.
- Make map resource spawning loadout-aware enough that factions can ignore universal steel/oil
  resources without breaking current-faction opponents.
- Avoid faction-specific map resource objects unless an approved faction brief explicitly requires
  them.
- Add a fixture faction or test-only fixture loadout with a different resource set and no steel/oil
  mining requirement.
- Enforce illegal cross-faction economy/build/train/research commands with server-side notices or
  no-ops.
- Keep match history and score calculations sensible for fixture entities and zero/alternate-cost
  definitions.
- Keep AI slots current-faction-only; fixture factions are human/dev/test only.

## Expected Touch Points

- `server/crates/rules/src/`
- `server/crates/sim/src/game/setup.rs`
- `server/crates/sim/src/game/setup/dev_scenarios/`
- `server/crates/sim/src/game/player_state.rs`
- `server/crates/sim/src/game/services/economy.rs`
- `server/crates/sim/src/game/services/construction.rs`
- `server/crates/sim/src/game/services/production.rs`
- `server/crates/sim/src/game/services/supply.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/lib.rs`
- `server/src/lobby/`
- `client/src/state.js`
- `client/src/hud.js`
- `client/src/config.js`
- `docs/design/protocol.md`
- `docs/design/balance.md`
- `docs/design/server-sim.md`

## Verification

- Rust tests for default current-faction start parity: starting entities, resources, supply,
  upgrades, fog, and resource nodes.
- Rust tests for fixture-faction starts with alternate resources and no steel/oil mining
  dependency.
- Server integration test for mixed current-faction plus fixture-faction match start.
- Command tests proving fixture players cannot issue current-faction-only economy/build/train
  commands.
- Fog tests proving resource node visibility and fixture economy data do not leak hidden enemy
  state.
- Client HUD tests for fixture resource display if the fixture is exposed through a dev path.

## Manual Testing Focus

Verify current-faction gathering and spending still work. If a fixture faction is exposed in a dev
path, verify it starts with its fixture loadout, shows the right resources, does not need steel/oil
mining, and cannot issue current-faction commands.

## Handoff Expectations

The handoff must describe the loadout schema, current-faction parity evidence, fixture faction
limitations, any remaining resource-node assumptions, and the mixed-faction test command/result.
It should tell Phase 5 how abilities attach to faction catalog definitions.

## Player-Facing Outcome

The current faction should play unchanged. Internally, starting state is faction-defined and test
fixtures prove alternate economy starts are possible.
