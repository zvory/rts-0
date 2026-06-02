# AI-5: Self-Play Migration

Move self-play from bespoke RTS scripts toward shared AI profiles while preserving artifact and
milestone quality.

This phase should be incremental. Do not delete a scripted scenario until the replacement matchup
or profile-driven script gives equal or better coverage.

## Goal

Self-play should use the same AI knowledge, action synthesis, and strategy profiles as live AI.
`server/src/game/selfplay.rs` should primarily own orchestration, milestones, replay comparison,
and artifact writing.

## Current Scripts

Important existing scripts include:

- `BuildTechAttackScript`
- `EconomyScript`
- `WorkerRushScript`
- `MineOnlyScript`

These scripts contain useful coverage, but they also duplicate AI mechanics. Migration should keep
the coverage and remove duplication.

Treat these as harness scenarios, not as the strategy profile list. The intended mapping is:

- `EconomyScript` -> `rifle_flood_full_saturation` or a profile-backed economy scenario.
- `BuildTechAttackScript` -> `tech_to_tanks`.
- `WorkerRushScript` -> keep as a special all-in worker-pull scenario until a real replacement
  exists; `rifle_flood_fast` may replace generic early rifle pressure, but not worker-pull
  semantics.
- `MineOnlyScript` -> keep as passive/minimal harness coverage when needed; it is not an AI
  strategy profile.

## Subtasks

### AI-5.1 Add Profile-Backed Script Wrapper

Add a self-play `ScriptedPlayer` implementation that:

- owns a profile id
- owns any required persistent AI state
- converts `PlayerView` into shared observation/facts
- runs the shared decision loop
- returns ordinary `Command`s

This wrapper is the bridge from the self-play harness to `ai_core`.

### AI-5.2 Migrate Economy Coverage

Replace or reduce `EconomyScript` using a profile-backed script.

Coverage to preserve:

- workers train beyond starting count
- workers gather steel
- supply is built before deadlock
- replay remains deterministic

Good replacement profile:

- `rifle_flood_full_saturation` with combat disabled or milestone assertions focused on economy
  only, if the profile supports that cleanly

### AI-5.3 Migrate Tech Coverage

Replace or reduce `BuildTechAttackScript`.

Coverage to preserve:

- oil workers are assigned
- tech structures are built in valid order
- tank factory is built
- tank is trained
- mixed army can attack
- artifacts explain missing milestones

Good replacement profile:

- `tech_to_tanks`

### AI-5.4 Keep Worker Rush Until a Real Replacement Exists

`WorkerRushScript` covers a special early aggression scenario. Do not delete it merely because it is
not a normal profile.

`rifle_flood_fast` is the correct replacement for early rifle-pressure coverage. It is not a
drop-in replacement for all-in worker-pull coverage, because its intent is to commit to production
early, not to redefine the worker economy as the attack force.

Possible replacements:

- future `proxy_rush`
- future explicit "all-in worker pull" test helper

Until then, keep it isolated as a test-only script.

### AI-5.5 Migrate Shared Pending-Build Semantics

Self-play has pending-build and failed-build-spot logic because snapshots do not immediately prove
that a build command succeeded.

Centralize what is general:

- pending build intent representation
- watchdog expiry
- skip failed spots

Keep artifact/report-specific details in `selfplay.rs`.

### AI-5.6 Update Milestones

Profile-backed tests should assert milestones, not exact command sequences.

Examples:

- economy profile reaches worker/supply thresholds
- fast flood deals damage before a broad deadline
- `tech_to_tanks` trains a tank
- profile emits at least one combat command when attack conditions are met
- worker-rush scenarios, while retained, assert their special worker-pull behavior explicitly

### AI-5.7 Remove Duplicated Mechanics Gradually

After a script is migrated:

- delete duplicated worker assignment from the old script
- delete duplicated build/training budget logic
- keep small scenario-specific wrappers only where they add test value

Do this one script at a time.

## Non-Goals

- No broad self-play harness rewrite.
- No artifact format rewrite unless necessary.
- No deletion of coverage without replacement.
- No exact tick-perfect assertions.
- No profile UI.

## Validation

Run the relevant self-play tests for each migrated script.

If a self-play failure is not immediately obvious, follow the repo guidance: start a fresh local
server on its own port and use the macOS `open` command to inspect the replay URL. Do not use the
Browser skill for that replay flow.

## Done Criteria

AI-5 is done when:

- self-play can run profile-backed scripts for the canonical profiles that have migrated
- `BuildTechAttackScript` and `EconomyScript` are reduced or replaced by shared-profile logic
- `WorkerRushScript` and `MineOnlyScript`, if still present, are documented as intentionally
  retained scenario coverage rather than strategy profiles
- artifacts and milestone diagnostics remain useful
- replay comparison still passes
