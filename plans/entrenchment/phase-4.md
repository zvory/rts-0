# Phase 4 - Entrenched Combat Benefits

## Phase Status

Status: not started.

## Objective

Apply all player-facing combat bonuses for eligible infantry that are stationary in an active
trench. This phase should use the occupation state from Phase 3 as the single source of truth for
whether benefits apply.

## Scope

- Increase weapon range by 1 tile only while an eligible infantry unit is stationary and occupying
  an active trench.
- Apply a 70% direct-shot miss chance against entrenched units. Combine this with existing miss
  policy deliberately and document whether the highest miss chance or a composed probability is
  used.
- Reduce area-of-effect damage against entrenched units by 70% for the general AoE damage path,
  including current Mortar and Artillery damage.
- Ensure future AoE weapons can reuse the same entrenched damage-reduction helper unless their
  requirements explicitly override it.
- Prevent projectiles from over-penetrating through an entrenched primary victim.
- Prevent entrenched units from taking secondary over-penetration damage.
- Do not apply benefits to units still digging in, units merely moving through a trench, units
  displaced out of occupation, or excluded unit kinds.
- Preserve other researched upgrade effects unless the requirements say otherwise.
- Preserve Methamphetamines interactions: Riflemen keep faster attack cadence while entrenched, but
  moving-fire and movement-speed effects must not make a moving Rifleman entrenched; Machine
  Gunners keep faster setup/teardown and speed rules, but only stationary occupation grants
  entrenchment benefits.
- Keep direct attack, attack-move, target acquisition, fog, and projectile events consistent with
  existing combat authority.
- Update `docs/design/balance.md` and `docs/design/server-sim.md` for entrenched combat behavior.

## Expected Touch Points

- `server/crates/sim/src/game/services/combat/weapons.rs`
- `server/crates/sim/src/game/services/combat/damage.rs`
- `server/crates/sim/src/game/services/combat/acquisition.rs`
- `server/crates/sim/src/game/mortar.rs`
- `server/crates/sim/src/game/artillery.rs`
- `server/crates/rules/src/combat.rs`
- `server/crates/rules/src/balance/`
- `server/crates/sim/src/rules/projection.rs`
- `server/crates/sim/src/game/services/combat/tests/`
- `server/crates/sim/src/game/tests/smoke_mortar_tests.rs`
- `server/crates/sim/src/game/tests/artillery_tests.rs`
- `docs/design/balance.md`
- `docs/design/server-sim.md`

## Verification

- Focused Rust combat tests proving entrenched range is base range plus 1 tile and non-entrenched
  range is unchanged.
- Deterministic or seeded tests proving direct shots against entrenched infantry miss according to
  the new 70% policy without making invulnerable units or buildings affected.
- Mortar and Artillery tests proving AoE damage is reduced by 70% for entrenched infantry and not
  reduced for non-entrenched or excluded units.
- Over-penetration tests proving entrenched primary victims stop secondary damage and entrenched
  secondary candidates are skipped.
- Methamphetamines regression tests for Riflemen and Machine Gunners.
- Fog/projection tests if changed target ids or events could reveal trench occupation.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if combat
  helpers move across service boundaries.
- `git diff --check`.

## Manual Test Focus

Pit entrenched Riflemen and Machine Gunners against direct-fire attackers, Mortars, Artillery, and
over-penetrating shots. Confirm entrenched units are tougher while stationary in trenches and lose
the benefits as soon as they move out.

## Handoff Expectations

Name the combat helper that decides whether benefits apply, the miss-probability composition rule,
and the shared AoE reduction helper. Provide factual patch-note bullets for changed combat behavior
and any cases that should be watched in playtests.
