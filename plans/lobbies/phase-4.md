# Phase 4 - Freshness, Hardening, and Docs

Status: planned.

## Goal

Polish the lobby browser into a durable production flow by improving freshness, expanding coverage,
and updating design/protocol documentation.

## Scope

- Add WebSocket push or invalidation for lobby-browser changes if it fits cleanly with the Phase 1
  server seams.
- Keep 1-2 second polling as a fallback, even if push is added.
- Ensure list updates happen after room creation, host changes, map changes, AI add/remove, active
  slot changes, spectator changes, countdown start, match start, match end/reset, and room empty
  cleanup.
- Harden stale row handling so the browser never keeps an enabled action for a room that the latest
  server state marks in-game or gone.
- Add or update tests for server summary state transitions, client browser rendering, create/join
  behavior, and responsive smoke coverage.
- Add a guardrail or focused assertion that the public browser only exposes normal-room summaries.
  This can live in lobby Rust tests, `scripts/check-lobby-architecture.mjs`, or both if the final
  implementation creates a stable helper worth enforcing.
- Update the relevant design docs and context capsules.
- Run focused verification and let the final phase commit hook provide broad coverage before merge.

## WebSocket Freshness Option

If push is added, prefer a small invalidation or summary message over coupling unjoined clients to
room internals:

- The client subscribes to lobby-browser updates while the lobby screen is visible.
- The server sends either a full `lobbyList` payload or a cheap `lobbyListDirty` signal.
- If using dirty signals, the client immediately refetches `GET /api/lobbies`.
- Polling stays active at a slower fallback cadence or resumes after missed push/connection close.
- Joined clients do not need browser updates unless the UI exposes the browser while joined.

If push adds too much protocol churn for the benefit, keep the HTTP poller and document that the
accepted freshness target is the configured 1-2 second interval.

## Hardening Requirements

- Names are validated consistently in client preflight and server authority, with server authority
  winning.
- Internal room prefixes remain hidden and uncreatable through the browser.
- Lab, replay artifact, persisted replay, replay branch, and dev scenario rooms remain hidden even
  though they are now all room-hosted sessions under the same policy shell.
- Summary collection cannot wedge on a slow room task.
- Browser polling stops on teardown and does not leak intervals across app lifecycle changes.
- Relative-age timers do not keep running after `Lobby.destroy()`.
- Disabled rows are not focusable as actions.
- Full waiting rows stay spectator-joinable after refresh.
- In-progress rows remain visible after match start and become joinable again only if the room
  returns to a waiting lobby.

## Documentation Updates

- Update `docs/design/protocol.md` for any WebSocket create/list/push messages.
- Update the relevant server/client design docs for `GET /api/lobbies` if the summary list is HTTP.
- Update `docs/context/protocol.md` if protocol section lists or lobby fields change.
- Update `docs/context/client-ui.md` if a new lobby browser module becomes a stable UI seam.
- Update `docs/context/server-sim.md` if the summary/create helper becomes a stable lobby seam or
  adds a new `RoomEvent`.
- Update testing docs only if a new dedicated lobby-browser suite is added.

## Touch Points

- `server/src/lobby/mod.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/session_policy.rs`
- `server/src/main.rs`
- `server/crates/protocol/src/lib.rs` if push/list messages are WebSocket
- `server/src/protocol.rs` if protocol adapters are affected
- `client/src/protocol.js` if WebSocket protocol changes
- `client/src/net.js`
- `client/src/lobby.js`
- `client/src/lobby_browser_view.js` or equivalent
- `client/index.html`
- `client/styles.css`
- `tests/*.mjs`
- `docs/design/*.md`
- `docs/context/*.md`

## Verification

- Run focused Rust tests for lobby summary state transitions.
- Run `node scripts/check-lobby-architecture.mjs` if lobby policy guardrails or helper boundaries
  change.
- Run `node tests/protocol_parity.mjs` if protocol changed.
- Run `node tests/client_contracts.mjs`.
- Run `node scripts/check-client-architecture.mjs`.
- Run the focused live Node lobby/team integration suite that covers multi-client joins, full rooms,
  spectator joins, and match start.
- Run client smoke after the UI is stable enough for screenshot-level behavior.
- Use `node tests/select-suites.mjs --from=<base-ref>` to confirm the final expected suite set for
  the changed files.

## Manual Testing Focus

- Open two browser tabs and confirm a lobby created in one appears in the other within the freshness
  target.
- Change the map as host and confirm the browser row updates.
- Add AI until the room is full and confirm the row stays visible and spectator-joinable.
- Start the match and confirm the row becomes muted and disabled.
- Let the room return to lobby or empty out and confirm the browser updates correctly.
- Test desktop and mobile widths for row/card alignment, disabled row affordance, and modal focus.

## Handoff

Mark this phase done in the implementation commit. Summarize whether push was added or polling
remains the accepted freshness mechanism, list the final verification commands and results, and
call out any product behavior that should be watched in beta playtests.
