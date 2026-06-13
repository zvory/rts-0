# Phase 1 - Baseline Metrics and Scenario Gates

Status: Not implemented.

## Objective

Establish a fast, repeatable evidence baseline for the current AI profiles before changing strategy
behavior. This phase should make it easy to tell whether later phases improve or regress opening
pressure, expansion, tank timing, army value, worker count, attacks launched, damage dealt, and
elimination outcomes.

## Scope

- Audit the existing `rts-ai` decision tests, self-play tests, matchup CLI, and balance matrix CLI.
- Add or refine scorecard fields for the AI 1.0 promotion bar:
  - first Rifleman attack command
  - first Scout Car completed
  - first Scout Car harassment command
  - first expansion City Centre planned and completed
  - first Tank completed
  - army value, building value, worker count, unit counts, attack commands, damage, deaths, and
    winner or tick-cap result
- Add compact authored scenario fixtures or self-play setup helpers for opening, mid-game
  expansion, tank tech, and blocked-goal situations.
- Add focused tests that pin the current `rifle_flood_full_saturation` baseline as runnable and
  selectable.
- Keep all new tooling profile-agnostic so later phases can compare old and new profiles without
  rewriting harness code.

## Expected Touch Points

- `server/crates/ai/src/selfplay/`
- `server/crates/ai/src/tools/matchup.rs`
- `server/crates/ai/src/tools/balance_matrix.rs`
- `server/crates/ai/src/ai_core/decision/tests.rs`
- `server/crates/ai/src/ai_core/profiles.rs`
- `docs/design/ai.md` if self-play, matchup output, or debug contracts change

## Verification

- Run focused AI crate tests that cover new scorecard and scenario helpers:

```bash
cd server && cargo test -p rts-ai
```

- Run at least one bounded matchup against the current baseline:

```bash
cargo run --manifest-path server/Cargo.toml -p rts-ai --bin ai-matchup -- \
  rifle_flood_full_saturation tech_to_tanks --ticks 8000 --seed 0 --no-verify-replay
```

- If replay verification or long profile-backed behavior changes, also run the relevant
  `RTS_FULL_AI_TESTS=1 cargo test` subset or document why it is deferred.

## Manual Testing Focus

Open any saved replay artifacts from new or changed scenario/matchup tooling and confirm the
scorecard numbers match what is visible: first attack, expansion, tank progress, and damage timing.
Confirm `rifle_flood_full_saturation` remains selectable and still behaves like the previous live
baseline.

## Handoff Expectations

The handoff must state which metrics now exist, where the fast baseline scenarios live, and which
baseline matchup results later phases should compare against. It should also name any promotion-bar
metric that still cannot be measured automatically.

## Player-Facing Outcome

No intended gameplay change. This phase improves the evidence used to build and tune AI 1.0.
