# Attack Priority Plan

## Purpose

Create a small, durable attack-priority system that replaces the current chain of target-selection
special cases with one explicit ranking boundary. The first iteration should make default weapons
choose sensible targets: small-arms units prefer soft targets, anti-armor weapons prefer armor and
anti-armor threats, Tanks still treat Anti-Tank Guns as the highest immediate threat, and Tank Trap
targeting is tied to movement obstruction rather than raw kind checks. Future grenades, satchel
charges, melee demolitions, autocast abilities, or alternate weapons must be able to plug into this
shape without turning default auto-acquisition into a full tactical ability planner.

## Current Baseline

- `server/crates/rules/src/defs.rs` owns `ArmorClass`, `WeaponClass`, and the current narrow
  `TargetPriority` field.
- `server/crates/rules/src/combat.rs` owns AP/armor predicates, damage formulas, miss chance, and
  the current Tank-only target priority list.
- `server/crates/sim/src/game/services/combat/acquisition.rs` owns target acquisition and currently
  sequences several procedural decisions: explicit target retention, Tank-specific priorities,
  moving-fire target retention, Anti-Tank Gun tank preference, unit-over-building preference, and
  nearest-target fallback.
- `server/crates/sim/src/game/services/world_query.rs` owns nearest hostile target scans, but it does
  not expose ranked candidate selection.
- Tank Trap behavior is intentionally split today: explicit attacks remain legal, infantry-like
  auto-acquisition ignores traps, vehicle-body auto-acquisition can target traps, movement pathing
  has a separate owner-aware `VehicleBodyOnly` blocker policy, and Tank Traps do not block shots.

## First-Iteration Target Rules

- Explicit `Attack` orders remain command intent. They keep the target if it remains enemy,
  targetable, visible, and fireable under the existing ordered-attack checks; ranking is only for
  acquisition after explicit target loss or for non-ordered combat modes.
- The first ranking system applies only to the unit's default attack profile. Grenades, satchels,
  sticky bombs, demolition charges, melee charges, or future alternate attacks must not silently make
  riflemen auto-walk into tanks unless a later phase explicitly adds a conservative autocast policy.
- Legal candidate filtering stays separate from ranking. A candidate must already pass hostile
  targetability, visibility, smoke, line-of-sight, friendly-blocker, acquisition-radius, stance, and
  direct/indirect-fire constraints before priority compares it.
- Small-arms default attacks prefer `ArmorClass::Small` targets over armored or hard targets when
  both are legal. They should still fall back to armored or hard targets when no better legal target
  exists.
- Anti-armor default attacks prefer anti-armor threats and armored/hard targets over ordinary soft
  targets. Anti-Tank Guns remain the highest priority target for Tanks because they are the clearest
  immediate anti-armor threat.
- Tank priority becomes a role/threat policy rather than a Tank-only kind list. The first iteration
  should preserve the important current outcomes: Tanks prefer Anti-Tank Guns, then other relevant
  armor/AP threats, then armored obstacles or vehicles, then lower-priority soft targets.
- Unit-over-building preference remains, but it becomes one rank term rather than a separate scan
  pass. A high-threat building or obstruction can outrank a low-value unit only when the policy says
  so.
- Tank Trap priority is context-sensitive. Vehicles should prioritize enemy Tank Traps only when a
  trap is blocking, pinching, or immediately obstructing the unit's intended movement route; otherwise
  Tank Traps should not outrank real combat threats just because they are nearby.
- Infantry-like units still do not auto-acquire Tank Traps. Direct attack orders against visible
  enemy Tank Traps remain legal.
- Target retention must be deliberate. Moving-fire units may keep firing at a valid current target,
  but a newly visible higher-rank threat such as an Anti-Tank Gun must be able to override that
  retained lower-rank target under a documented rule.

## Overall Constraints

- Keep the server authoritative. Client code must not decide target priority, target legality, or
  obstruction context for real matches.
- Keep `Game::tick()` panic-free. Stale ids, destroyed targets, hidden targets, invalid path state,
  missing movement intents, non-finite positions, and empty candidate sets must degrade to no target
  or existing fallback behavior.
