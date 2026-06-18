# Phase 5 - Client Capability Affordances

## Phase Status

- [ ] Pending.

## Objective

Make the browser decide controls and overlays from explicit room capability metadata. Client modules
should not ask whether the current experience is replay, dev scenario, lab, or Debug mode when a
neutral capability answers the actual question.

## Work

- Introduce a small client-side capability model derived from the start payload and reliable room
  state messages.
- Drive room-time controls from clock/time capability metadata rather than `payload.replay` or
  `devWatch.kind`.
- Drive diagnostic toggles from diagnostic capability metadata rather than `debugMode` or scenario
  route identity.
- Drive observer analysis and vision-control affordances from visibility/diagnostic capability
  metadata where practical, while keeping product-specific controls such as replay branch creation or
  lab scenario tools in product-owned shells.
- Keep lab panel and lab transport app-owned. `Match`, HUD, input, minimap, and renderer may receive
  small collaborators or capability records through dependency injection, but they must not import lab
  panels or scenario storage.
- Remove compatibility fallbacks for old start payload fields or old room-time tags created by prior
  phases.
- Add client contract and architecture tests for capability parsing, control mounting, teardown, and
  removed mode-name fallbacks.

## Expected Touch Points

- `client/src/app.js`
- `client/src/match.js`
- `client/src/state.js`
- `client/src/net.js`
- `client/src/protocol.js`
- `client/src/replay_controls.js` or a renamed room-time control module
- `client/src/settings_panels.js`
- `client/src/lab_control_policy.js`
- `client/src/lab_panel.js`
- `client/src/observer_analysis_overlay.js`
- `scripts/check-client-architecture.mjs` only if a precise new rule is justified
- `tests/client_contracts.mjs`
- `docs/design/client-ui.md`
- `docs/context/client-ui.md`
- `plans/lab/room2/phase-5.md`

## Implementation Checklist

- [ ] Add a narrow client capability parser/model.
- [ ] Route time controls through clock/time capability metadata.
- [ ] Route diagnostic settings through diagnostic capability metadata.
- [ ] Remove old mode-name fallback behavior for room-time and diagnostics.
- [ ] Preserve product-owned controls for replay branch and lab setup tools.
- [ ] Add or update client contract and architecture tests.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/protocol_parity.mjs`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `git diff --check`

## Manual Test Focus

Open a normal match, a replay, a dev scenario, and a lab. Confirm each screen mounts the expected
controls, tears them down cleanly between sessions, and does not expose controls that the start
payload did not advertise.

## Handoff Expectations

Name the client capability model, list removed mode-name fallbacks, identify any product-specific UI
references that intentionally remain, and state what guardrails Phase 6 should add.
