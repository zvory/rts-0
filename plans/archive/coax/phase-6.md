# Phase 6 - Declarative Target Priority Policies

## Phase Status

Status: done.

## Objective

Move current target ranking into named priority policies that preserve existing behavior and add a
machine-gun-like policy for the future coax. The new policy surface should separate target facts,
weapon profile, priority ranking, and activation constraints.

## Scope

- Introduce named priority policy identifiers or an equivalent declarative policy shape.
- Migrate current default attack ranking into policies without changing behavior:
  - ordinary small-arms/default soft-target preference,
  - anti-armor weapon preference,
  - Tank cannon immediate-threat behavior,
  - vehicle Tank Trap route-obstruction behavior,
  - moving-fire target retention,
  - deterministic distance/id tie-breaks.
- Make `priority.rs` choose targets from target facts and a policy context instead of directly
  mixing all ranking rules with attacker entity kind branches.
- Add a machine-gun-like policy for future `tank_coax`. It should rank infantry-priority targets
  ahead of fallback legal targets, use distance/id ties, and avoid Tank cannon anti-armor threat
  ordering.
- Keep the machine-gun-like policy unused or test-only until Phase 7 wires the coax runtime.
- Add an activation-constraint query shape for secondary weapons that can filter by current turret
  arc, weapon range, and intended-target direct-fire legality without chasing, rotating, or altering
  movement.
- Preserve existing ordered attack behavior and current fallback acquisition semantics.

## Out Of Scope

- No `tank_coax` live firing.
- No cooldown or protocol changes.
- No new player command/toggle.
- No balance changes to existing units.

## Expected Touch Points

- `server/crates/rules/src/combat.rs`
- `server/crates/sim/src/game/services/combat/priority.rs`
- `server/crates/sim/src/game/services/combat/acquisition.rs`
- `server/crates/sim/src/game/services/combat/weapons.rs`
- `server/crates/sim/src/game/services/combat/tests/target_priority.rs`
- `server/crates/sim/src/game/services/combat/tests/retention.rs`
- `server/crates/sim/src/game/services/combat/tests/tank_traps.rs`
- `docs/design/server-sim.md`
- `docs/design/balance.md` if priority policy contracts are documented

## Edge Cases To Cover

- Tank cannon still prioritizes in-range Anti-Tank Guns and other anti-armor threats as before.
- Tank cannon route-obstructing Tank Trap behavior remains unchanged.
- Scout Car and Tank moving-fire retention behavior remains unchanged.
- Anti-Tank Gun still prefers armored/anti-armor targets as before.
- Unit attackers still prefer units over buildings in current fallback situations.
- Machine-gun-like policy prioritizes Worker, Rifleman, and Machine Gunner over fallback vehicles
  and buildings.
- Machine-gun-like policy does not treat Mortar Team, Artillery, Anti-Tank Gun, Ekat, or Golem as
  infantry-priority.
- Machine-gun-like policy falls back to legal vehicles/buildings when no infantry-priority target
  is available.
- Machine-gun-like policy does not choose an infantry-priority intended target when the secondary
  weapon activation filter says the shot would hit an intervening enemy hard blocker first.
- Ties inside the same policy bucket use distance first, then id.

## Verification

- Existing pure `combat::priority::tests`.
- Existing `target_priority.rs`, `retention.rs`, and `tank_traps.rs` tests.
- New pure machine-gun-like policy tests for infantry-priority, exclusions, fallback, and tie
  ordering.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `node scripts/check-docs-health.mjs` if docs are changed.
- `git diff --check`

## Manual Test Focus

No manual gameplay test is required if behavior-preserving priority tests are strong. If a smoke
test is performed, confirm current Tank, Scout Car, Anti-Tank Gun, and Machine Gunner target choice
still feels unchanged in a mixed-target local scenario.

## Handoff Expectations

Name the priority policy ids, the machine-gun-like policy entry point, and the secondary-weapon
activation filter shape Phase 7 should use. Confirm that all current attackers still use their
default policies and that no live coax runtime exists yet.
