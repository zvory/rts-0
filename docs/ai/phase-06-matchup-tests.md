# AI-6: Matchup Tests

Done. Self-play now has profile-vs-profile matchup configuration for the three required first
profiles, shared milestone goals for economy/tech/production/combat behavior, and replay
determinism checks through the existing self-play success path. The old economy/build-tech scripts
were already removed by AI-5; `WorkerRushScript` and `MineOnlyScript` remain documented as
intentional scenario coverage rather than matchup profiles.

Replace brittle scripted coverage with profile-vs-profile tests where that gives better signal.

This phase should start only after live AI and self-play can both use the shared AI core.

## Goal

Create deterministic matchups that prove the first profiles are meaningfully different and continue
to exercise economy, tech, production, combat, and replay.

The initial matchup set is exactly the pairwise combinations of the required first profiles in
`AI-PLAN.md`: `rifle_flood_fast`, `rifle_flood_full_saturation`, and `tech_to_tanks`. Scenario
scripts such as worker rush or mine-only can remain useful tests, but they are not matchup
profiles unless the index promotes them to named profiles.

## Required Initial Matchups

### `rifle_flood_fast` vs `rifle_flood_full_saturation`

Purpose:

- prove early pressure exists
- prove full saturation still grows economy and transitions to combat

Useful assertions:

- fast flood launches or damages earlier than full saturation
- full saturation reaches a stronger worker/economy milestone
- both avoid supply deadlock

### `rifle_flood_fast` vs `tech_to_tanks`

Purpose:

- prove early pressure can threaten a greedy tech path
- prove tech path still progresses under pressure when not dead

Useful assertions:

- fast flood attacks before the first tank
- `tech_to_tanks` assigns oil workers
- `tech_to_tanks` builds prerequisites when it survives long enough

### `rifle_flood_full_saturation` vs `tech_to_tanks`

Purpose:

- prove macro rifle play and tech play both function in a less all-in matchup

Useful assertions:

- full saturation reaches strong rifle production
- `tech_to_tanks` reaches tank production
- combat happens after both sides have meaningful armies

## Test Style

Prefer milestone and ordering assertions over exact command sequences.

Good assertions:

- "trained at least N workers"
- "built at least one depot"
- "issued an attack command before first tank"
- "trained at least one tank"
- "combat damage happened"
- "replay matched live"

Avoid:

- exact tick for every command
- exact entity positions
- exact production queue sequences
- tests that pass only because of incidental unit ids

## Subtasks

### AI-6.1 Add Matchup Configuration

Add a small matchup config for self-play:

- player specs
- profile per player
- milestone goals per player
- combat goal
- maximum ticks
- artifact name

Keep this local to self-play unless live lobby profile selection needs it later.

### AI-6.2 Add Shared Milestone Helpers

Consolidate milestone checks that profiles need:

- worker count
- supply cap
- buildings by kind
- units by kind
- oil gathered or oil worker assignment
- first attack tick
- first damage tick
- tank trained

Preserve existing artifact payload value.

### AI-6.3 Add Replay Comparison for Matchups

Every matchup test should keep replay determinism checks.

If replay world hashing from `PLAN.md` Phase 1.2 is available, include first-divergent-tick
diagnostics. If not, keep the existing replay comparison and avoid broad nondeterministic changes.

### AI-6.4 Retire Superseded Scripts

Only after matchup tests are stable:

- remove or shrink old scripts that duplicate profile behavior
- keep scenario-specific scripts that still cover unique behavior
- update comments so future agents know why any script remains

Expected script handling:

- `EconomyScript` should be replaced or reduced once `rifle_flood_full_saturation` covers the same
  economy milestones.
- `BuildTechAttackScript` should be replaced or reduced once `tech_to_tanks` covers oil, tech, tank
  production, and mixed attack milestones.
- `WorkerRushScript` should remain until `proxy_rush` or an explicit all-in worker-pull helper
  covers that scenario.
- `MineOnlyScript` can remain as passive/minimal harness coverage where it helps isolate replay or
  snapshot behavior.

## Non-Goals

- No balance tuning wars.
- No exact win-rate targets.
- No nondeterministic random profile selection.
- No new profile names outside `AI-PLAN.md`.
- No replacing all self-play tests in one branch.

## Validation

Run the matchup tests and replay comparisons. Save or inspect artifacts for failures that are not
obvious from logs.

## Done Criteria

AI-6 is done when:

- the three required matchups exist
- each matchup has useful milestone assertions
- replay comparison passes for profile-backed matchups
- obsolete scripted duplication is removed or clearly marked as intentionally retained