- Keep acquisition deterministic. Rank comparisons must have stable tie-breaks, ending with distance
  and entity id where appropriate.
- Preserve fog and smoke authority. Ranking must not inspect or retain hidden enemy data beyond what
  the existing visibility and event projection contracts allow.
- Keep low-level rules pure. `rules::combat` may describe immutable target classes, attack profiles,
  and rank policy inputs, but it must not depend on sim state, pathing, fog, or entity stores.
- Keep sim-local context in sim. Movement obstruction, current order, retained target id, path goal,
  line-of-sight, fog, and smoke belong in `rts-sim` combat/movement services.
- Keep the ranking boundary real. After Phase 2, normal auto-acquisition should read as: handle
  explicit ordered-attack intent, collect already-legal candidates, rank them with named terms, and
  return the winner. If `resolve_target` still contains parallel Tank, Anti-Tank Gun,
  unit-over-building, retained-target, or Tank Trap priority branches that compete with the ranker,
  stop before Phase 3 and either fold them into the boundary or document a narrow, temporary
  exception with an owner and cleanup point.
- Keep movement obstruction facts one-way into combat. Combat may consume bounded, read-only facts
  derived from occupancy, current path segment, `path_goal`, `move_intent`, or recent path/chase
  failure state, but it must not run A* per target, mutate movement/path state while ranking, or reach
  around `MoveCoordinator`/pathing APIs for implementation details.
- Do not add per-unit `if kind == ...` branches where a role, weapon profile, armor class, movement
  body class, or target tag expresses the rule. If a one-off is necessary, isolate and name it as an
  explicit policy term.
- Do not implement grenade, satchel, demolition, or alternate-weapon autocast in this plan. The plan
  must leave a clean extension point for those attacks, but default auto-acquisition remains about
  default weapons only.
- Do not change wire protocol, snapshots, client command surfaces, or ability metadata unless a
  later approved attack-profile feature actually needs them.
- Update `docs/design/server-sim.md` and `docs/design/balance.md` when behavior or rule ownership
  changes. If only tests or internal helpers change, the phase handoff should state why docs were
  unchanged.
- Use focused Rust tests during development. Run `cargo test --manifest-path server/Cargo.toml -p
  rts-rules ...` for pure rules changes and `cargo test --manifest-path server/Cargo.toml -p rts-sim
  ...` for combat acquisition changes. The full `./tests/run-all.sh` gate remains the PR merge
  authority.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the phase head is reachable
  from `origin/main`.
- After each phase, the implementing agent must provide a handoff message describing exact
  verification, behavior affected, remaining risks, what the next agent should do, and what should be
  manually tested. Manual testing notes should cover core combat scenarios, not an exhaustive matrix.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Phase Summaries

### [Phase 1 - Baseline Contract Tests](phase-1.md)

Lock down the current acquisition contracts before introducing a new ranking abstraction. This phase
should add focused tests for explicit attack preservation, Tank Anti-Tank Gun priority, moving-fire
retention override, Anti-Tank Gun armored preference, small-arms fallback behavior, infantry Tank
Trap ignore rules, vehicle Tank Trap acquisition, and fog/smoke/fireability rejection. It should not
change gameplay; it creates the regression net that lets later phases refactor without guessing.

### [Phase 2 - Ranking Boundary Without Gameplay Change](phase-2.md)

Introduce a combat target ranking module and candidate shape, then route current acquisition through
it while preserving current behavior. This phase should separate legal-candidate collection from rank
comparison, provide stable tie-breaks, and move the existing Tank priority, Anti-Tank Gun preference,
unit-over-building preference, retained moving-fire target rule, and Tank Trap filters into named rank
or eligibility terms. Done correctly, Phase 2 should be mostly architecture with tests proving no
intentional gameplay change; if it leaves the old procedural branches alongside the new ranker, later
gameplay phases should pause instead of building on a split decision path.

### [Phase 3 - Default Weapon Fit Policy](phase-3.md)

