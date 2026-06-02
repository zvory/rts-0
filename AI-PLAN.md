# AI PLAN

This is the canonical AI planning index for gameplay AI and AI-driven self-play.

Use this file to decide what phase comes next. Use the linked phase files for implementation
details. `PLAN.md` stays the top-level project dependency map; this file owns the AI architecture,
AI rollout order, and AI-specific handoff tasks.

## How to Use This Plan

- Work phases in order unless a later phase explicitly says it is parallel-safe.
- Keep each implementation branch scoped to one phase or one subtask inside a phase.
- Read `DESIGN.md` before implementation; update it in the same change if a public contract changes.
- Keep live AI, self-play, replay, and human players on the ordinary `Command` path.
- Prefer deterministic, inspectable heuristics over opaque planning/search frameworks.
- Do not add a second AI roadmap. Update this index and the relevant detailed phase file instead.

## Current State

The live gameplay AI is currently a single `AiController` in `server/src/game/ai.rs`. It:

- surveys authoritative state directly
- keeps workers mining steel
- trains workers up to starting steel saturation
- builds depots and barracks
- pumps riflemen
- stages riflemen on a rally line
- launches escalating rifleman waves at public enemy start tiles

Shared helper extraction has started in `server/src/game/ai_shared.rs`:

- deterministic near-base build-site search
- worker saturation target helpers for entity and snapshot views
- local spend reservation
- basic attack-wave readiness

Self-play still has separate scripted RTS logic in `server/src/game/selfplay.rs`, including
`BuildTechAttackScript`, `EconomyScript`, `WorkerRushScript`, and `MineOnlyScript`. Those scripts
duplicate worker assignment, production, pending-build tracking, tech progression, and attack
logic that should eventually live in the shared AI core.

## Goal

Build a maintainable AI system that:

- supports at least three first-class strategy profiles:
  - `rifle_flood_fast`
  - `rifle_flood_full_saturation`
  - `tech_to_tanks`
- can later add profiles such as `standard`, `proxy_rush`, and `eco_expand`
- shares game knowledge across live AI and self-play
- stays deterministic under replay
- keeps strategy differences in profiles, priorities, and thresholds rather than copied mechanics
- remains easy to update when balance, tech requirements, or command validation change

## Non-Goals

These are intentionally out of scope for the first architecture pass:

- machine learning
- deep future-branch search
- generic GOAP/HTN framework adoption
- large behavior-tree framework adoption
- adaptive opponent modeling
- perfect play
- sophisticated scouting
- advanced unit micro beyond small targeted controllers

The near-term target is believable, maintainable, testable play.

## Main Architectural Decision

Do not build several separate bots that each know how to play the whole game.

Build one shared AI core with:

1. a constrained world-model and facts layer
2. a shared action-synthesis layer
3. one deterministic decision loop
4. thin strategy profiles
5. adapters for live gameplay AI and self-play
6. matchup tests that exercise personalities instead of brittle scripts

The useful model is not "give a generic optimizer levers and an objective function." The useful
model is a small RTS-specific hierarchy where each layer has a limited action space and good
derived facts.

## Proposed Module Ownership

This is the intended direction. Adjust names only if the implementation reveals a better local fit.

- `server/src/game/ai.rs`
  - live AI adapter
  - `AiController` state and cadence
  - profile selection wiring for real AI players
- `server/src/game/ai_shared.rs`
  - temporary compatibility home for already-extracted helpers
  - should shrink over time as helpers move into the shared core
- `server/src/game/ai_core/`
  - `mod.rs`
  - `observation.rs`
  - `facts.rs`
  - `actions.rs`
  - `decision.rs`
  - `profiles.rs`
  - optional `tactics.rs` only when needed
- `server/src/game/selfplay.rs`
  - test orchestration
  - artifact writing
  - milestone assertions
  - adapter from self-play `PlayerView` to shared AI core

Keep `ai.rs` as a file for the live adapter while the new core is introduced. Avoid creating an
`ai/` directory unless `ai.rs` is deliberately moved, because Rust cannot use both module shapes
for the same module name.

