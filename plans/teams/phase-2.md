# Phase 2 - Scriptable Lobby Team Setup

Status: planned.

## Goal

Make team setup fully scriptable from tests while keeping non-FFA team games gated from normal UI
exposure. This phase should let automation create solo sandbox, FFA, 1v2, 1v3, and 2v2 rooms
without manual browser setup.

## Scope

- Add host-only lobby commands:
  - `setTeamPreset { preset }`
  - `setTeam { id, teamId }`
  - `addAi { teamId? }`
- Store team preset and per-seat team assignment for human players and AI slots.
- Supported short-run presets:
  - `solo`: exactly one active non-spectator player on Team 1, no forced AIs, existing sandbox
    never-ending outcome behavior.
  - `ffa`: one unique nonzero team per active player, default.
  - `1v2`: host Team 1, two seats Team 2.
  - `1v3`: host Team 1, three seats Team 2.
  - `2v2`: two seats Team 1, two seats Team 2.
- Keep `ffa` as the default for ordinary rooms.
- Reassign current seats deterministically when the host changes preset, keeping the host on Team 1
  when possible.
- Validate preset capacity, team sizes, nonzero team ids, unknown player ids, and spectator
  exclusion before start.
- Expose enough protocol helpers that Node integration tests can configure teams directly through
  WebSocket messages.
- Do not add normal user-facing lobby controls yet. If a temporary test/dev control is needed, keep
  it clearly gated and document the gate.
- Do not assert teammate start proximity yet; start assignment remains current player-order behavior
  until the start phase.

## Expected Touch Points

- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/design/ai.md`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `server/src/lobby/room_task.rs`
- `client/src/protocol.js`
- `client/src/net.js`
- `tests/team_integration.mjs`
- `tests/ai_integration.mjs`
- `tests/server_integration.mjs`

## Verification

Automated coverage should avoid manual tab workflows:

```bash
node tests/team_integration.mjs
node tests/ai_integration.mjs
node tests/server_integration.mjs
node tests/client_contracts.mjs
```

Required scenarios for `tests/team_integration.mjs`:

- Default room reports FFA and unique teams.
- Solo preset starts with one active player on Team 1 and does not force an AI opponent.
- Host can configure 1v2 with two AIs on Team 2 and start.
- Host can configure 1v3 with three AIs on Team 2 and start.
- Host can configure 2v2 with one human plus AIs filling open seats and start.
- Non-host `setTeamPreset`, `setTeam`, and `addAi(teamId)` are rejected or ignored.
- Invalid team id `0`, unknown player ids, spectator assignments, and overfull preset moves are
  rejected or ignored.
- Invalid preset composition leaves `canStart` false.

## Acceptance Criteria

- Team setup is possible through WebSocket tests.
- Every lobby row reports `teamId`.
- Every valid preset can start through automation.
- Invalid preset/team mutations are covered by regression-style tests.
- Non-FFA team presets are not normally user-facing yet.
- In-game behavior remains FFA-like until later phases make relationships authoritative.

## Manual Testing Focus

None expected beyond confirming any temporary test/dev gate is not visible in ordinary lobby use.

## Handoff Requirements

The phase handoff must describe how to script each preset in tests, list the new lobby commands,
document solo semantics, and call out that user-facing lobby controls are intentionally deferred.
