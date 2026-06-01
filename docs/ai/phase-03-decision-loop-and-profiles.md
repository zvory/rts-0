# AI-3: Decision Loop and Strategy Profiles

Introduce the shared decision loop and the first strategy profiles.

This is the phase where the AI starts becoming multiple personalities. The implementation should
still avoid multiple copied bots.

## Goal

Create:

- one deterministic decision loop
- profile data that parameterizes that loop
- the first three required profiles:
  - `rifle_flood_fast`
  - `rifle_flood_full_saturation`
  - `tech_to_tanks`

## Suggested Files

- Add `server/src/game/ai_core/decision.rs`.
- Add `server/src/game/ai_core/profiles.rs`.
- Extend `server/src/game/ai_core/actions.rs`.
- Extend tests near the new modules.

## Decision Loop Shape

Keep the loop simple and inspectable.

Recommended order:

1. refresh facts
2. handle urgent supply
3. satisfy required tech/building goals
4. train workers toward profile target
5. train combat units by profile priorities
6. assign idle workers to resources
7. stage or launch army actions

This is a ranked checklist with policy inputs, not a generic planner.

## Profile Shape

Suggested fields:

- `id`
- worker policy
  - steel saturation fraction or cap
  - extra oil workers
  - whether to delay workers for early pressure
- supply policy
  - free supply buffer
  - emergency depot threshold
- building policy
  - target barracks curve
  - required tech path
  - max pending buildings per kind
- production policy
  - queue depth
  - unit priority list
  - whether to save for first tech unit
- attack policy
  - first attack size
  - wave growth
  - regroup/reset behavior
  - reissue cadence
- resource policy
  - steel-first vs oil timing

Do not let a profile provide its own full `think()` function unless a later phase proves that a
small explicit override is unavoidable.

## Required Profiles

### `rifle_flood_fast`

Behavior targets:

- lower worker target than full saturation
- early barracks
- shallow rifle queue
- small first attack wave
- frequent pressure

Validation examples:

- produces riflemen before the full-saturation profile would normally move out
- sends an attack before tech-to-tanks reaches tanks in a direct matchup

### `rifle_flood_full_saturation`

Behavior targets:

- saturates starting steel first
- builds supply before choking
- scales barracks when steel bank grows
- attacks with a larger first wave than `rifle_flood_fast`

Validation examples:

- reaches starting steel saturation
- eventually transitions into rifle pressure
- does not deadlock on supply

### `tech_to_tanks`

Behavior targets:

- assigns oil workers
- builds barracks if required for tech chain
- builds tech prerequisites
- builds tank factory
- saves for first tank when needed
- attacks with riflemen plus at least one tank

Validation examples:

- gathers oil
- builds required tech structures
- trains a tank
- can still defend or pressure enough to avoid idle deadlock

## Subtasks

### AI-3.1 Add Profile Definitions

Add profile ids and static profile definitions.

Keep them in code first. Do not introduce TOML or client-visible profile selection in this phase.

### AI-3.2 Add Decision Output Type

If useful, add an intermediate `AiIntent` or `AiDecision` type before commands.

Only add it if it makes tests clearer. Do not build a large planning IR prematurely.

### AI-3.3 Implement Macro Decision Loop

Use facts from AI-1 and actions from AI-2.

The first implementation may cover:

- supply
- buildings
- worker training
- combat-unit production
- gather assignment
- attack readiness

Leave unit-specific micro to AI-7.

### AI-3.4 Add Profile Unit Tests

Write small tests that compare decisions between profiles.

Examples:

- fast flood has lower first attack threshold than full saturation
- tech-to-tanks requests oil workers and a tank factory path
- full saturation requests more workers before production pressure
- all profiles use deterministic profile ids and stable priorities

### AI-3.5 Keep Old Behavior Available

If the live AI cannot migrate immediately, keep the old controller path available while the shared
decision loop is tested in isolation. Do not delete the old path until AI-4.

## Non-Goals

- No lobby UI for choosing profiles.
- No protocol changes.
- No self-play script deletion.
- No machine-gunner/AT/tank micro beyond producing tanks for `tech_to_tanks`.
- No opponent adaptation.

## Validation

Run unit tests for profile decisions and action synthesis. If the shared loop can run in a small
game fixture, add one smoke test that each required profile emits plausible commands from the
starting state.

## Done Criteria

AI-3 is done when:

- the shared decision loop exists
- the three required profiles exist
- profile differences are data-driven and visible
- tests prove that profiles produce meaningfully different priorities
