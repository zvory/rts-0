# Phase 5 - Team Victory and Game-Over Semantics

Status: implemented.

## Goal

Make match resolution team-aware without mixing it into combat or fog work. A team should lose only
when every member is defeated, and final game-over messages should explain both player and team
outcomes.

## Scope

- Add team-aware alive/defeated helpers on the `Game` API or lobby-facing simulation seam.
- Team victory should replace per-player victory in team games:
  - singleton FFA remains behavior-compatible.
  - one-player sandbox remains never-ending.
  - a player losing all buildings should not receive a losing `gameOver` while any teammate keeps
    the team alive.
  - final `gameOver` should include `winnerTeamId`.
- Keep `winnerId` for FFA compatibility. For multi-player team wins, define whether `winnerId` is
  `null`, the first living winner by stable order, or another compatibility value, and document it in
  protocol docs.
- Score rows remain per-player and include `teamId`.
- Detached match-history recording should continue to receive per-player score rows; schema changes
  are deferred unless required for `winnerTeamId`.
- Branch live seat mapping must evaluate outcomes by original match seat/team, not by connection id.

## Expected Touch Points

- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `server/crates/sim/src/game/teams.rs`
- `server/crates/sim/src/game/mod.rs`
- `server/crates/sim/src/game/scoring.rs`
- `server/src/lobby/room_task.rs`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- `tests/team_integration.mjs`
- `tests/server_integration.mjs`

## Verification

```bash
cd server && cargo test team --workspace
node tests/team_integration.mjs
node tests/server_integration.mjs
node tests/protocol_parity.mjs
```

Required scenarios:

- Solo sandbox starts and does not resolve to a winner.
- A 2v2 does not end when one player on a team loses all buildings.
- A 2v2 ends when all players on one team are defeated.
- Defeated player on a living team does not receive early `gameOver`.
- Final team victory sends winning result to every connected teammate on the winning team.
- FFA still resolves with the same `winnerId` semantics as today.

## Acceptance Criteria

- Team victory and defeat are authoritative in the room task.
- Game-over payloads carry `winnerTeamId` and documented `winnerId` compatibility behavior.
- Score rows remain per-player and team-stamped.
- No shared economy, shared production, or shared command authority is introduced.

## Manual Testing Focus

Optional single-browser check of a scripted 2v2 AI setup only if automated team victory coverage is
ambiguous.

## Handoff Requirements

The phase handoff must describe winner-id compatibility decisions, team defeat tests, and any match
history implications deferred to the replay/history phase.

## Handoff

- `winnerTeamId` is the authoritative winner field for team games. `winnerId` remains populated
  for compatibility and is the first living player on the winning team in stable start/lobby order.
- Team defeat coverage now includes a 2v2 live integration scenario where one teammate gives up
  without receiving an early loss, then the match ends only after the opposing team is fully
  defeated. Rust coverage exercises `Game` alive-team helpers directly.
- Score rows remain per-player and already include `teamId`. Match-history schema is unchanged;
  `winner_name` continues to derive from compatibility `winnerId`, so richer team-winner history
  display remains deferred to the replay/history phase.
