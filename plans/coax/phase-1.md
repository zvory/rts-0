# Phase 1 - Weapon Profile Foundation

## Phase Status

Status: done.

## Objective

Introduce a rules-owned weapon profile model without changing gameplay. Current unit and building
default attacks should still expose the same range, damage, cooldown, and weapon class they expose
today, but the rules layer should be able to name the weapon profile that produced those values.

## Scope

- Add a stable weapon identity type, for example `WeaponKind` or `WeaponProfileId`, in the rules
  crate. It must be able to name at least `rifleman_rifle`, `machine_gunner_mg`,
  `scout_car_mg`, `anti_tank_gun`, `mortar_team_mortar`, `artillery_gun`, `tank_cannon`,
  current building attacks, and later `tank_coax`.
- Add a `WeaponProfile` record carrying at least id, range, damage, cooldown, weapon class, miss
  policy hook, facing-damage policy hook, and overpenetration policy metadata.
- Map every current combat-capable entity kind to exactly one default weapon profile with values
  identical to current `attack_profile(kind)` and `weapon_class(kind)` behavior.
- Keep `attack_profile(kind)`, `weapon_class(kind)`, `is_ap(kind)`, and related public helpers
  behavior-compatible by delegating to default weapon profiles where practical.
- Add focused rules tests proving old entity-kind attack values and weapon classes are unchanged.
- Document the existence of weapon profiles in `docs/design/balance.md` or
  `docs/design/server-sim.md` only to the extent needed for the new rules surface.

## Out Of Scope

- No sim combat execution changes.
- No target acquisition or priority changes.
- No cooldown storage changes.
- No damage, overpenetration, protocol, event, client, wiki, or generated stats changes.
- Do not add `tank_coax` as a live weapon profile yet unless it is a reserved id with no entity
  mapping and no gameplay effect.

## Expected Touch Points

- `server/crates/rules/src/combat.rs`
- `server/crates/rules/src/defs.rs`
- `server/crates/rules/src/balance.rs`
- `server/crates/rules/src/balance/*.rs` only if a focused module is cleaner
- `docs/design/balance.md`
- `docs/design/server-sim.md` if the combat rules contract text changes

## Edge Cases To Cover

- Tank default weapon remains `tank_cannon`: anti-tank/AP, 5-tile moving base range, 60 damage,
  and 72-tick cooldown.
- Machine Gunner default weapon remains small-arms, 6-tile range, 4 damage, and 6-tick cooldown.
- Rifleman, Worker, Scout Car, Anti-Tank Gun, Mortar Team, Artillery, and combat-capable buildings
  keep current range, damage, cooldown, and weapon class values.
- Buildings and non-combatants still report zero/no attack exactly as before.
- Existing tests that call `attack_profile(kind)` and `weapon_class(kind)` continue to pass.
- New type names do not leak into protocol vocabulary in this phase.

## Verification

- Focused Rust tests for `rts-rules::combat` profile parity.
- `cargo test --manifest-path server/Cargo.toml -p rts-rules combat::tests`
- Existing focused rules tests that cover current default attack values.
- `node scripts/check-docs-health.mjs` if docs are changed.
- `git diff --check`

## Manual Test Focus

No manual gameplay test is required for this behavior-preserving rules extraction. If a smoke test
is performed, confirm a Tank, Machine Gunner, Rifleman, Scout Car, and Anti-Tank Gun still show the
same practical attack behavior in a local match or dev scenario.

## Handoff Expectations

Name the final weapon identity type, the default-profile lookup helper, and any legacy helpers that
still exist for compatibility. Call out whether Phase 2 can pass a weapon profile directly into
damage helpers or needs one additional adapter.
