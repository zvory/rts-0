# Phase 1 - Team Identity and Relationship Contract

Status: implemented.

## Goal

Add canonical team identity and relationship helpers while preserving today's FFA gameplay. This
phase is contract work: teams become visible in data, but combat, fog, starts, and victory should
continue to behave as one-player-per-team FFA.

## Scope

- Add `TeamId = u32` or equivalent in the simulation and make `teamId` nonzero for match players.
- Add `team_id` to `PlayerInit` and `PlayerState`.
- Add relationship helpers such as `team_of_player`, `same_team_player`, `same_team_owner`,
  `is_enemy_player`, `is_enemy_owner`, and `allied_player_ids`.
- Add or update fixture constructors so hand-built Rust and JS tests can default missing team fields
  to singleton-team FFA without repeated boilerplate.
- Thread `teamId` through:
  - `LobbyPlayer`
  - `PlayerStart`
  - `PlayerScore`
  - replay player specs and fixtures
  - branch staging seat metadata, or explicitly reject team replay branching until the replay phase
  - `gameOver` as `winnerTeamId: u32 | null`, while keeping `winnerId` for FFA compatibility
- Add `GameState` helpers:
  - `playerById(id)`
  - `teamIdForPlayer(id)`
  - `isOwnOwner(owner)`
  - `isAllyOwner(owner)`
  - `isEnemyOwner(owner)`
  - `isNeutralOwner(owner)`
- Keep every current seated player on a unique team by default.
- Update docs for the new wire fields and relationship layer.

## Expected Touch Points

- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/client-ui.md`
- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `server/crates/sim/src/game/mod.rs`
- new or updated `server/crates/sim/src/game/teams.rs`
- `server/crates/sim/src/game/replay.rs`
- `server/crates/sim-wasm/src/lib.rs`
- `server/src/lobby/room_task.rs`
- `client/src/protocol.js`
- `client/src/state.js`
- tests and fixtures that construct player/start/score/replay payloads

## Verification

Run focused contract tests:

```bash
cd server && cargo test team --workspace
cd server && cargo test protocol --workspace
node tests/protocol_parity.mjs
node tests/client_contracts.mjs
node tests/team_integration.mjs
```

Use narrower Rust package selectors if the final touched files make a full workspace command too
broad during development, but the phase commit should pass the contract-focused coverage above.

## Acceptance Criteria

- Lobby/start/score/game-over payloads carry team identity.
- Replay and sim-wasm fixtures parse and emit the new fields.
- Old or hand-built fixtures without explicit team fields have documented singleton-FFA defaults at
  every deserialization/test-helper boundary.
- FFA defaults assign one unique nonzero team per seated player.
- Relationship helper tests prove singleton FFA players are enemies and neutral owner `0` is never
  allied with a player.
- Branch/replay structures either carry team ids immediately or fail clearly for team-specific
  artifacts until the replay phase completes.
- No combat, fog, start-location, or victory behavior changes are introduced yet.

## Manual Testing Focus

Start one ordinary FFA match from the lobby and confirm the lobby/start/score UI still appears normal.

## Handoff Requirements

The phase handoff must call out every protocol field added, any compatibility defaults used for old
fixtures, and which tests prove FFA behavior is unchanged.
