# Phase 11 - AI Team Safety

Status: implemented.

## Goal

Carry team relationships into AI observation and decision logic without adding shared AI strategy.
AI players should remain strategically independent but must classify allies separately from enemies.

## Scope

- AI observation:
  - include `teamId` in player summaries.
  - treat allied visible entities as allies, not visible enemies.
  - use shared team fog from Phase 6.
- AI decisions:
  - nearest public enemy base ignores allied starts.
  - expansion safety uses enemy starts only.
  - defense/panic logic considers enemy visible entities only.
  - attack waves choose living enemy players.
  - live AI teammates do not attack each other during scripted matches.
- Do not add shared strategy, shared build order, resource donation, or a team controller.
- Self-play fixtures that build explicit players should use the Phase 1 team fixture helpers.

## Expected Touch Points

- `docs/design/ai.md`
- `docs/design/testing.md`
- `server/crates/ai/src/`
- `server/crates/sim/src/game/replay.rs`
- `server/src/lobby/room_task.rs`
- `tests/team_integration.mjs`
- `tests/ai_integration.mjs`
- AI self-play/replay tests

## Verification

```bash
cd server && cargo test -p rts-ai
node tests/ai_integration.mjs
node tests/team_integration.mjs
```

Use `RTS_FULL_AI_TESTS=1 cargo test` only if the AI behavior touched depends on long self-play or
balance outcomes.

Required automated scenarios:

- AI observation excludes allied units from `visible_enemies`.
- AI nearest enemy base ignores allied starts.
- AI attack target selection picks an enemy player in 2v2.
- Live AI teammates do not attack each other during a scripted match.
- AI strategy remains per-player and does not share economy, production, or command authority.

## Acceptance Criteria

- AI is team-safe and strategically independent.
- AI-visible relationship semantics match server relationship helpers.
- Team integration can run AI-filled 1v2, 1v3, and 2v2 setup without allied AI attacks.

## Manual Testing Focus

None expected unless an AI self-play failure needs visual replay inspection under the repo's
self-play failure protocol.

## Handoff Requirements

The phase handoff must list AI behavior covered by tests, say whether long AI coverage was needed,
and call out any strategic limitations intentionally left unchanged.
