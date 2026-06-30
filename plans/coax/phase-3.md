# Phase 3 - Weapon Cooldown State

## Phase Status

Status: pending.

## Objective

Prepare combat cooldown state for more than one weapon per entity while preserving current
single-weapon gameplay. After this phase, the simulation should be able to tick, query, delay, and
reset cooldowns by weapon identity, even though current entities still fire only their default
weapon.

## Scope

- Replace or wrap `CombatState::attack_cd` with a weapon-aware cooldown interface.
- Replace or wrap `CombatState::firing_reveal_response_target` with weapon-aware response-delay
  tracking, so one weapon's first counterfire delay cannot suppress or consume another weapon's
  delay state.
- Preserve `attack_cd()`, `set_attack_cd()`, and `tick_attack_cd()` compatibility shims where they
  are still needed for current callers, but make them explicitly operate on the default weapon.
- Add direct helpers such as `weapon_cooldown(weapon)`, `set_weapon_cooldown(weapon, ticks)`, and
  `tick_weapon_cooldowns()` or an equivalent local pattern.
- Make normal direct-fire combat use the current default weapon cooldown through the new interface.
- Make firing-reveal response delay apply to the weapon that is trying to fire, while preserving
  current default weapon timing. The delay should add to that weapon's cooldown only, not to every
  weapon on the entity.
- Keep ability-specific cooldown/lockout behavior distinct from weapon cooldowns.
- Handle missing cooldown entries as ready/default-zero rather than panicking.
- Add tests that prove all current combatants keep their previous cooldown cadence.

## Out Of Scope

- No attack-event weapon field.
- No protocol or client changes.
- No target classification or priority changes.
- No `tank_coax` firing.
- No change to artillery/mortar ability cooldown semantics beyond adapting default-weapon shims
  where required.

## Expected Touch Points

- `server/crates/sim/src/game/entity/state.rs`
- `server/crates/sim/src/game/entity/entity.rs`
- `server/crates/sim/src/game/services/combat/mod.rs`
- `server/crates/sim/src/game/services/combat/weapons.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/ability_orders.rs`
- `server/crates/sim/src/game/services/combat/tests*.rs`
- `docs/design/server-sim.md` if the combat state contract changes

## Edge Cases To Cover

- Rifleman, Worker, Machine Gunner, Scout Car, Anti-Tank Gun, Mortar Team, Artillery, Tank, and
  combat-capable buildings still fire at the same cadence as before.
- Firing-reveal response delay still delays the default weapon exactly as before.
- Mortar autocast and point-fire ability cooldown checks remain unchanged.
- Artillery firing/reload behavior remains unchanged.
- Stale entities, missing combat state, unknown weapon ids, and zero-damage profiles are safe
  no-ops.
- A Tank can later hold separate `tank_cannon` and `tank_coax` cooldown and firing-reveal response
  entries without one overwriting the other.

## Verification

- Focused Rust combat tests for default cooldown parity.
- Focused Rust combat tests for default firing-reveal response-delay parity.
- Existing tests that assert `attack_cd()` behavior for mortar, artillery, support weapons,
  entrenchment, and Tank firing.
- `cargo test --manifest-path server/Cargo.toml -p rts-sim attack_cd`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim cooldown`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if
  sim architecture boundaries move.
- `node scripts/check-docs-health.mjs` if docs are changed.
- `git diff --check`

## Manual Test Focus

No required manual gameplay test if focused cooldown tests are strong. If a smoke test is performed,
confirm Machine Gunners, Tanks, Mortars, and Artillery still fire with recognizable timing.

## Handoff Expectations

Name the cooldown and firing-reveal response-delay storage shapes, plus the exact APIs Phase 7
should use for `tank_coax`. Call out which compatibility shims remain and which callers still need
migration before they can use non-default weapons.
