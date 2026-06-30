# Phase 1 - Rules Weapon Profile Skeleton

## Phase Status

Status: pending.

## Objective

Introduce a rules-owned weapon profile model without changing gameplay. Current unit and building
default attacks should still expose the same range, damage, cooldown, and weapon class they expose
today, but the rules layer should be able to name the profile that produced those values.

## Scope

- Add a small weapon identity vocabulary, for example `WeaponKind`, `WeaponProfileId`, or an
  equivalent rules-owned type. Prefer a name that can distinguish `tank_cannon` and `tank_coax`
  later without implying every profile is an entity kind.
- Add a `WeaponProfile` record that carries at least range, damage, cooldown, weapon class, and any
  flags needed by later phases to express direct-fire overpenetration.
- Make every current combat-capable entity map to exactly one default weapon profile with values
  identical to the current `attack_profile(kind)` and `weapon_class(kind)` behavior.
- Keep existing public helpers such as `attack_profile(kind)` and `weapon_class(kind)` working by
  delegating to the default profile if that keeps the phase smaller and safer.
- Add focused rules tests that prove the old entity-kind profile values and weapon classes did not
  change.
- Document in `docs/design/balance.md` or `docs/design/server-sim.md` that weapon profiles exist
  but only default profiles are live after this phase.
- Do not change combat execution, target priority, cooldown storage, damage calculation,
  overpenetration, event shape, client code, or generated stats in this phase unless required by the
  smallest clean rules extraction.

## Expected Touch Points

- `server/crates/rules/src/combat.rs`
- `server/crates/rules/src/defs.rs`
- `server/crates/rules/src/balance.rs`
- `server/crates/rules/src/balance/*.rs` if a new focused module is cleaner
- `docs/design/balance.md`
- `docs/design/server-sim.md` if the combat rules contract text changes

## Edge Cases To Cover

- Tank default weapon remains anti-tank/AP with 5-tile moving base range, 60 damage, and 72-tick
  cooldown.
- Machine Gunner default weapon remains small-arms with 6-tile range, 4 damage, and 6-tick cooldown.
- Buildings and non-combatants still report zero/no attack exactly as before.
- Existing tests that call `attack_profile(kind)` and `weapon_class(kind)` continue to pass.
- The new type names do not require protocol vocabulary yet; this phase is rules-internal unless a
  later phase makes weapon identity visible.

## Verification

- Focused Rust tests for `rts-rules::combat` profile parity.
- Existing focused combat/rules tests that cover current default attack values.
- `cargo test --manifest-path server/Cargo.toml -p rts-rules` or the narrowest available rules test
  command if package names differ.
- `node scripts/check-docs-health.mjs` if docs are changed.
- `git diff --check`.

## Manual Test Focus

No manual gameplay test is required for this behavior-preserving rules extraction. If a smoke test
is performed, confirm a Tank, Machine Gunner, Rifleman, and Anti-Tank Gun still show the same
practical attack behavior in a local match or dev scenario.

## Handoff Expectations

Name the final weapon identity type, the default-profile lookup helper, and any legacy helpers that
still exist for compatibility. Call out whether Phase 2 can pass a weapon profile directly into
damage helpers or needs one additional adapter.
