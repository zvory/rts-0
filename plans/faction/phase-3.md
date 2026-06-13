# Phase 3 - Economy and Starting Loadouts

Status: Designed, not implemented.

## Objective

Make faction-specific starts and economy models possible while preserving the current steel/oil/
supply faction. Add a fixture faction for tests that can start without steel/oil mining, but do
not implement the real new faction's roster yet.

## Scope

- Move starting entities and starting resources into faction loadout definitions.
- Let a faction define whether it uses steel, oil, supply, current resource nodes, alternate
  resources, or no mined resources.
- Decide the minimal protocol shape for alternate resources:
  - either keep fixed `steel/oil/supply` for Phase 3 and allow the fixture to ignore them, or
  - introduce a generic resource bag if the approved architecture requires it now.
- Keep current faction snapshot fields compatible unless a deliberate protocol migration is made.
- Make resource node spawning map/faction aware enough that factions can ignore map resources
  without breaking opponents who still need them.
- Add server validation for illegal cross-faction economy commands.
- Keep score calculations and match history sensible for zero-cost or alternate-cost fixture
  entities.

## Expected Touch Points

- `server/crates/sim/src/game/setup.rs`
- `server/crates/sim/src/game/player_state.rs`
- `server/crates/sim/src/game/services/economy.rs`
- `server/crates/sim/src/game/services/construction.rs`
- `server/crates/sim/src/game/services/production.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/lib.rs`
- `client/src/state.js`
- `client/src/hud.js`
- `client/src/config.js`
- `docs/design/protocol.md`
- `docs/design/balance.md`

## Verification

- Rust tests for default current-faction start parity.
- Rust tests for fixture-faction starts with no steel/oil mining requirement.
- Server integration test for mixed current-faction plus fixture-faction match start.
- Protocol parity tests if resource or start payload shapes change.
- Client contract tests for HUD resource rendering if generic resources are introduced.
- Fog tests proving resource node visibility does not leak faction-specific hidden state.

## Manual Testing Focus

Verify current faction gathering and spending still work. If a fixture faction is exposed in a dev
path, verify it starts with its fixture loadout, does not need steel/oil mining, and cannot issue
current-faction build/train commands.

## Handoff Expectations

The handoff must state the chosen resource protocol strategy, list what remains fixed to
steel/oil/supply, and identify any UI limitations left for Phase 5. It should also include the
mixed-faction test command and result.

## Player-Facing Outcome

The current faction should play unchanged. Dev/test fixtures prove a different economy and start
model can exist without destabilizing normal matches.

