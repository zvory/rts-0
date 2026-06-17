# Tank Trap Pathing and Acquisition Plan

## Purpose

Refine Tank Trap movement and targeting so they behave like field obstacles instead of generic
walls. Friendly Tank Traps should shape vehicle routes, while enemy Tank Traps should be breachable
obstacles that vehicles can drive into, auto-acquire, destroy, and then move through. Infantry-like
units should treat Tank Traps as irrelevant for auto-acquisition, but direct attack orders against
enemy Tank Traps must remain legal.

## Current Rule Baseline

- Tank Traps are `StaticBlockerClass::VehicleBodyOnly` in `server/crates/rules/src/kind.rs`.
- `MovementBodyClass::InfantryLike` units can path and stand on Tank Trap tiles.
- `MovementBodyClass::VehicleBody` units currently path around Tank Traps without considering
  whether the trap is owned by self, ally, or enemy.
- Combat acquisition currently has a generic unit-preference pass, then falls back to nearest
  enemy targetable entity, which can include Tank Traps.
- Explicit attack commands validate against enemy targetability and visibility, not against
  auto-acquisition preference. Preserve that split.

## Target Rules

- Own and allied Tank Traps are vehicle blockers. Vehicle-body units should path around them.
- Enemy Tank Traps are breachable vehicle obstacles. Vehicle-body units should not include them in
  the A-star/static path blocker layer, so a direct movement order can route into the obstacle line.
- Enemy Tank Traps remain normal auto-acquisition targets for vehicle-body attackers. Tanks, Scout
  Cars, Anti-Tank Guns, Mortar Teams, and Artillery should be able to naturally shoot enemy Tank
  Traps when those traps are the nearest relevant target after current unit-preference rules.
- Infantry-like units must not auto-acquire Tank Traps. Riflemen, Machine Gunners, and Workers
  should pass through Tank Traps without stopping, setting target ids, setting up, or emitting attack
  events unless explicitly ordered to attack.
- Direct attack orders remain legal for infantry-like units against visible enemy Tank Traps.
- Friendly and allied Tank Traps are never legal hostile attack targets under existing team rules.
- Do not hide, despawn, make invulnerable, or otherwise special-case Tank Traps outside movement
  blocking and auto-acquisition policy. They remain normal visible, targetable, damageable
  buildings.

## Scenario Matrix

Phase 1 should encode these cases as automated regression coverage and human-inspectable dev
scenarios before Phase 2 changes behavior:

| Actor | Trap relation | Order | Expected behavior |
| --- | --- | --- | --- |
| Tank | own/allied wall | move through wall | routes around the wall; no attack events against friendly traps |
| Scout Car | own/allied wall | move through wall | routes around the wall; no attack events against friendly traps |
| Tank | enemy wall | move through wall | drives into the wall, auto-acquires a blocking/nearby enemy trap, emits attack events, destroys enough traps, then proceeds |
| Scout Car | enemy wall | move through wall | same breach behavior as Tank, scaled to its weapon and durability |
| Anti-Tank Gun / Mortar Team / Artillery | enemy wall | move or attack-move | vehicle-body pathing treats enemy traps as breachable; combat behavior remains bounded by existing setup/range rules |
| Rifleman | enemy trap line | move or attack-move through line | passes through without targeting or attacking traps |
| Rifleman with Methamphetamines charge | enemy trap line | explicit attack order while moving | direct order remains legal; existing moving-fire behavior may let the rifleman pass through while attacking |
| Rifleman with Methamphetamines charge | enemy trap line | move or attack-move only | still does not auto-acquire traps |
| Machine Gunner | enemy trap line | move or attack-move through line | passes through without setting up, stopping, or attacking traps |
| Worker | enemy trap line | move/gather/build-adjacent route through line | passes through without auto-attacking traps |
| Any infantry-like unit | own/allied trap line | move or attack-move through line | passes through; no friendly target ids or attack events |
| Any unit | visible enemy trap | direct attack order | explicit attack remains valid when the attacker can attack and the target is visible |

The Methamphetamines case is a direct-order regression, not an auto-acquisition exception. The
important invariant is that the infantry auto-acquisition filter never blocks explicit orders, and
existing charged-rifleman moving-fire behavior remains intact.

## Phase Summaries

### [Phase 1 - Scenario Matrix and Regression Harness](phase-1.md)

Add the dev scenarios and focused automated tests that describe the desired Tank Trap pathing and
auto-acquisition matrix before changing the behavior. These scenarios should include friendly
vehicle rerouting, enemy vehicle breaching with attack events and eventual progress, infantry
pass-through without auto-attacks, explicit infantry attack preservation, and charged rifleman
direct-order behavior. The phase should leave product behavior unchanged except for adding
development/test entry points.

### [Phase 2 - Owner-Aware Pathing and Acquisition Policy](phase-2.md)

Implement the rule changes behind shared policy helpers instead of scattered `TankTrap` branches.
Vehicle pathing should include own/allied Tank Traps as blockers and exclude enemy Tank Traps from
the static path blocker layer, while combat auto-acquisition should keep Tank Traps available to
vehicle-body attackers and filter them from infantry-like auto-acquisition only. The phase is done
only when the Phase 1 matrix passes and manual dev scenarios show vehicles breach enemy trap walls
while infantry passes through without unsolicited attacks.

## Overall Constraints

- Keep the server authoritative. Client previews and dev scenarios can help inspect behavior, but
  pathing, target selection, attack events, and damage must be decided by the simulation.
- Preserve the existing direct attack command contract. Do not add a generic "infantry cannot attack
  Tank Traps" validation rule.
- Preserve the existing combat target priority shape unless a focused test proves it conflicts with
  the target rules above. Units still prefer enemy units over buildings; within a bucket, nearest
  target wins.
- Keep team ownership explicit. Rules should distinguish own/allied/enemy Tank Traps through
  `TeamRelations` or an equivalent sim-owned relationship helper, not by raw owner equality alone.
- Keep movement/body classification as the main abstraction. Avoid hard-coding Rifleman,
  MachineGunner, Tank, or ScoutCar lists where `MovementBodyClass` expresses the rule.
- Do not make enemy Tank Traps non-targetable or non-colliding globally. They are ignored by
  strategic path planning for vehicle-body units, but still exist as attackable obstacles.
- Preserve panic-free tick behavior. Stale ids, destroyed traps, team changes, missing targets,
  invalid paths, and unreachable direct attacks must remain no-ops or normal order completion.
- Update design docs only if Phase 2 changes a documented contract. At minimum, the handoff should
  state whether `docs/design/server-sim.md`, `docs/design/balance.md`, or testing docs were touched
  or intentionally left unchanged.
- Use focused tests during development. The full `./tests/run-all.sh` gate remains the PR merge
  authority.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the phase head is
  reachable from `origin/main`.
- After each phase, the implementing agent must provide a handoff message describing verification
  results, what the next agent should do, and the core manual testing focus. Manual testing notes
  should cover the core scenarios, not an exhaustive matrix.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Suggested Execution

Run Phase 1 first and review the scenario/harness names before implementing behavior:

```bash
scripts/phase-runner.sh --plan tanktrap-pathing 1 --pr --wait
scripts/phase-runner.sh --plan tanktrap-pathing 2 --pr --wait
```
