# Phase 1 - Separate Moving-Fire Policy

## Phase Status

Status: planned.

## Objective

Separate the combat capability "can fire while moving" from vehicle-specific weapon handling and
from auto-chase pathing. A unit following a player-issued `Move` or `AttackMove` destination should
not replace that path merely because it can fire while moving.

## Scope

- Split or rename policy helpers so moving-fire capability, vehicle/turret handling, and chase
  permission are represented separately.
- Make active `Move` and `AttackMove` paths stay pointed at their commanded destinations for current
  moving-fire units.
- Keep explicit `Attack` command pursuit and idle-aggressive behavior separate from movement-order
  drive-by fire.
- Keep vehicle-only aiming, turret facing, standoff, and projection logic tied to vehicle policy
  rather than shared moving-fire policy.
- Update design docs where they currently say attack-moving vehicles can chase out-of-range targets.

## Expected Touch Points

- `server/crates/rules/src/kind.rs`
- `server/crates/sim/src/game/services/combat/`
- `server/crates/sim/src/rules/projection.rs`
- focused combat tests around tanks, scout cars, attack-move, move orders, and direct attack
- `docs/design/balance.md`
- `docs/design/server-sim.md`
- `docs/design/hardening.md` if its moving-fire guarantees change

## Verification

- Focused Rust tests covering moving-fire `Move` and `AttackMove` path preservation.
- Focused Rust tests showing explicit `Attack` still pursues as intended.
- Sim architecture check if helper movement changes module boundaries.
- `git diff --check`.

## Manual Test Focus

Check a tank or scout car moving past enemies: it should keep its commanded path and fire only when
able. Also check a direct attack order against an out-of-range enemy so intentional pursuit still
works.

## Handoff Expectations

Name the final helper/policy split, summarize any behavior that changed for tanks or scout cars, and
call out any old chase/standoff tests that were removed or rewritten.
