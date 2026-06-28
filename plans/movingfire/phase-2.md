# Phase 2 - Movement-Order Target Filtering

## Phase Status

Status: planned.

## Objective

Make active `Move` and `AttackMove` orders choose and retain only targets that are currently
fireable. Out-of-range preferred targets should not occupy `target_id` or suppress fire into an
in-range fallback target, and target acquisition for these orders must not request a chase/repath as
a fallback.

## Scope

- Apply in-range/fireable target filtering to movement-order auto-acquisition and retained targets.
- Keep movement-order acquisition side-effect-free with respect to pathing: if no target is
  currently fireable, continue the commanded path rather than chasing a visible enemy.
- Treat player-issued `Move` and `AttackMove` consistently while a moving-fire unit is still
  pathing.
- Preserve the intended difference for units that cannot fire while moving: they may stop to shoot
  valid in-range targets on attack-move, but should not prefer unreachable targets while continuing
  their path.
- Add a focused regression shape based on the DV vs Soupman riflemen ignoring an in-range tank while
  holding an out-of-range soft target.
- Keep fog, smoke, line-of-sight, blocker, and team legality filters intact.

## Expected Touch Points

- `server/crates/sim/src/game/services/combat/acquisition.rs`
- `server/crates/sim/src/game/services/combat/priority.rs`
- `server/crates/sim/src/game/services/combat/mod.rs`
- focused combat tests for riflemen, tanks, scout cars, target retention, and target fallback
- `docs/design/balance.md`
- `docs/design/server-sim.md`

## Verification

- Focused Rust combat tests for attack-moving riflemen with an in-range armored fallback and an
  out-of-range preferred soft target.
- Focused Rust combat tests for moving-fire units retaining only fireable targets while moving.
- `git diff --check`.

## Manual Test Focus

Recreate the replay pattern: attack-move riflemen past a nearby tank while softer enemies are visible
farther away. The riflemen should shoot the in-range tank instead of walking with an unreachable
soft target selected.

## Handoff Expectations

Summarize the final target-retention rule for movement orders, list the regression scenarios added,
and call out any remaining cases where out-of-range acquisition is still intentional.
