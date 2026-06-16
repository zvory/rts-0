# Phase 2 - First-Screen Browser UI

Status: planned.

## Goal

Replace the pre-join room-name form with a polished, read-only lobby browser that is visible on
first paint and refreshes quickly while the lobby screen is open.

## Scope

- Add a browser surface to the pre-join lobby screen.
- Remove the visible room-name input from the normal pre-join UI.
- Keep the player name input visible and editable before join, defaulting through the existing
  localStorage behavior.
- Fetch lobby summaries immediately when the Lobby controller is constructed or shown.
- Poll every 1-2 seconds while the lobby screen is visible and not joined.
- Render host, map, relative age, occupied slots, row status, and row action state from the server
  summary.
- Render in-progress rows as muted and disabled.
- Render full waiting rows distinctly from open rows and keep them visible.
- Keep action buttons disabled or no-op in this phase if Phase 3 owns final create/join behavior.
- Preserve the existing joined-lobby team/slot UI after `_joined` is true.

## UI Requirements

Desktop:

- Use a stable grid header with columns: lobby, host, map, made, slots, action.
- Make the lobby name the strongest text in the row. Host, map, made, and slots should be easy to
  scan but visually secondary.
- Use status chips that combine text and shape, not color alone.
- Keep row action buttons aligned so scanning down the right edge is easy.
- Keep in-game rows visible but visually muted with disabled action buttons.

Mobile:

- Collapse each row into a compact card-like row with no nested cards.
- Put lobby name and status on the first line.
- Put host, map, age, and slots into a two-column metadata grid.
- Put the action button at the bottom of the row/card with stable width.
- Ensure long lobby names and host names truncate or wrap cleanly without overlapping buttons.

States:

- Loading state appears inside the browser surface without shifting the whole page.
- Empty state is compact and should offer the create action once Phase 3 wires it.
- Disconnected state keeps the screen stable and disables row actions.
- Error state should be concise and should not replace the last known list unless there is no data.

## Suggested Client Architecture

- Add a focused browser view helper, for example `client/src/lobby_browser_view.js`, rather than
  expanding `lobby_view.js` or `lobby.js` too much.
- Keep `Lobby` as the coordinator that owns Net, current player name, joined state, and lifecycle.
- Add a small browser controller or polling helper if that keeps timers and fetch cancellation out
  of the rendering helper.
- Ensure any interval, abort controller, or DOM listener is cleaned up from `Lobby.destroy()`.
- Compute relative age in client code from server `createdAtUnixMs`; refresh displayed ages on poll
  and optionally on a lightweight local timer.
- Use dependency injection for fetch/clock in pure helpers where tests need deterministic behavior.

## Touch Points

- `client/index.html`
- `client/styles.css`
- `client/src/lobby.js`
- `client/src/lobby_view.js` only if shared lobby helpers are reused
- New `client/src/lobby_browser_view.js` or equivalent
- `client/src/net.js` if the summary fetch belongs there
- `scripts/check-client-architecture.mjs` only if a new module needs classification or allowlist
  updates
- Client contract/smoke tests that assert pre-join lobby markup

## Constraints

- Do not implement a marketing landing page. The browser is the first useful screen.
- Do not add visible tutorial copy explaining how to use the browser.
- Do not add separate role-selection controls.
- Do not show map descriptions or thumbnails in this phase. The row shows map name only.
- Do not make the page depend on a JavaScript build step or framework.
- Do not use team-colored row decorations. Player/team color semantics stay in the joined lobby.
- Avoid growing large files without a reason. Extract a helper if the browser renderer becomes
  substantial.

## Verification

- Add focused JS tests for summary sorting, row state rendering, relative age formatting, and empty
  state rendering where the existing client test style supports it.
- Run `node scripts/check-client-architecture.mjs` if a new client module is added.
- Run `node tests/client_contracts.mjs` if DOM/protocol contracts are touched.
- Run `node tests/protocol_parity.mjs` if Phase 1 added or changed protocol mirrors.

## Manual Testing Focus

- Load the app with no rooms and confirm the browser is visible on first paint with a stable empty
  or loading state.
- Create rooms through existing helpers or another tab and confirm the browser updates within the
  configured polling interval.
- Confirm open, full, and in-game rows look distinct.
- Resize to mobile width and confirm text, metadata, and buttons do not overlap.
- Join a room through existing controls or test paths and confirm the post-join lobby still uses
  the current slot-based UI.

## Handoff

Mark this phase done in the implementation commit. Summarize the browser module structure, polling
interval, rendered row states, responsive behavior, and any remaining action wiring left for Phase
3.