## Phase Map

| Phase | Detailed Plan | Status | Main Output |
| --- | --- | --- | --- |
| AI-0 | [Boundary and invariants](docs/ai/phase-00-boundary-and-invariants.md) | planned by this doc set | One AI architecture contract and handoff rules |
| AI-1 | [Shared world model](docs/ai/phase-01-shared-world-model.md) | partial helper extraction exists | Deterministic AI observations and reusable facts |
| AI-2 | [Action synthesis](docs/ai/phase-02-action-synthesis.md) | not started | Shared command builder with budget and reservation semantics |
| AI-3 | [Decision loop and profiles](docs/ai/phase-03-decision-loop-and-profiles.md) | not started | `rifle_flood_fast`, `rifle_flood_full_saturation`, `tech_to_tanks` profiles |
| AI-4 | [Live AI migration](docs/ai/phase-04-live-ai-migration.md) | not started | `AiController` delegates to the shared core |
| AI-5 | [Self-play migration](docs/ai/phase-05-selfplay-migration.md) | not started | Self-play scripts replaced or reduced by shared profiles |
| AI-6 | [Matchup tests](docs/ai/phase-06-matchup-tests.md) | not started | Personality-vs-personality coverage and replay checks |
| AI-7 | [Future behavior expansion](docs/ai/phase-07-future-behavior-expansion.md) | future | Proxy, eco, standard, MG, AT, tank, and terrain-aware behavior |

## Dependency Gates

From `PLAN.md`, advanced AI depends on:

- Phase 1.2 replay world hashing
- Phase 2.2 world helpers
- Phase 2.3 formal command processor
- Phase 3.1 shared definition registry
- later relevant unit mechanics, such as machine-gunner setup behavior

Practical interpretation:

- AI-1 and AI-2 can continue extracting current duplicated helper logic before all gates land.
- Any change that depends on tech requirements, unlock chains, command error semantics, or
  generated definitions should either wait for the relevant gate or be written as a narrow interim
  adapter with a removal note.
- AI-6 should not replace important scripted coverage until replay determinism diagnostics are good
  enough to explain failures.

## Required First Profiles

### `rifle_flood_fast`

Intent:

- build one engineer
- mine steel with all engineers, except one which goes to the middle of the map
- engineer in middle of map creates a barracks at 150 steel and returns home
- nonstop rifleman production from the centre barracks
- riflemen attack the enemy
- after building a supply depot at 10, transition to another profile?

### `rifle_flood_full_saturation`

Intent:


economic priorities:
- build supply depots if low headroom
- build workers until main steel patch is saturated
- build riflemen
- if everything else is in progress and there's >300 steel, build barracks

riflemen are sent to attack the enemy in increasing wave sizes, and the riflemen try to attack in a line

### `tech_to_tanks`

Intent:

- standard play
- should have constant worker production until steel then oil are saturated
- should make defensive riflemen at first, then MGs for defense, and then mass tanks
- tanks attack the enemy in increasing waves

## Cross-Cutting Rules

- AI commands must be ordinary `Command` values.
- AI must not mutate game state directly.
- AI helper tests should prefer pure functions and stable sorted inputs.
- Do not rely on hash iteration order for command decisions.
- Keep profile fields inspectable. A profile should say "prefer this timing/composition", not
  redefine mining, building, production, and attack behavior.
- Keep self-play artifact quality. Replacing scripts is only an improvement if failures remain easy
  to inspect.
- Keep the current live AI behavior covered while migrating it.

## Exit Criteria for the First Refactor

The first AI architecture pass is complete when:

- the three required profiles run through the same shared decision loop
- live gameplay AI uses the shared world-model and action-synthesis layers
- self-play can run profile-vs-profile matchups without duplicating core RTS mechanics
- changing a tech requirement, unit cost, or supply rule usually means changing one shared helper
- replay determinism remains intact
- important old self-play scenarios are either still present or replaced by better matchup
  assertions
