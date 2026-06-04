# Phase 2 - AI Decision Decomposition

Goal: split `server/src/game/ai_core/decision.rs` into policy components without weakening AI
determinism or profile-driven behavior.

## Target Components

- `ai_core/decision/mod.rs`: `AiDecision`, `AiIntent`, `AiDecisionMemory`, and the top-level
  `decide_profile` orchestration.
- `ai_core/decision/policies.rs`: active policy selection for attack, production, worker,
  resource, recovery, and expansion policies.
- `ai_core/decision/defense.rs`: defensive panic detection, threat scoring, local defense target
  selection, and defensive-line assignment.
- `ai_core/decision/production.rs`: production building choice, unit priority counts, save-for-tech
  logic, and producer mapping.
- `ai_core/decision/resources.rs`: worker resource assignment counts, oil/steel goals, saturation,
  and resource-distance helpers.
- `ai_core/decision/expansion.rs`: expansion candidate discovery, scoring, anchor tiles, and City
  Centre placement.
- `ai_core/decision/proxy.rs`: proxy barracks decision, transit sites, worker selection, and proxy
  site scoring.
- `ai_core/decision/raids.rs`: rifle raid unit selection, target choice, and fallback movement.
- `ai_core/decision/geometry.rs`: local pure helpers for tile centers, distances, map clamping, and
  simple geometric scoring.

## Design Notes

Keep the profile model in `profiles.rs` and observation model in `observation.rs`; do not merge
them into decision modules. Decision modules should remain pure over `AiObservation`, `AiFacts`,
`AiProfile`, and memory, returning intents/commands rather than mutating game state.

Avoid splitting constants away from the behavior they tune unless they are used by multiple
decision modules. Constants like panic thresholds and raid radius should live with their owning
policy.

## Tests

- Move existing tests with the behavior they cover.
- Add focused tests for any helper whose visibility broadens during extraction.
- Run `cargo test` in `server/`.
- Run relevant self-play/profile matchup tests when policy code moves.

## Done

- `decision/mod.rs` reads as orchestration rather than a catalog of all AI behavior.
- Each extracted module has a single tactical concern.
- AI output for existing deterministic tests is unchanged.
