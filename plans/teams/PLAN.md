# Team Games - Multi-Phase Plan

This plan adds team games while keeping the simulation server-authoritative and player-owned.
Short term UI support is limited to solo sandbox, FFA, 1v2, 1v3, and 2v2 on the current
four-start map. The backend model must be vector-based and must not bake in those shapes, so future
maps can support more than four players and arbitrary team sizes.

## Confirmed Decisions

- Default lobby mode is FFA.
- Short-run selectable shapes: current solo sandbox, FFA, 1v2, 1v3, and 2v2.
- Team defeat only happens when the whole team is defeated.
- Allies share current line of sight and explored history.
- Allied entity snapshots expose full details.
- Allied units are click-selectable for inspection only, not group-selectable, and never
  commandable through shared control.
- Right-clicking an allied unit should behave like right-clicking an own unit under the current
  command model. Today that means it is not treated as an attack target.
- Teammates should receive together-biased spawn positions.
- AI teammates should remain strategically independent for now. They only need team awareness:
  do not target allies, use shared vision, and pick enemy players as enemies.
- Host can add AI to any team.
- Resources, tech, supply, production, build requirements, and command authority remain strictly
  per-player.
- Score screens stay per-player, but every row indicates that player's team.

## Research Anchors

- StarCraft II team guidance keeps players independently owned while adding team victory,
  shared vision/control options, and resource-transfer features. Shared control is explicitly
  limited: it does not let a teammate spend another player's economy.
  Source: https://news.blizzard.com/en-us/article/4552959/game-guide-team-strategies
- Age of Empires II exposes team behavior as lobby options: lock teams, team-together starts,
  team positions, and shared exploration.
  Source: https://support.ageofempires.com/hc/en-us/articles/360047306372-How-do-I-create-a-multiplayer-match-in-Age-of-Empires-II-Definitive-Edition
- OpenRA models team/diplomacy as player relationship data, including `Team`,
  `IsAlliedWith`, and relationship filters such as Ally, Enemy, and Neutral.
  Source: https://docs.openra.net/en/release/lua/

## Core Model

The team feature is a relationship layer, not a new owner model.

- `Entity.owner` remains the owning player id. Neutral remains owner `0`.
- Every match player has a nonzero `teamId`.
- FFA is represented as one player per team.
- The server owns lobby team assignment and the match's final `playerId -> teamId` mapping.
- Client helpers should classify owners through `isOwn`, `isAlly`, `isEnemy`, and `isNeutral`.
  Do not keep adding `owner !== playerId` checks.
- Server helpers should classify players through one central relationship API. Combat, command
  validation, fog projection, AI observations, event delivery, and victory must all call that API.

## Short-Run Team Shapes

| Shape | Required seated players | Team sizes | Notes |
|-------|--------------------------|------------|-------|
| Solo sandbox | 1 | 1 | Current never-ending sandbox. |
| FFA | 1-4 | 1 per team | Default. One-player FFA is the sandbox. |
| 1v2 | 3 | 1, 2 | Host starts on Team 1 by default. |
| 1v3 | 4 | 1, 3 | Host starts on Team 1 by default. |
| 2v2 | 4 | 2, 2 | Team Together start assignment applies. |

The backend should also accept generic team assignments in lobby state, but the first UI should only
advertise the shapes above.

## Phases

- [Phase 0 - Data model and wire contract](PHASE_0.md)
- [Phase 1 - Lobby team assignment](PHASE_1.md)
- [Phase 2 - Team relationships, combat, and victory](PHASE_2.md)
- [Phase 3 - Shared vision and snapshot projection](PHASE_3.md)
- [Phase 4 - Client team interactions and score UI](PHASE_4.md)
- [Phase 5 - Team-together start positions](PHASE_5.md)
- [Phase 6 - Minimal AI team awareness](PHASE_6.md)
- [Phase 7 - Integration, hardening, and documentation audit](PHASE_7.md)

## Non-Negotiable Invariants

1. Protocol mirrors stay in sync: `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`,
   `client/src/protocol.js`, and `docs/design/protocol.md` must be changed together for every
   wire field.
2. Player economies stay isolated. No team resource pool, no shared tech, no shared supply, no
   allied worker building, and no allied production commands.
3. Allies are not enemies. Raw `attack` commands, auto-acquisition, overpenetration, target tracers,
   worker retreat, AI observations, and score kill credit must all respect team relationships.
4. Shared vision is authoritative. If one teammate can currently see an enemy, all teammates may
   receive that enemy in snapshots. If no teammate can see it, none may receive it.
5. Shared explored history is client-visible state. The client should accumulate explored tiles from
   all allied vision sources it receives over time.
6. Team defeat is the only team-game terminal condition. Individual players may be economically dead
   while their team continues.
7. No fixed-size team arrays. Store players, teams, and assignments in `Vec`/maps. The current lobby
   may cap at four because the current map has four starts, but the simulation model must not.
8. FFA behavior must remain compatible with today's player-vs-player behavior.

## Suggested Implementation Order

Implement one phase at a time. Do not skip Phase 0: adding team behavior without a stable relationship
contract will scatter owner comparisons through the codebase and make later fixes fragile.

Each phase should leave the repo in a working state. Where a phase changes behavior, add the tests in
that same phase. If a phase updates `docs/design/*.md`, update it in the same commit as the code.
