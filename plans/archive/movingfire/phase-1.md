# Phase 1 - Separate Moving-Fire Policy

## Phase Status

Status: done.

## Objective

Separate the combat capability "can fire while moving" from vehicle-specific weapon handling and
from auto-chase pathing. A moving-fire unit following a player-issued `Move` or `AttackMove`
destination should not replace that path merely because combat auto-acquisition saw an enemy.

## Scope

- Split or rename policy helpers so moving-fire capability, vehicle/turret handling, and chase
  permission are represented separately.
- Remove auto-acquisition-driven repaths for moving-fire units on player-issued `Move` and
  `AttackMove` orders. These moving-fire orders must not chase visible enemies, must not route to
  vehicle standoff points, and must not replace the path goal with an enemy-directed goal.
- Make active moving-fire `Move` and `AttackMove` paths stay pointed at their commanded
  destinations for all affected units, including current moving-fire vehicles.
- Do not redefine post-arrival `AttackMove` semantics. Once the commanded destination has been
  reached, preserve the current idle/aggressive behavior exactly as it exists before this phase.
- Preserve normal non-moving-fire attack-move engagement semantics unless a later phase makes a
  targeted target-filtering change.
- Keep explicit `Attack` command pursuit and idle-aggressive behavior separate from movement-order
  drive-by fire.
- Keep vehicle-only aiming, turret facing, standoff, and projection logic tied to vehicle policy
  rather than shared moving-fire policy.
- Update contradictory design-doc passages where they currently say attack-moving vehicles can
  chase out-of-range targets or use standoff paths during player-issued movement orders, especially
  `docs/design/balance.md`, `docs/design/server-sim.md`, and `docs/design/hardening.md`.

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
- Focused Rust tests proving visible out-of-range enemies do not change moving-fire `Move` or
  `AttackMove` path goals.
- Focused Rust tests should cover active movement-order path preservation without asserting a new
  post-arrival `AttackMove` behavior.
- Focused Rust tests proving normal non-moving-fire attack-move behavior is not accidentally
  disabled.
- Focused Rust tests showing explicit `Attack` still pursues as intended.
- Sim architecture check if helper movement changes module boundaries.
- `git diff --check`.

## Manual Test Focus

Check a tank or scout car moving past enemies: it should keep its commanded path and fire only when
able, with no enemy-directed repath or standoff path. Also check a direct attack order against an
out-of-range enemy so intentional pursuit still works.

## Handoff Expectations

Name the final helper/policy split, summarize any behavior that changed for tanks or scout cars, and
call out any old chase/standoff tests that were removed or rewritten.
