# Phase 7 - Integration, Hardening, and Documentation Audit

Goal: make team games robust end to end and leave the architecture documented.

This phase is not a refactor bucket. It is for final cross-module verification, malicious-input
coverage, and contract cleanup after Phases 0-6.

## Integration Coverage

Add a dedicated team integration script if the existing scripts become too crowded.

Suggested file:

- `tests/team_integration.mjs`

Cover:

- 1v2 with one human plus two AIs on the opposing team starts from lobby and resolves.
- 1v3 with one human plus three AIs starts from lobby.
- 2v2 with host adding AIs to specific teams starts from lobby.
- FFA still starts and reports singleton teams.
- A defeated player on a living team does not receive immediate `gameOver`.
- A defeated team receives `gameOver`.
- Score payload rows include `teamId`; game-over includes `winnerTeamId`.

## Hardening Coverage

Add or extend regression tests:

- Non-host cannot change preset, move teams, or add AI to a team.
- `setTeam` with team id `0` is rejected or normalized safely.
- `setTeam` for an unknown player id is ignored.
- `addAi` with invalid team id is ignored or assigned safely.
- Raw `attack` command against an ally is ignored.
- Compact snapshots do not leak hidden enemy entities through allied target tracers.
- A disconnected player is eliminated; the team only loses if no teammate remains alive.

## Manual Browser Check

After implementation, run the server and inspect:

- Lobby FFA.
- Lobby 1v2.
- Lobby 1v3.
- Lobby 2v2.
- In-game allied unit click inspection.
- In-game right-click on allied unit.
- Score screen team column.

Use the Browser plugin for normal local frontend verification unless debugging self-play replay
failures, where the project instructions require macOS `open`.

## Documentation Audit

Update `DESIGN.md` in the same implementation changes that alter contracts.

Sections that must mention teams:

- High-level architecture and fog summary.
- Wire protocol: new lobby commands, new fields, game-over winner team.
- `Game` public API: `PlayerInit.team_id`, team-aware start payloads, team victory.
- Lobby concurrency/model: team preset and team assignment.
- Rules/projection: allies and shared vision.
- Client module contracts: state helpers, fog over allied entities, lobby team UI.
- AI opponents: minimal team awareness.
- Invariants: team relationship layer, isolated economy, no shared control.

## Final Test Run

Run the broad suites once implementation is complete:

```bash
cd server && cargo fmt && cargo clippy && cargo test
node tests/server_integration.mjs
node tests/regression.mjs
node tests/ai_integration.mjs
node tests/client_contracts.mjs
cd tests && npm install && node client_smoke.mjs
```

If `tests/run-all.sh` has been kept current with new team tests, it can be used as the broad
verification wrapper.

## Acceptance Criteria

- Every supported short-run shape can be configured from the lobby.
- FFA remains the default and still works.
- Team victory, shared vision, allied inspection, and per-player score rows all work together.
- Malicious clients cannot attack allies or mutate teams without host authority.
- `DESIGN.md` accurately describes the final implementation.
- No implementation depends on a fixed number of teams or fixed team sizes.
