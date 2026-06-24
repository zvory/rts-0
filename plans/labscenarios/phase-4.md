# Phase 4 - Browser Submission Workflow

## Phase Status

- [x] Done.

## Objective

Expose the one-button authoring flow in the lab UI: validate current lab state, submit it, and show
the draft PR link.

## Work

- Add lab panel capability detection for scenario PR submission, with clear disabled text when the
  backend is not configured.
- Add a "Submit scenario PR" action that uses the Phase 2 authoring metadata and Phase 3 server
  submission contract.
- Keep the user flow explicit: validate current authoritative state, show progress while the server
  submits, and display either a PR link or a specific error.
- Prevent duplicate clicks while a submission is pending. Keep the request bounded and recoverable
  if the socket disconnects or the async job fails.
- Keep authoring controls in the app-owned lab panel; do not put GitHub logic into `Match`, input,
  renderer, HUD, minimap, or `GameState`.
- Preserve local JSON download as the visible fallback when submission is unavailable or fails.
- If the PR opens in a new tab/window, also render a copyable link so popup blocking does not hide
  the result.
- Add focused client coverage for disabled state, validation failure, pending state, success link,
  failure display, and teardown while a request is in flight.

## Expected Touch Points

- `client/src/lab_panel.js`
- `client/src/lab_client.js`
- Optional `client/src/lab_scenario_authoring.js`
- `client/src/net.js` if submission uses HTTP instead of lab WebSocket results
- `client/styles.css`
- `tests/client_contracts/lab_contracts.mjs`
- `docs/design/client-ui.md`
- `docs/design/protocol.md` if client/server DTOs changed in this phase

## Verification

- `node tests/client_contracts/lab_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/protocol_parity.mjs` if protocol changes.
- `git diff --check`

If a mocked browser flow exists after implementation, run the smallest relevant browser smoke or
`node tests/select-suites.mjs --verify`.

## Manual Test Focus

Author a small scenario, validate it, submit it, and confirm the lab panel shows a usable draft PR
link. Repeat with submission disabled and confirm the panel guides the user back to local JSON
download rather than failing silently.

## Handoff Expectations

Summarize the final browser workflow, list the exact disabled/pending/success/error states, and
name any server behavior Phase 5 should harden before treating the feature as ready.
