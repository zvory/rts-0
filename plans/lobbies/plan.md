# Lobby Browser - Multi-Phase Plan

This plan replaces manual room-name joining with a first-screen lobby browser. Players should see
current rooms quickly, understand whether a room is joinable, and create a named lobby through a
modal instead of typing a room name into the normal join form. The browser must show host, map,
relative age, occupied active slots, and a row action without reintroducing role selection or team
preset concepts.

## Product Contract

- The first lobby screen shows a lobby browser immediately, before the player has joined any room.
  The list can show a loading or empty state on first paint, but it must request data immediately
  and keep refreshing while the browser is visible.
- Manual room-name joining disappears from the normal UI. Room names remain internal identifiers
  and may still be accepted by protocol/tests, but players should join from a listed row or create a
  lobby through the modal.
- A `Create Lobby` button opens a modal where the user chooses the room/lobby name. Creation must
  not silently join an existing room with the same name.
- "Creator" terminology is not used. The list says `Host`, and the value is the current room host,
  not necessarily the original first creator if host ownership changes.
- The list includes waiting, full, and in-progress normal rooms. In-progress rows are visible,
  visually muted, and disabled.
- Active-slot count means occupied active slots, including humans and AI. Spectators are shown
  separately only if space allows; they do not count toward the occupied-slot number.
- Full waiting lobbies are not deleted or hidden. They remain joinable as a spectator, with the row
  action making that behavior clear.
- The map column shows only the map name.
- Age is shown as relative time such as `just now`, `3m ago`, or `1h ago`. Exact timestamps can be
  exposed in a tooltip or `title` attribute, but they should not dominate the row.
- New lobbies should appear quickly. Polling every 1-2 seconds is acceptable; a WebSocket push path
  is preferred if it can be added without making the browser brittle.
- Stale or disabled rows must not look clickable. When the server says a room is in-game, gray the
  row and disable its action.

## UI Design Contract

The browser should feel like an operational lobby board, not a marketing page. It should prioritize
scan speed, obvious row actions, and stable layout over large decorative panels.

Before join, the first screen should use the existing lobby shell but change its emphasis:

- The main, wider area becomes `Lobby Browser`, with a dense list/table of lobbies.
- The side panel becomes a compact identity and creation panel, with the editable player name, a
  `Create Lobby` button, connection/status text, and any existing host controls hidden until after
  joining.
- The current joined-lobby room/team layout remains the post-join state. It should reuse the
  existing slot-based joined lobby UI after a successful create or join.

Lobby row information hierarchy:

- Primary line: lobby/room name plus status chip. Status values should be plain and scannable:
  `Open`, `Full`, `Starting`, `In match`, `Stale`.
- Secondary line or columns: `Host`, `Map`, `Made`, `Slots`.
- Action: one right-aligned button per row. Use `Join lobby` for open waiting rows, `Join as
  spectator` for full waiting rows, and a disabled button such as `In match` for in-progress rows.
- Do not show team colors on lobby rows. Player colors remain attached to players only after join.
- Do not show role-selection controls. The row action decides active join vs spectator join from
  server summary state.

Desktop layout:

- Use a table-like grid with stable columns: lobby, host, map, made, slots, action.
- Keep row height consistent, with a left status accent or chip that is not solely color-coded.
- Sort joinable waiting rows first, then full waiting rows, then starting/countdown rows, then
  in-progress rows. Within each group, sort newest first or preserve server order if the server
  deliberately sorts.
- Use restrained colors with enough contrast for disabled rows. Do not make the screen a one-hue
  blue/slate or purple gradient theme.

Mobile layout:

- Collapse rows into compact cards, one per lobby, with the lobby name/status on top and a two-row
  metadata grid underneath.
- Keep the action button full width at the bottom of each row/card so it remains easy to tap.
- Ensure long lobby names and host names wrap or truncate without overlapping the action.

Create modal:

- The modal should be small and task-focused: title, lobby name input, validation/error line, and
  `Cancel` / `Create lobby` actions.
- Focus should move into the modal on open, return to the triggering button on close, and close on
  Escape. Tab focus must stay inside the modal while it is open.
