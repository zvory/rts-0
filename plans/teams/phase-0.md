# Phase 0 - Automated Team Harness Foundation

Status: planned.

## Goal

Create reusable automated test support for team-game work before changing gameplay behavior. This
phase should make future phases cheap to verify by script instead of by manually opening three or
four browser tabs.

## Scope

- Add shared Node test helpers for multi-client room setup, host/non-host actions, AI seating,
  readiness, match start, snapshot waits, command sends, and game-over waits.
- Add assertions for stable lobby/start/score protocol fields that can be extended with `teamId` in
  Phase 1.
- Add a placeholder or initial `tests/team_integration.mjs` that verifies today's FFA-compatible
  baseline and documents future scenario slots.
- Add small Rust/JS fixture helpers or identify the right helper locations for constructing
  `PlayerInit`, `PlayerStart`, `PlayerScore`, replay player specs, branch seats, and start payloads
  once Phase 1 adds team fields.
- Update `tests/select-suites.mjs` only if needed so future teams-plan files and team-related code
  select the new team integration suite.

## Expected Touch Points

- `tests/server_integration.mjs`
- `tests/ai_integration.mjs`
- new or updated `tests/team_integration.mjs`
- test utility code under `tests/` if existing scripts have duplication worth extracting
- `tests/select-suites.mjs`
- Rust test helper modules near the sim/protocol tests, if a helper is useful before Phase 1

## Verification

Run the smallest relevant suite after adding the harness:

```bash
node tests/team_integration.mjs
node tests/server_integration.mjs
node tests/ai_integration.mjs
node tests/select-suites.mjs --verify
```

If the new suite requires a live server, start only the local server needed by that suite and document
the port behavior in the test file.

## Acceptance Criteria

- A scripted test can create a room, connect multiple clients, add AIs, ready humans, start a match,
  and observe start/snapshot/game-over messages without manual browser interaction.
- The helper API is general enough for 1v2, 1v3, and 2v2 scenarios once team commands exist.
- The team integration suite states whether it starts its own server or requires an already-running
  server, and documents the port behavior.
- Existing FFA, AI seating, and server integration behavior still pass.
- Future phases can add team assertions without copying WebSocket orchestration into every test.

## Manual Testing Focus

None expected beyond confirming the new test command runs locally if environment setup changed.

## Handoff Requirements

The phase handoff must name the new helper functions or suite entry points, explain how later phases
should add team scenarios, and list the exact automated commands that passed.
