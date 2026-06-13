# Phase 4 - Ability Registry and Effect Hooks

Status: Designed, not implemented.

## Objective

Prepare the engine for an ability-heavy faction by turning ability discovery and projection into a
registry-backed contract. Existing abilities must retain their behavior, but adding a new ability
should not require editing unrelated command-card, protocol, cooldown, and carrier logic in many
places.

## Scope

- Add a faction-aware ability registry with stable ids, labels, carriers, target mode, range,
  cooldown, charges, resource cost, queue behavior, tech requirement, and autocast support where
  applicable.
- Keep custom effect implementations for complex abilities, but route command validation,
  cooldown projection, remaining uses, and command-card discovery through the registry.
- Preserve existing behavior for:
  - Smoke
  - Mortar Fire
  - Artillery Point Fire
  - Breakthrough
  - legacy Charge no-op compatibility
- Add reusable effect hooks for common ability classes:
  - self buff
  - targeted world effect
  - delayed projectile/impact
  - area effect
  - toggle/autocast
  - limited charges
- Add tests for invalid ability ids, wrong-faction carriers, cooldowns, charges, and fog-safe
  events.

## Expected Touch Points

- `server/crates/sim/src/game/ability.rs`
- `server/crates/sim/src/game/services/ability_orders.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/combat/`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/protocol/src/lib.rs`
- `client/src/protocol.js`
- `client/src/config.js`
- `client/src/hud_command_card.js`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/balance.md`

## Verification

- Rust ability registry tests for all existing abilities.
- Rust command tests for carrier eligibility, costs, cooldowns, queueing, and invalid ids.
- Fog/security tests for ability events and reveal data.
- Client descriptor tests proving existing ability buttons and cooldown clocks are unchanged.
- Protocol parity tests for ability ids and compact ability projection.

## Manual Testing Focus

In debug mode, verify Scout Car Smoke, Mortar Fire, Artillery Point Fire, and Command Car
Breakthrough still appear on the correct units and execute as before.

## Handoff Expectations

The handoff must list the ability registry fields, remaining one-off effect code, and the recipe
for adding a new ability in Phase 6. It should also call out any ability UI limitations deferred to
Phase 5.

## Player-Facing Outcome

No intended current-faction balance change. The ability system becomes ready for a faction whose
core gameplay is active abilities instead of only weapon stats and production unlocks.

