# AI-4: Live AI Migration

Move real lobby/gameplay AI from the monolithic controller path onto the shared AI core.

Done. `AiController` now acts as the live adapter for `ai_core`: it owns the AI player id, the
selected profile id, and `AiDecisionMemory`, then builds a live observation and delegates command
synthesis to the shared profile decision loop. The default live profile is
`rifle_flood_full_saturation`.

This phase must preserve the current gameplay feature: host adds AI, match starts, AI grows its
economy, builds production, attacks, and remains replay-deterministic.

## Goal

`server/src/game/ai.rs` should become a live adapter and small state holder. The actual RTS
knowledge and command synthesis should come from `ai_core`.

Live AI should run one of the required profiles from `AI-PLAN.md`; it should not become an extra
live-only bot with copied economy, production, or attack code.

## Current Live Behavior to Preserve First

The current AI:

- thinks on a staggered cadence
- tracks escalating rifleman wave size
- trains workers to starting steel saturation
- builds depots before supply deadlock
- builds barracks
- trains riflemen
- assigns idle workers to steel
- stages riflemen on a rally line
- attacks public enemy start tiles

Do not remove these behaviors without replacing them with profile-driven equivalents and tests.

## Subtasks

### AI-4.1 Wrap Current Controller State

Keep `AiController` owning live-only state such as:

- player id
- selected profile id
- next wave size
- last wave launch tick
- any temporary migration flags

The controller should not own general RTS knowledge once the migration is complete.

### AI-4.2 Build Live Observation and Facts

Replace the ad hoc survey pass in `AiController::think()` with AI-1 observation/fact builders.

If a fact is missing, add it to the shared facts layer instead of recomputing it locally.

### AI-4.3 Replace Local Command Construction

Move each command-producing block to shared action helpers:

- depot build
- barracks build
- worker training
- rifleman training
- worker gather assignment
- attack-move/staging

Keep the emitted command order stable unless a test is updated intentionally.

### AI-4.4 Select a Default Profile

Choose a default live profile without changing lobby protocol.

Recommended default:

- `rifle_flood_full_saturation` if preserving current macro behavior matters most
- `rifle_flood_fast` if pressure and shorter matches matter most

Do not choose `tech_to_tanks` as the default merely because it is available; that would introduce
oil and tank behavior beyond the current live-AI feature. It is better covered first in profile and
self-play tests.

If multiple AIs are present, it is acceptable to assign deterministic profiles by AI slot later,
but that should still select from the canonical profile ids. Do not add UI in this phase.

### AI-4.5 Preserve Fairness Rule

Until scouting/memory exists, live AI attacks should continue using public information such as
enemy start tiles, not hidden enemy unit positions.

The live adapter may read authoritative state to build own facts, resource facts, and liveness
facts. Strategy code should not depend on hidden enemy positions.

### AI-4.6 Update Tests

Keep or update existing tests that assert:

- AI trains workers beyond the start
- AI reaches saturation target
- AI builds supply
- AI builds barracks
- AI produces riflemen
- AI damages an opponent
- AI replay determinism holds

Add a test that the live controller uses a profile id, even if it is hard-coded.

## Non-Goals

- No self-play script migration.
- No profile selection UI.
- No new protocol fields.
- No advanced micro.
- No behavior randomization.
- No extra live-only strategy names.

## Validation

Run targeted Rust tests for:

- `server/src/game/ai.rs`
- `server/src/game/ai_core/*`
- existing AI behavior tests in `server/src/game/mod.rs`

If a self-play replay determinism test starts failing, stop and fix determinism before adding more
profile behavior.

## Done Criteria

AI-4 is done when:

- `AiController::think()` delegates most facts and commands to shared AI core
- the live AI runs one of the shared profiles
- the lobby AI feature still works without protocol changes
- live AI behavior tests and replay determinism remain green
