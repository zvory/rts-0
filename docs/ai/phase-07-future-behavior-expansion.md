# AI-7: Future Behavior Expansion

Add richer personas and unit-specific behavior after the shared core is stable.

Do not start this phase to compensate for missing foundations. Advanced behavior becomes expensive
quickly if observations, actions, profiles, and tests are still duplicated.

## Goal

Use the shared AI core to add more interesting strategy and tactics without forking whole bots.

## Candidate New Profiles

### `standard`

Balanced baseline.

Expected style:

- saturate a reasonable worker count
- build moderate rifle pressure
- tech when safe
- defend if threatened
- attack with mixed forces

Use as the default human-facing AI once it is better than the current rifle pressure bot.

### `proxy_rush`

Aggressive forward production.

Expected style:

- send a worker toward a forward build zone
- build barracks closer to enemy or center
- produce early riflemen
- accept economic risk

Required foundations:

- safe forward build-site selection
- better path/failure handling for long-distance builders
- public/fair target selection rules

### `eco_expand`

Economy-first profile.

Expected style:

- prioritize workers and depots
- claim additional resources when expansion mechanics exist
- delay attack until economy lead converts to production

Required foundations:

- expansion/base location facts
- worker transfer or new town hall behavior, if added

### `tech_to_support_weapons`

Machine-gunner and AT-team focused profile.

Expected style:

- tech to support infantry
- use machine gunners defensively or to hold lanes
- mix AT teams when enemy armor appears or is expected

Required foundations:

- machine-gunner setup/teardown behavior in shared tactics
- composition facts and threat facts

## Tactical Controllers

Add small tactical controllers only when a unit needs special handling.

Good candidates:

- machine gunner
  - avoid moving deployed units unnecessarily
  - deploy near rally/defense points
  - tear down only for meaningful repositioning
- tank
  - avoid unsupported dives when AT teams exist
  - group with rifle support
  - focus structures or machine gunners when appropriate
- AT team
  - prefer tank targets
  - hold forests or choke points after terrain mechanics exist

Keep tactics local and limited. Do not build a general behavior-tree framework unless repeated
controllers prove that simple functions are not enough.

## Future Perception Work

Possible additions:

- enemy memory from last seen units
- scouting tasks
- threat map
- influence map
- attack lane scoring
- defense point scoring
- terrain-aware rally points

Add these as facts/maps with tests. Do not let every profile compute its own spatial model.

## Future UI and Protocol Work

Profile selection can eventually become user-facing, but it is not part of the first refactor.

When it is needed:

- update protocol docs in `DESIGN.md`
- update server protocol types
- update client protocol helpers
- update lobby UI
- add compatibility behavior for old rooms if needed

Until then, choose profiles server-side for tests and default live AI behavior.

## Validation

Every new persona should come with:

- profile-level unit tests
- at least one self-play matchup or scenario milestone
- replay comparison
- artifact inspection on non-obvious failures

Every new tactical controller should come with:

- focused unit tests for decision thresholds
- at least one integration test proving it issues legal commands
- a regression test for any bug it fixes

## Done Criteria

AI-7 is ongoing future work. Each new behavior is done only when it uses shared facts/actions,
preserves determinism, and has matchup or focused integration coverage.