- Validate the room name before sending and mirror server errors if creation fails because the name
  already exists, is reserved, or the server is draining.
- Keep visible text sparse. Do not add tutorial copy explaining the whole feature.

Browser states:

- Loading: show a stable skeleton or quiet loading row inside the browser surface.
- Empty: show a compact empty state with `Create Lobby` as the primary action.
- Disconnected: keep the last known rows if available, mark actions disabled, and show the
  connection status in the side panel.
- Stale row after a failed join: refresh the list immediately and leave the row muted/disabled if
  the latest summary says it is in-game or gone.

## Phase Summaries

Phase 1 establishes the server-owned lobby summary contract. It adds a bounded way to ask normal
room tasks for browser summaries, exposes a list endpoint or equivalent DTO, and adds an atomic
create-lobby path that rejects duplicate or reserved names. It also documents the new contract so
client work does not scrape joined-lobby messages or room internals.

Phase 2 builds the read-only first-screen browser UI. It replaces the pre-join room-name form with a
browser surface, starts immediate 1-2 second polling while the screen is visible, and renders host,
map, relative age, occupied slots, status, and disabled in-game rows. It keeps create and join
actions inert or stubbed until Phase 3, so layout and state rendering can be validated separately.

Phase 3 wires the player actions into the browser. It implements the create-lobby modal, joins open
rows as active players, joins full waiting rows as spectators, and removes the manual room-name join
path from the visible UI. It also handles stale row errors by refreshing immediately and showing
clear disabled states instead of leaving users in a dead click path.

Phase 4 upgrades freshness, tests, and documentation. It adds WebSocket push or invalidation if the
server-side seams support it cleanly, while retaining polling as a fallback. It broadens coverage
for summary state, duplicate create rejection, spectator join from full lobbies, in-game disabled
rows, responsive rendering, and the updated design/protocol docs.

## Phase Index

1. [Phase 1 - Server Lobby Summary Contract](phase-1.md)
2. [Phase 2 - First-Screen Browser UI](phase-2.md)
3. [Phase 3 - Create and Join Flows](phase-3.md)
4. [Phase 4 - Freshness, Hardening, and Docs](phase-4.md)

## Overall Constraints

- Preserve the server-authoritative room model. The browser is a summary of server-owned room
  state; the client must not infer joinability from stale local state when the server rejects a
  join.
- Keep the `Game` API seam intact. Lobby browser work should live in lobby/room orchestration and
  client lobby UI, not by reaching into simulation internals.
- Hide internal rooms from the public browser, including dev self-play rooms, dev scenario rooms,
  match replay rooms, and replay branch rooms.
- Do not reintroduce commander/spectator role selection as a separate pre-join choice. The row
  state determines whether the click is an active join or spectator join.
- Keep the existing slot-based joined lobby behavior. Teams are host-managed slots, not presets,
  and active-slot counts include humans plus AI.
- Keep protocol mirrors synchronized whenever a WebSocket message changes:
  `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, `client/src/protocol.js`, and
  `docs/design/protocol.md`.
- If the lobby list is exposed through HTTP, document that route and use bounded, server-owned DTOs.
  Do not return raw room task state.
- Treat clients as untrusted. Validate lobby names on the server, reject reserved/internal prefixes,
  cap lengths, and keep duplicate create behavior atomic.
- Prefer focused client modules/helpers over growing `client/src/lobby.js` further. If a new module
  owns timers, event listeners, or modal DOM, it must implement teardown and be called from
  `Lobby.destroy()`.
- Use the existing visual language, but improve the pre-join screen into a polished browser. Keep
  cards to real row/card items, avoid nested cards, avoid decorative orbs/gradients, and make
  responsive dimensions stable.
- Do not run broad local test bundles during development by default. Use focused Rust, protocol,
  client contract, architecture, and live Node suites that match each phase, then rely on the commit
  hook when the phase is ready to merge.

## Implementation Process

Implement one phase at a time. Each phase should be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit for that phase.

After each phase, the implementing agent must provide a handoff message describing what the next
agent should do, any constraints or decisions discovered during implementation, and the core
features that should be manually tested. Manual testing notes should name the essential flows, not
a comprehensive test matrix.
