# Phase 3 - Create and Join Flows

Status: done.

## Goal

Make the lobby browser fully usable: users create named lobbies in a modal, join open rows as active
players, join full waiting rows as spectators, and no longer use a manual room-name join field.

## Scope

- Wire `Create Lobby` to an accessible modal where the player chooses the lobby name.
- Validate the lobby name client-side for immediate feedback and server-side for authority.
- Submit create through the atomic server API from Phase 1.
- On successful create, join the created room and show the existing joined-lobby UI.
- Wire row actions:
  - `Join lobby` sends a normal active join for open waiting rows.
  - `Join as spectator` sends a spectator join for full waiting rows.
  - in-game rows stay disabled.
  - countdown/starting rows should follow the server `joinState` and be disabled if the room rejects
    new joins during countdown.
- Persist the edited player name before create or join, using the existing name storage behavior.
- Remove visible room-name joining from `client/index.html` and `client/src/lobby.js`.
- Update the pinned DOM-contract comment and nearby client tests so the normal pre-join path no
  longer promises `#lobby-room`/`#lobby-join` as visible product controls.
- Refresh the lobby list immediately after create failure, join rejection, stale row detection, and
  successful leave/return-to-lobby flows where appropriate.

## Modal Requirements

- Open from `Create Lobby` in the side panel and from the empty state.
- Move focus into the lobby name input on open.
- Close on Escape, backdrop click if consistent with existing modals, and Cancel.
- Trap Tab within the modal while open.
- Return focus to the triggering button on close.
- Disable submit while the name is invalid or a create request is in flight.
- Show server errors inline:
  - name already exists
  - invalid or reserved name
  - server is draining
  - network disconnected
- Keep the modal compact. It should not contain feature explanation copy.

## Join Behavior Details

- Open waiting row: call the existing join path with `spectator=false`.
- Full waiting row: call the existing join path with `spectator=true`.
- In-game row: no join request from the browser UI.
- Stale row: if a row click fails because the server rejects or the room state changed, show a
  concise status line, refresh immediately, and keep the user on the browser.
- Replay rooms should not appear in the browser. The existing replay prompt remains for explicit
  replay join paths outside this browser.
- The default name remains usable. A player can click a row without editing the name and joins as
  `Commander` or their saved name.

## Touch Points

- `client/index.html`
- `client/styles.css`
- `client/src/lobby.js`
- `client/src/lobby_browser_view.js` or equivalent
- `client/src/net.js`
- `client/src/protocol.js` if create uses WebSocket
- `server/crates/protocol/src/lib.rs` and `server/src/protocol.rs` if create uses WebSocket
- `server/src/main.rs` and `server/src/lobby/mod.rs` for create errors if not finished in Phase 1
- Client smoke and integration tests that cover create/join behavior

## Constraints

- Do not keep the old room-name field hidden-but-focusable. If compatibility markup remains for
  tests, it must be inaccessible from the normal product flow.
- Do not remove lower-level join-by-room protocol support that tests, direct URLs, replay prompts,
  or dev flows still use. This phase removes the normal product UI affordance, not the underlying
  server join capability.
- Do not let `Create Lobby` race into joining an existing room. Duplicate create is an error.
- Do not make users choose active vs spectator manually. Full waiting rows choose spectator
  automatically; open waiting rows choose active automatically.
- Do not disable full waiting rooms just because active seats are full.
- Do not allow the browser UI to join in-game rows even if the lower-level protocol would reject
  them anyway.
- Keep the joined-lobby host tools, map selector, AI controls, and ready/start behavior unchanged
  after join.

## Verification

- Add focused tests for:
  - create modal validation and focus behavior
  - duplicate create displays an inline error
  - open row sends active join
  - full row sends spectator join
  - in-game row action is disabled and sends no join
  - manual room-name input is absent from the normal pre-join DOM path
- Run `node tests/client_contracts.mjs`.
- Run `node tests/protocol_parity.mjs` if WebSocket protocol changed.
- Run the smallest live Node suite that covers lobby join/create behavior. If no focused suite
  exists, add one or extend the existing lobby/team integration path narrowly.

## Manual Testing Focus

- Load the app, leave the name unchanged, and join an open listed lobby.
- Edit the name, create a named lobby in the modal, and confirm the joined lobby shows the created
  room and host.
- Attempt to create the same lobby name from another tab and confirm the modal shows a duplicate
  error rather than joining it.
- Fill a waiting room, then join from the full row as a spectator.
- Start a match and confirm the row remains visible, gray, and disabled.
- Disconnect/reconnect or force a stale list and confirm actions do not strand the user.

## Handoff

Mark this phase done in the implementation commit. Summarize the final create and join flows,
server error mapping, any compatibility remnants for tests, and the manual browser cases Phase 4
should retest while hardening.
