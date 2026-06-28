# Phase 2 - Client Replay Lobby UI

Status: Not started.

## Goal

Render replay staging rooms as group-watch lobbies with spectator slots only and no Ready/team
setup.

## Scope

- Update `Watch replay` so it joins the returned replay staging room through the lobby flow instead
  of redirecting into immediate playback.
- Teach the lobby browser to label replay lobby rows clearly and join them as spectators.
- Teach the joined lobby UI to hide Ready, team columns, faction controls, AI controls, map
  selection, and active-seat counts for replay lobby rooms.
- Show only spectator occupants in the joined replay lobby.
- Keep the host Start Match button visible and enabled whenever the server says `canStart`; hide or
  disable it for non-hosts as the normal lobby currently does.
- Preserve the existing replay join confirmation for rooms that are already in playback.

## Expected Touch Points

- `client/src/match_history.js`
- `client/src/app.js`
- `client/src/bootstrap.js`
- `client/src/lobby.js`
- `client/src/lobby_browser_view.js`
- `client/src/lobby_view.js`
- `client/index.html` if static control structure needs a small hook
- `tests/client_contracts/lobby_contracts.mjs`
- `docs/design/client-ui.md`

## Verification

- Client contract tests for replay browser row labels/actions and replay joined-lobby control
  visibility.
- `node scripts/check-client-architecture.mjs` if client module boundaries change.

## Manual Testing Focus

From the lobby, click Watch Replay, confirm the player lands in a replay lobby with only spectator
slots, no Ready button, and a host Start Match button. Open a second browser/client, join the replay
room from the lobby browser, then start playback and confirm both clients enter replay viewing.

## Handoff Expectations

Describe the UI state flags or metadata that distinguish replay lobbies, any copy chosen for the
browser row, and remaining integration coverage needed in Phase 3.