Use the new ranking boundary to ship the first real priority behavior. Small-arms default attacks
should prefer soft targets over armored or hard targets, anti-armor default attacks should prefer
armored/hard targets and anti-armor threats, and Tank target priority should generalize from a fixed
kind list into a threat/role policy that still puts Anti-Tank Guns first. This phase should update
rules docs and focused combat tests with player-facing patch notes for the changed targeting behavior.
Make these behavior changes by adjusting rank terms, not by adding new special-case branches to
`resolve_target`.

### [Phase 4 - Retargeting And Future Attack Profile Guardrails](phase-4.md)

Make target retention and future alternate attacks explicit so the priority system stays stable as
grenades and satchels arrive later. This phase should document and test when a unit keeps its current
target, when a higher-rank target can steal focus, and how default weapon ranking is kept separate
from explicit-only or future autocast ability profiles. It should not add new abilities; it prevents
the first priority system from becoming an accidental ability planner.

### [Phase 5 - Tank Trap Obstruction Context](phase-5.md)

Tie vehicle Tank Trap priority to movement obstruction instead of raw proximity. This phase should
add a small sim-owned obstruction context from movement/pathing or occupancy into combat acquisition,
then rank enemy Tank Traps highly only when they block, pinch, or immediately obstruct the unit's
current route or intended movement. Infantry-like auto-acquisition should still ignore Tank Traps,
explicit attacks should remain legal, and vehicles should stop wasting priority on irrelevant nearby
traps while still breaching real obstacles. The obstruction query must stay bounded and read-only; if
the implementation wants full pathfinding during target ranking, split that design problem out before
continuing.

## Phase Index

1. [Phase 1 - Baseline Contract Tests](phase-1.md)
2. [Phase 2 - Ranking Boundary Without Gameplay Change](phase-2.md)
3. [Phase 3 - Default Weapon Fit Policy](phase-3.md)
4. [Phase 4 - Retargeting And Future Attack Profile Guardrails](phase-4.md)
5. [Phase 5 - Tank Trap Obstruction Context](phase-5.md)

## Non-Goals

- Do not implement grenades, satchel charges, demolition charges, sticky bombs, melee attacks, or
  alternate-weapon autocast.
- Do not make default auto-acquisition solve full tactical optimization across abilities, cooldowns,
  risk, travel time, focus fire, overkill, or group composition.
- Do not change damage formulas except where a phase explicitly needs a pure classification helper.
- Do not change client command UI, wire protocol, compact protocol, snapshots, fog projection, or
  replay format.
- Do not make Tank Traps globally non-targetable, non-colliding, invisible, or invulnerable.
- Do not remove explicit attack support for poor but intentional targets. Player commands may still
  force a Rifleman to attack a Tank Trap or a Tank when visible and legal.
- Do not add a hard-coded priority table for every unit kind. The point of this plan is to replace
  that failure mode with named policy terms.

## Future Extension Point

Future alternate attacks should be modeled as separate attack profiles, not as mutations to the
default weapon's target priority. A Rifleman might eventually have:

- a rifle profile with medium range, small-arms damage, and soft-target preference;
- a grenade profile with short range, partial hard-target utility, and explicit or conservative
  autocast rules;
- a satchel profile with melee range, demolition/AP utility, and explicit-only behavior until a
  separate autocast plan proves safe.

Each profile should declare its own legal targets, activation mode, range, cooldown/resource
constraints, and ranking policy. Default auto-acquisition should choose a target for the active
default weapon; it should not silently decide to path into melee range because an unused special
attack would be good in theory.

## Suggested Execution

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait gate
and confirm the phase head is reachable from `origin/main`.

```bash
scripts/phase-runner.sh --plan attackpriority phase-1 --pr --wait
scripts/phase-runner.sh --plan attackpriority phase-2 --pr --wait
scripts/phase-runner.sh --plan attackpriority phase-3 --pr --wait
scripts/phase-runner.sh --plan attackpriority phase-4 --pr --wait
scripts/phase-runner.sh --plan attackpriority phase-5 --pr --wait
```
