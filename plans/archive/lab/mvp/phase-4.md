# Phase 4 - Client Lab Shell

## Phase Status

- [x] Done.

## Objective

Add the browser lab route, client lab services, and app-shell composition needed to run the normal
`Match` inside a lab UI without importing lab panels into match internals.

## Work

- Add a `/lab` client entry flow with map and lab-room selection. Keep normal lobby entry behavior
  unchanged.
- Add `LabClient`, a thin transport service that owns request ids, pending results, lab state
  subscription, timeout/error handling, and teardown.
- Add `LabPanel` or similarly focused UI modules mounted by `App`, not by `Match`.
- Pass lab metadata, `LabClient`, and placeholder control collaborators into `Match` through
  constructor options.
- Render lab state and request results in a compact lab panel: operator/viewer role, room name, map,
  selected vision, dirty state, last result, and server error state.
- Implement vision controls first because they exercise lab protocol, room state, and projection
  without mutating the world.
- Keep the panel layout operational and dense. Do not create a marketing page, nested cards, or a
  separate renderer.
- Add teardown for every new DOM listener, timer, or subscription.
- Add client contract and architecture coverage for protocol builders, app-shell composition, lab
  service teardown, and route behavior.

## Expected Touch Points

- `client/index.html`
- `client/styles.css`
- `client/src/app.js`
- `client/src/bootstrap.js`
- `client/src/net.js`
- `client/src/protocol.js`
- `client/src/match.js`
- `client/src/lab_client.js`
- `client/src/lab_panel.js`
- `client/src/lab_control_policy.js` if a placeholder policy is introduced here
- `tests/client_contracts.mjs`
- `tests/protocol_parity.mjs`
- `scripts/check-client-architecture.mjs`
- `docs/design/client-ui.md`
- `docs/context/client-ui.md`

## Implementation Checklist

- [ ] Add lab route parsing and auto-join/create behavior.
- [ ] Add `LabClient` with request id tracking, state/result handlers, and `destroy()`.
- [ ] Add app-shell-owned lab panel mount/unmount around `Match`.
- [ ] Pass lab collaborators into `Match` through dependency injection.
- [ ] Add lab vision controls and result/error display.
- [ ] Keep normal lobby, replay, branch staging, and dev scenario routes unchanged.
- [ ] Add client tests for protocol builders, route parsing, panel teardown, and lab service result
      handling.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/select-suites.mjs --verify`
- `git diff --check`

If the phase changes rendered behavior materially, also run the narrow client smoke path selected by
`tests/select-suites.mjs`.

## Manual Test Focus

Open `/lab`, create or join a lab room, confirm the normal match view mounts, switch vision modes,
disconnect/reconnect, and return to a normal lobby page without leaked panels or stale listeners.

## Handoff Expectations

Describe the client module boundaries, how lab collaborators are injected into `Match`, and which
controls are intentionally still missing. Call out any client architecture allowlist changes and
why they were needed.
