# Phase 2 - Weapon-Aware Damage Refactor

## Phase Status

Status: pending.

## Objective

Make direct-fire damage calculation depend on the weapon profile that fired instead of deriving AP
or small-arms behavior only from the firing entity kind. This phase still passes every current
attacker its default weapon profile, so current gameplay remains behavior-preserving.

## Scope

- Thread weapon profile identity or weapon class through the direct-fire `apply_damage` path.
- Update effective damage helpers so AP versus small-arms armor reduction can be computed from the
  firing weapon profile.
- Preserve existing attacker-kind rules that truly are about the attacker, not the weapon, such as
  Tank victim facing modifiers if current cannon behavior depends on them.
- Make overpenetration damage use the same firing weapon profile as the primary shot.
- Preserve attribution to the firing owner for last-damage owner, scoring, under-attack notices,
  firing reveal, and AI damage-memory behavior.
- Keep every current shot behavior equivalent: Tank cannon remains AP and overpenetrates as before;
  Machine Gunner and Scout Car remain small-arms; Anti-Tank Gun miss and damage behavior remain
  unchanged.
- Add tests that would fail if Tank default cannon stopped being AP or if a small-arms default shot
  started using AP behavior.
- Do not add the coax profile, independent cooldowns, event weapon hints, client feedback changes,
  or target-priority changes in this phase.

## Expected Touch Points

- `server/crates/rules/src/combat.rs`
- `server/crates/sim/src/game/services/combat/damage.rs`
- `server/crates/sim/src/game/services/combat/mod.rs`
- `server/crates/sim/src/game/services/combat/projection.rs`
- `server/crates/sim/src/game/services/combat/tests*.rs`
- `docs/design/server-sim.md`
- `docs/design/balance.md` if the damage contract text changes

## Edge Cases To Cover

- Tank cannon still deals full AP damage to armored and hard targets.
- Rifleman, Machine Gunner, Scout Car, and Worker small-arms damage still reduce against armored
  targets exactly as before.
- Overpenetration secondary damage uses the same effective damage policy as the primary shot.
- Entrenchment miss/reduction behavior, Anti-Tank Gun miss behavior, and Tank victim facing
  modifiers do not drift.
- Damage attribution still updates `last_damage_owner`, under-attack notices, and scores exactly as
  before when damage is actually applied.
- Shots at resource nodes still do not apply direct-fire damage.

## Verification

- Focused Rust combat tests for AP/small-arms parity, Tank facing damage, overpenetration, and
  attribution.
- Existing combat tests that cover direct shots, overpenetration, Tank Traps, entrenchment, and
  moving fire where practical.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if
  combat module boundaries move.
- `node scripts/check-docs-health.mjs` if docs are changed.
- `git diff --check`.

## Manual Test Focus

No required manual gameplay test if focused Rust parity is strong. If a smoke test is performed,
verify a Tank still kills and damages the same targets as before, and a Machine Gunner still chips
armored targets rather than dealing AP damage.

## Handoff Expectations

Explain which helpers now take weapon identity or weapon class and which helpers still use attacker
kind for attacker-specific policy. Call out any remaining places where entity kind still implies
weapon behavior so Phase 3 or Phase 4 can avoid re-coupling.
