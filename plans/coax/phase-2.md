# Phase 2 - Weapon-Aware Damage And Overpenetration

## Phase Status

Status: done.

## Objective

Make direct-fire damage and overpenetration depend on the weapon profile that fired instead of
deriving AP or small-arms behavior only from the firing entity kind. This phase still passes every
current attacker its default weapon profile, so current gameplay remains behavior-preserving.

## Scope

- Thread weapon profile identity or a borrowed weapon profile through the direct-fire
  `apply_damage` path.
- Update effective damage helpers so AP versus small-arms armor reduction is computed from the
  firing weapon profile.
- Preserve attacker-kind rules that are truly about the firing entity rather than the weapon.
  Current Tank cannon armor-facing modifiers may remain tied to `tank_cannon`, not to Tank entity
  kind in general.
- Thread weapon profile identity through direct miss policy, including Anti-Tank Gun miss behavior
  and entrenchment direct-miss composition.
- Make overpenetration use the firing weapon profile for damage class, facing policy, and
  overpenetration depth.
- Preserve attribution to the firing owner for last-damage owner, scoring, under-attack notices,
  firing reveal, and AI damage-memory behavior.
- Keep every current shot behavior equivalent: Tank cannon remains AP and overpenetrates as before;
  Rifleman, Worker, Machine Gunner, and Scout Car remain small-arms; Anti-Tank Gun miss and damage
  behavior remain unchanged.
- Add tests that would fail if Tank default cannon stopped being AP or if a small-arms default shot
  started using AP behavior.

## Out Of Scope

- No `tank_coax` runtime profile or firing.
- No independent cooldowns.
- No attack-event weapon hints.
- No client feedback changes.
- No target-priority changes.

## Expected Touch Points

- `server/crates/rules/src/combat.rs`
- `server/crates/sim/src/game/entrenchment_combat.rs`
- `server/crates/sim/src/game/services/combat/damage.rs`
- `server/crates/sim/src/game/services/combat/mod.rs`
- `server/crates/sim/src/game/services/combat/projection.rs`
- `server/crates/sim/src/game/services/combat/tests*.rs`
- `docs/design/server-sim.md`
- `docs/design/balance.md` if the damage contract text changes

## Edge Cases To Cover

- Tank cannon still deals full AP damage to armored and hard targets.
- Rifleman, Worker, Machine Gunner, and Scout Car small-arms damage still reduce against armored
  targets exactly as before.
- Anti-Tank Gun miss behavior against infantry-sized targets remains unchanged.
- Entrenchment direct miss/reduction behavior remains unchanged.
- Tank victim facing modifiers apply only to the current AP cannon/anti-tank behaviors that use
  them today.
- Overpenetration secondary damage uses the same effective damage policy as the primary shot.
- Anti-Tank Gun overpenetration depth remains larger than other direct shots.
- Damage attribution still updates `last_damage_owner`, under-attack notices, scores, and firing
  reveals exactly as before when damage is actually applied.
- Shots at resource nodes still do not apply direct-fire damage.

## Verification

- Focused Rust combat tests for AP/small-arms parity, Tank facing damage, overpenetration, miss
  policy, entrenchment, and attribution.
- `cargo test --manifest-path server/Cargo.toml -p rts-sim overpenetration`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim tank_front_and_rear_hits_take_different_damage`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim firing_reveal`
- Existing combat tests that cover direct shots, Tank Traps, entrenchment, and moving fire where
  practical.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if
  combat module boundaries move.
- `node scripts/check-docs-health.mjs` if docs are changed.
- `git diff --check`

## Manual Test Focus

No required manual gameplay test if focused Rust parity is strong. If a smoke test is performed,
verify a Tank still damages targets as before, and a Machine Gunner still chips armored targets
rather than dealing AP damage.

## Handoff Expectations

Explain which helpers now take weapon identity or weapon profiles and which helpers still use
attacker kind for attacker-specific policy. Call out any remaining places where entity kind still
implies weapon behavior so later phases can avoid re-coupling.
