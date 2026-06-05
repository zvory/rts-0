# Phase 2 - Team Relationships, Combat, and Victory

Goal: make the simulation understand allies and team victory.

This phase makes teams real in the authoritative game state, but it does not yet implement shared
vision. Allies should stop attacking each other, and game-over should resolve by team.

## Relationship API

Build one central relationship surface and use it everywhere.

Recommended server shape:

- `TeamIndex` built from `PlayerState`
- `same_team_player(a, b)`
- `same_team_owner(owner_a, owner_b)` where owner `0` is neutral and never allied
- `is_enemy_owner(attacker_owner, target_owner)`
- `team_alive(team_id, entities, players)`
- `alive_teams(entities, players)`

Do not let services compare `owner != player` for hostility after this phase.

## Command Validation

Update `server/src/game/services/commands.rs`.

- Raw `attack` commands must reject allied targets.
- Move, gather, build, train, cancel, and stop remain own-only.
- No command may target or control allied units except by treating their position as an ordinary
  clicked world point on the client.

## Combat Targeting

Update:

- `server/src/game/services/world_query.rs`
- `server/src/game/services/combat.rs`

Required changes:

- Auto-acquisition ignores allied units and buildings.
- Ordered attacks lose their target if the target is allied.
- AT-team tank preference ignores allied tanks.
- Overpenetration cannot damage allies.
- Worker direct-hit retreat only reacts to enemy damage.
- Last-damage owner and kill credit should only record enemy damage.

Friendly fire is not part of this phase.

## Victory and Elimination

Update `Game` and `RoomTask`.

Rules:

- A team is alive when at least one player on the team is alive under the existing player-alive
  rules.
- Human players are still individually alive if they have at least one building.
- AI players keep the existing special rule: AI also needs at least one unit.
- A team game ends when zero or one teams remain alive.
- A team member who has lost all buildings does not receive a game-over screen while any teammate is
  still keeping the team alive.
- If a team is eliminated while another team remains, all connected humans on that team receive a
  losing `gameOver`.
- Final victory sends all remaining connected humans on the winning team `you: "won"` and all
  others `you: "lost"`.
- FFA remains naturally compatible because every team has one player.

`gameOver` should include:

- `winnerTeamId`
- `winnerId` only when exactly one winning player is appropriate
- per-player scores with `teamId`

## Scores

Keep score rows per player. Add `teamId` to each row.

Do not aggregate team scores in this phase.

## Files to Touch

- `DESIGN.md`
- `server/src/game/teams.rs`
- `server/src/game/mod.rs`
- `server/src/game/services/commands.rs`
- `server/src/game/services/world_query.rs`
- `server/src/game/services/combat.rs`
- `server/src/game/services/death.rs`
- `server/src/lobby.rs`
- tests under `server/src/game/*`
- `tests/regression.mjs`

## Tests

Add Rust tests:

- Two allied riflemen near each other do not auto-acquire.
- Raw attack command against an ally is ignored.
- Overpenetration through an enemy does not damage an allied unit behind it.
- A 2v2 game does not end when one player on a team loses all buildings.
- A 2v2 game ends when all players on one team are defeated.
- FFA still sends individual losing game-over when a singleton team is eliminated in a 3-4 player
  FFA match.

Add regression coverage:

- Malicious client cannot attack allied entity ids.

Run:

```bash
cd server && cargo test
node tests/regression.mjs
```

## Acceptance Criteria

- All hostile checks are team-aware.
- Team victory works for singleton FFA and multi-player teams.
- No shared economy, tech, or control behavior is introduced.
- Existing FFA behavior remains compatible.
