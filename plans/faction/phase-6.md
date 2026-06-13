# Phase 6 - Ability Registry Parity

Status: Designed, not implemented.

## Objective

Route existing ability discovery, validation, cooldown projection, remaining uses, costs, and
command-card affordances through faction-aware registry definitions while preserving current
behavior. This phase is a parity refactor, not a new ability-effect framework.

## Scope

- Add a faction-aware ability registry with stable ids, labels, carriers, target mode, range,
  cooldown, charges, resource cost, queue behavior, tech requirement, and autocast metadata.
- Represent existing Smoke, Mortar Fire, Artillery Point Fire, Breakthrough, and legacy Charge in
  the registry.
- Keep current one-off effect implementations intact where practical.
- Route command validation, carrier eligibility, target-mode validation, resource cost checks,
  cooldown projection, remaining uses, autocast availability, and command-card discovery through
  the registry.
- Reject wrong-faction ability use on the server.
- Update generated or mechanically checked client ability descriptors so the command card mirrors
  Rust registry data.
- Keep protocol ids and compact ability projection synchronized with the registry.
- Apply the Phase 3 command-id namespace rule to ability command ids and hotkey profile catalog
  entries.

## Expected Touch Points

- `server/crates/rules/src/`
- `server/crates/sim/src/game/ability.rs`
- `server/crates/sim/src/game/services/ability_orders.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/protocol/src/lib.rs`
- `client/src/protocol.js`
- `client/src/config.js`
- `client/src/hud_command_card.js`
- `tests/hud_command_card.mjs`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/balance.md`

## Verification

- Rust ability registry tests for all existing abilities.
- Rust command tests for carrier eligibility, wrong-faction carriers, target modes, resource costs,
  cooldowns, charges, queueing, autocast gating, and invalid ids.
- Client descriptor tests proving existing ability buttons, hotkeys, labels, costs, and cooldown
  clocks are unchanged.
- Hotkey profile tests proving ability command ids remain stable and do not collide across current
  and fixture faction descriptors.
- Protocol parity tests for ability ids and compact ability projection.
- Focused debug-mode sim/client test for existing ability availability.

## Manual Testing Focus

In debug mode, verify Scout Car Smoke, Mortar Fire, Artillery Point Fire, and Command Car
Breakthrough still appear on the correct units and execute as before.

## Handoff Expectations

The handoff must list the ability registry fields, remaining one-off effect code, generated/client
mirror status, command-id namespace details, and the recipe for adding a registry-backed ability in
Phase 7 or Phase 11.

## Player-Facing Outcome

No intended current-faction balance change. Existing abilities are now cataloged and faction-aware.
