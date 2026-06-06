# Phase 6 - Minimal AI Team Awareness

Goal: make AI players team-safe without adding real coordination.

AI teammates should play independently for now. They need to avoid allies, use shared team vision,
and select enemy players as enemies.

## Observation Model

Update `server/src/game/ai_core/observation.rs`.

Add `team_id` to:

- `AiPlayerSummary`
- any test helper that constructs player summaries

Update live observation:

- `visible_enemies` excludes allied units/buildings.
- Player summaries include team ids and alive status.
- Shared fog from Phase 3 naturally broadens what an AI can see through teammates.

Update self-play snapshot observation similarly so tests and replay adapters remain consistent.

## Facts and Decisions

Update `server/src/game/ai_core/facts.rs` and `decision.rs`.

Required behavior:

- `nearest_public_enemy_base` ignores allied players.
- expansion safety/ranking uses enemy starts only.
- defensive panic only considers enemy visible entities.
- attack wave target selection chooses a living enemy player, not an ally.
- target tie-breaking remains deterministic.

Do not add:

- shared build orders,
- coordinated attacks,
- defense requests,
- resource donation,
- role assignment,
- team-level strategy memory.

## Live AI Adapter

Update `server/src/game/ai.rs`.

The live adapter should continue to instantiate one `AiController` per AI player. It should not
introduce a team controller.

## Self-Play and Replay

Default self-play remains FFA unless a test explicitly creates teams.

Update replay/player specs so `team_id` is preserved. A replay of a team game must reconstruct the
same team relationships.

## Files to Touch

- `DESIGN.md`
- `server/src/game/ai.rs`
- `server/src/game/ai_core/observation.rs`
- `server/src/game/ai_core/facts.rs`
- `server/src/game/ai_core/decision.rs`
- `server/src/game/selfplay.rs`
- `server/src/game/replay.rs`
- AI unit tests and self-play fixtures

## Tests

Add Rust tests:

- AI observation excludes allied combat units from `visible_enemies`.
- AI nearest public enemy base ignores allied starts.
- AI attack command targets an enemy start in a 2v2 setup.
- Replay preserves team ids.
- AI teammates do not attack each other during a live simulated match.

Run:

```bash
cd server && cargo test
```

## Acceptance Criteria

- AI is team-safe.
- AI remains strategically independent.
- Shared vision can inform AI because the authoritative fog grid is team-aware.
- Replay artifacts include enough team data to reproduce team matches.
