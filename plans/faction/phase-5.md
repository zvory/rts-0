# Phase 5 - Faction Starting Loadouts

Status: Designed, not implemented.

## Objective

Move starting entities, starting Steel/Oil/Supply values, supply rules, opening upgrades, and
resource-node usage into faction loadout definitions. Prove that a fixture faction can start with a
different loadout and command set inside the existing economy without implementing the real second
faction roster.

## Scope

- Define faction loadout data in the Rust-authoritative faction catalog.
- Move current-faction City Centre, Worker ring, starting resources, and starting supply into the
  current faction loadout.
- Replace or explicitly retire any temporary global `starting_steel`, `starting_oil`, or
  `starting_loadout` replay/start schema shims left from Phase 1. Replay and branch reconstruction
  must derive per-player starts from recorded faction/loadout data, not a single global resource
  pair.
- Allow loadouts to define starting units, buildings, completed construction state, initial Steel,
  initial Oil, initial supply capacity, opening upgrades/flags, and debug-mode additions.
- Keep map resource spawning on the current universal Steel/Oil nodes.
- Do not add faction-specific map resource objects in this plan.
- Add a fixture faction or test-only fixture loadout with different starting entities, resources,
  supply, and legal commands while still using Steel/Oil/Supply.
- Enforce illegal cross-faction economy/build/train/research commands with server-side notices or
  no-ops.
- Keep match history and score calculations sensible for fixture entities and zero/alternate
  Steel/Oil-cost definitions.
- Keep AI slots current-faction-only; fixture factions are human/dev/test only.
- Update the lifecycle matrix with the fixture/dev start path, replay reconstruction behavior, and
  any remaining intentionally current-faction-only paths.

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
- Rust tests for fixture-faction starts with alternate Steel/Oil/Supply values and no illegal
  current-faction command access.
- Replay/branch tests proving faction loadout and starting resources survive reconstruction from
  the new schema.
- Server integration test for mixed current-faction plus fixture-faction match start.
- Command tests proving fixture players cannot issue current-faction-only economy/build/train
  commands.
- Fog tests proving resource node visibility and fixture start data do not leak hidden enemy state.
- Client HUD tests proving fixture starts still display the shared Steel/Oil/Supply resources if the
  fixture is exposed through a dev path.

## Manual Testing Focus

Verify current-faction gathering and spending still work. If a fixture faction is exposed in a dev
path, verify it starts with its fixture loadout, shows Steel/Oil/Supply correctly, and cannot issue
current-faction commands.

## Handoff Expectations

The handoff must describe the loadout schema, current-faction parity evidence, fixture faction
limitations, the replay/branch schema replacement, the explicit Steel/Oil map-resource assumption,
and the mixed-faction test command/result.
It should tell Phase 6 how abilities attach to faction catalog definitions.

## Player-Facing Outcome

The current faction should play unchanged. Internally, starting state is faction-defined and test
fixtures prove alternate faction starts are possible without changing the global economy.
