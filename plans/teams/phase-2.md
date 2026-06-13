# Phase 2 - Lobby Presets and Scripted Team Setup

Status: planned.

## Goal

Make teams configurable in the lobby and fully scriptable from tests. This phase should let tests
create solo sandbox, FFA, 1v2, 1v3, and 2v2 rooms without manual browser setup.

## Scope

- Add host-only lobby commands:
  - `setTeamPreset { preset }`
  - `setTeam { id, teamId }`
  - `addAi { teamId? }`
- Store team preset and per-seat team assignment for human players and AI slots.
- Supported short-run presets:
  - `solo` or equivalent one-player sandbox mode
  - `ffa`
  - `1v2`
  - `1v3`
  - `2v2`
- Keep `ffa` as the default.
- Reassign current seats deterministically when the host changes preset.
- Keep host on Team 1 when possible.
- Validate preset capacity and team sizes before start.
- Add a compact lobby UI for presets, grouped team rows, and host-only AI/team controls.
- Expose enough test hooks or protocol helpers that Node integration tests can configure teams
  directly through WebSocket messages.

## Expected Touch Points

- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/design/ai.md`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `server/src/lobby/room_task.rs`
- `client/src/protocol.js`
- `client/src/net.js`
- `client/src/lobby.js`
- `client/index.html`
- `client/styles.css`
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
- Host can configure 1v2 with two AIs on Team 2 and start.
- Host can configure 1v3 with three AIs on Team 2 and start.
- Host can configure 2v2 with one human plus AIs filling open seats and start.
- Non-host `setTeamPreset`, `setTeam`, and `addAi(teamId)` are rejected or ignored.
- Invalid team id `0`, unknown player ids, and overfull preset moves are rejected or ignored.
- Invalid preset composition leaves `canStart` false.

## Acceptance Criteria

- Team setup is possible from both UI and WebSocket tests.
- Every lobby row reports `teamId`.
- Every valid preset can start through automation.
- Invalid preset/team mutations are covered by regression-style tests.
- In-game behavior remains FFA-like until later phases make relationships authoritative.

## Manual Testing Focus

Use one browser tab to confirm the host can see preset controls and grouped team rows. Do not require
manual multi-tab validation for this phase.

## Handoff Requirements

The phase handoff must describe how to script each preset in tests, list the new lobby commands, and
name any UI behavior intentionally left for later client polishing.
