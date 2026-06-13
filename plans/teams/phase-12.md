# Phase 12 - Replay, Branch, Sim-Wasm, Logs, and Match History

Status: planned.

## Goal

Preserve team identity and team outcomes through non-lobby runtime systems. Replays, replay branch
staging, local prediction fixtures, sim-wasm, structured logs, and match history should carry enough
team data to reconstruct and explain team matches.

## Scope

- Replay:
  - preserve player team ids and winner team id.
  - show correct team scores in playback game-over.
  - seeking retains team-aware start payloads, snapshots, and vision modes.
- Branch staging:
  - expose original seat team ids.
  - preserve or intentionally remap teams when users claim seats.
  - tests cover branch starts from a team replay.
- Local prediction and sim-wasm:
  - parse team fields.
  - keep prediction scoped to owned units.
- Match history and logs:
  - include team-aware score rows.
  - represent winner team without losing `winnerId` compatibility where needed.
  - avoid schema churn unless the current JSON fields are insufficient.
- Replay artifact compatibility:
  - old artifacts without team ids default to singleton-team FFA.
  - team artifacts missing required team fields fail clearly instead of replaying with wrong
    relationships.

## Expected Touch Points

- `docs/design/protocol.md`
- `docs/design/match-history.md`
- `docs/design/testing.md`
- `server/crates/sim/src/game/replay.rs`
- `server/crates/sim-wasm/src/lib.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/dev_replay.rs`
- `server/src/db.rs`
- `server/src/structured_log.rs`
- `client/src/replay_viewer.js`
- `client/src/replay_controls.js`
- `client/src/sim_wasm_adapter.js`
- `tests/team_integration.mjs`
- `tests/sim_wasm_smoke.mjs`
- replay/branch tests

## Verification

```bash
cd server && cargo test replay --workspace
node tests/team_integration.mjs
node tests/sim_wasm_smoke.mjs
node tests/client_contracts.mjs
```

Required automated scenarios:

- Replay artifact preserves player `teamId` and `winnerTeamId`.
- Old singleton-FFA replay artifacts still load through documented defaults.
- Replay seek preserves team-aware start payloads and snapshots.
- Replay vision modes respect team ids where relevant.
- Branch staging from a team replay preserves or explicitly validates team assignments.
- Sim-wasm parses current start payloads with team ids and prediction remains own-control-only.
- Match history score JSON includes per-player `teamId`; winner team representation is documented.

## Acceptance Criteria

- Team replays can be captured, played, sought, and branched without losing team identity.
- Match history and logs can explain team outcomes.
- Prediction and sim-wasm continue to parse current start payloads and remain own-control-only.
- Compatibility behavior for old artifacts and JSON fields is documented.

## Manual Testing Focus

Open one replay of a scripted team match and verify the score screen and vision mode labels look
reasonable. Do not manually replay full matches.

## Handoff Requirements

The phase handoff must list replay artifact/schema implications, branch behavior decisions,
sim-wasm compatibility, and match-history/logging decisions.
