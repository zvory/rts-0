# Phase 0 - Data Model and Wire Contract

Goal: add the canonical team fields and helper seams while preserving current FFA behavior.

This phase should not change combat, fog, start positions, or lobby visuals beyond carrying and
displaying a team id. Existing two-player games should still behave as FFA with one player per team.

## Server Protocol

Update `server/src/protocol.rs` and mirror in `client/src/protocol.js`.

Add team identity to:

- `LobbyPlayer`: `teamId: u32`
- `PlayerStart`: `teamId: u32`
- `PlayerScore`: `teamId: u32`
- `gameOver`: add `winnerTeamId: u32 | null`

Keep `winnerId` for now for singleton-team FFA compatibility. In team games where multiple players
can win together, `winnerId` may be `null` and `winnerTeamId` is authoritative.

Add lobby commands:

- `setTeamPreset`: host-only, lobby-only, fields `{ preset: string }`
- `setTeam`: host-only, lobby-only, fields `{ id: u32, teamId: u32 }`
- Extend `addAi` with optional `teamId: u32`

Use nonzero team ids for players. Team id `0` is invalid for match players and must be rejected or
normalized by lobby code. Entity owner `0` remains neutral and is unrelated to team ids.

## Game Data Model

Add a reusable team type and helpers. A small `server/src/game/teams.rs` module is preferred over
scattered methods.

Required concepts:

- `TeamId = u32`
- `PlayerInit.team_id`
- `PlayerState.team_id`
- `TeamIndex` or equivalent lookup built from current match players
- `team_of(player_id) -> Option<TeamId>`
- `same_team(a, b) -> bool`
- `is_enemy_player(a, b) -> bool`
- `teams_in_lobby_order(players) -> Vec<TeamId>`

Do not encode `1v2`, `1v3`, or `2v2` in the simulation. Those are lobby presets only.

## Client Data Model

Add team helpers to `client/src/state.js`:

- `playerById(id)`
- `teamIdForPlayer(id)`
- `isOwnOwner(owner)`
- `isAllyOwner(owner)`
- `isEnemyOwner(owner)`
- `isNeutralOwner(owner)`

All later client phases should use these helpers instead of comparing directly to `state.playerId`.

## FFA Defaults

For current behavior, every seated player gets a unique team id by default.

Recommended default assignment:

- First seated player: Team 1
- Second seated player: Team 2
- Continue in lobby/start order

The exact team id values only need to be stable within a match and visible in lobby/start/score
payloads.

## Files to Touch

- `docs/design/*.md`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- `server/src/game/mod.rs`
- new `server/src/game/teams.rs`
- `client/src/state.js`
- test fixtures that construct `PlayerInit`, `PlayerStart`, `LobbyPlayer`, or `PlayerScore`

## Tests

Add focused tests before broad integration work:

- Rust protocol serialization includes `teamId` on lobby/start/score payloads.
- `PlayerInit` fixtures still build cleanly after adding `team_id`.
- `TeamIndex` treats singleton FFA players as enemies.
- Client contract tests confirm `GameState` exposes team helper methods.

Run:

```bash
cd server && cargo test
node tests/client_contracts.mjs
```

## Acceptance Criteria

- Existing FFA games still start and resolve.
- Every lobby row, start player, and score row carries `teamId`.
- `winnerTeamId` exists in `gameOver` payloads.
- No combat/fog behavior changes are introduced in this phase.
- No fixed-size team storage is introduced.
