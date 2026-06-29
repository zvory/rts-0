# Phase 3 - Meth Riflemen Use Shared Moving Fire

## Phase Status

Status: done.

## Objective

Make Methamphetamines grant riflemen the shared moving-fire capability directly. Upgraded riflemen
should move and shoot on `Move` and `AttackMove` without inheriting vehicle-specific handling.

## Scope

- Derive rifleman moving-fire capability from the owning player's Methamphetamines upgrade through a
  clear policy path.
- Make upgraded riflemen keep moving toward their command destination while firing at valid in-range
  targets.
- Keep unupgraded riflemen on their current stop-to-fire behavior.
- Avoid routing meth riflemen through vehicle/turret/standoff policy.
- Update docs so Methamphetamines is described as permanent moving fire, not temporary Charge.

## Expected Touch Points

- `server/crates/sim/src/game/upgrade.rs`
- `server/crates/sim/src/game/services/combat/`
- `server/crates/sim/src/game/services/movement/`
- focused meth, combat, and movement tests
- `docs/design/balance.md`
- `docs/design/server-sim.md`
- `docs/design/hardening.md`

## Verification

- Focused Rust tests showing meth riflemen fire while keeping `Move` and `AttackMove` paths.
- Focused Rust tests showing meth riflemen do not use vehicle chase, standoff, or turret behavior.
- Focused Rust tests showing unupgraded riflemen still stop to fire.
- `git diff --check`.

## Manual Test Focus

Research Methamphetamines, issue both move and attack-move orders through enemy infantry and armor,
and confirm upgraded riflemen keep advancing while firing only at legal in-range targets.

## Handoff Expectations

Describe how meth moving-fire capability is computed, how it differs from vehicle policy, and what
manual comparisons against unupgraded riflemen still need attention.
