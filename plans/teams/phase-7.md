# Phase 7 - AI, Replay, Branch, and Match-History Coverage

Status: planned.

## Goal

Carry team identity and team relationships through non-lobby runtime systems. AI should be team-safe,
and replay, branch staging, local prediction fixtures, logs, and match history should preserve enough
team data to reconstruct and explain team matches.

## Scope

- AI observation:
  - include `teamId` in player summaries
  - treat allied visible entities as allies, not visible enemies
  - use shared team fog from Phase 4
- AI decisions:
  - nearest public enemy base ignores allied starts
  - expansion safety uses enemy starts only
  - defense/panic logic considers enemy visible entities only
  - attack waves choose living enemy players
  - no shared strategy, shared build order, resource donation, or team controller
- Replay:
  - preserve player team ids and winner team id
  - show correct team scores in playback game-over
  - seeking and branch starts retain team relationships
- Branch staging:
  - expose original seat team ids
  - preserve or intentionally remap teams when users claim seats
  - tests cover branch starts from a team replay
- Match history and logs:
  - include team-aware score rows
  - represent winner team without losing `winnerId` compatibility where needed
  - avoid schema churn unless the current JSON fields are insufficient
- Local prediction and sim-wasm:
  - parse team fields
  - keep prediction scoped to owned units

## Expected Touch Points

- `docs/design/ai.md`
- `docs/design/match-history.md`
- `docs/design/testing.md`
- `server/crates/ai/src/`
- `server/crates/sim/src/game/replay.rs`
- `server/crates/sim-wasm/src/lib.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/dev_replay.rs`
- `server/src/db.rs`
- `server/src/structured_log.rs`
- `client/src/replay_viewer.js`
- `client/src/replay_controls.js`
- `tests/team_integration.mjs`
- `tests/ai_integration.mjs`
- AI self-play/replay tests

## Verification

```bash
cd server && cargo test -p rts-ai
cd server && cargo test replay --workspace
node tests/ai_integration.mjs
node tests/team_integration.mjs
node tests/sim_wasm_smoke.mjs
```

Required automated scenarios:

- AI observation excludes allied units from `visible_enemies`.
- AI nearest enemy base ignores allied starts.
- AI attack target selection picks an enemy player in 2v2.
- Live AI teammates do not attack each other during a scripted match.
- Replay artifact preserves player `teamId` and `winnerTeamId`.
- Replay seek preserves team-aware start payloads and snapshots.
- Branch staging from a team replay preserves or explicitly validates team assignments.
- Match history score JSON includes per-player `teamId`.

## Acceptance Criteria

- AI is team-safe and strategically independent.
- Team replays can be captured, played, sought, and branched without losing team identity.
- Match history and logs can explain team outcomes.
- Prediction and sim-wasm continue to parse current start payloads and remain own-control only.

## Manual Testing Focus

Open one replay of a scripted team match and verify the score screen and vision mode labels look
reasonable. Do not manually replay full matches.

## Handoff Requirements

The phase handoff must list replay artifact/schema implications, AI behavior covered by tests, and
any match-history compatibility decisions.
