# Capsule: client UI

Use when changing rendering, input, HUD, lobby UI, or any module under `client/src/`.

## Read first
- [docs/design/architecture.md](../design/architecture.md) — just the client paragraph
- [docs/design/client-ui.md](../design/client-ui.md) — JS client modules and exported APIs
- §4.1 Module export contracts
- §4.2 Rendering and look
- §4.3 Client architecture workflow

## Code map
- `app-shell`: `main.js`, `app.js`, `match.js`, `frame_recovery.js`, `frame_entity_views.js`,
  `match_health.js`, `observer_analysis_overlay.js`, `observer_analysis_signatures.js`,
  `replay_controls.js`, `replay_viewer.js`, `lab_control_policy.js`, `room_capabilities.js`; app lifecycle, dependency injection,
  frame-loop health, observer analysis, and replay/spectator/lab shell wiring.
- `model`: `state.js`, `client_intent.js`, `command_budget.js`, `command_composer.js`,
  `progress_extrapolator.js`, prediction controllers/adapters, and browser-local display state.
- `transport`: `net.js`, `protocol.js`, `lab_client.js`.
- `rules-mirror`: `config.js`.
- `ui`: HUD, command card, hotkeys, lobby controller/view, match history, resource icons,
  scoreboard, status badge, minimap, branch staging, lab panel, and settings.
- `input`: `input/` plus `replay_camera_input.js`; shared command-free camera gestures live in
  `input/camera_navigation.js`.
- `renderer`: `renderer/` Pixi facade, layers, terrain, entities, fog, feedback, art helpers,
  rig schema/import/runtime, and renderer-local palette helpers.
- `platform`: `bootstrap.js`, `audio.js`, `combat_audio.js`, `alerts.js`, `fog.js`, `camera.js`,
  and `prediction_settings.js`.

## Invariants
- **No framework, no JS build step.** Plain ES2020 modules. PixiJS v7 is the global `PIXI`; do not
  import it.
- **Client architecture check.** Run `node scripts/check-client-architecture.mjs` for client module
  or architecture-policy changes.
- **Cross-area imports are constrained.** `protocol.js` and `config.js` are shared mirrors,
  same-area imports are allowed, and `app-shell` may compose other areas. Non-shell cross-area
  imports should use dependency injection through `Match`/`App`, or be documented in
  `ALLOWED_CROSS_AREA_IMPORTS` with a reason.
- **Lab UI stays app-owned.** `App` owns `LabClient` and `LabPanel`; `Match` receives injected lab
  metadata/control policy and must not import the lab transport or panel modules directly.
- **Room affordances are metadata-driven.** `room_capabilities.js` parses `startPayload.capabilities`
  and `startPayload.diagnostics`; shared controls must not be inferred from replay/dev/lab identity.
- **Client intent is explicit.** `Match` owns `ClientIntent` and injects it into HUD, input,
  minimap, and renderer feedback. Do not read or write placement, command targeting, command-card
  mode, previews, or command feedback through `GameState` shims.
- **Teardown.** Any module that holds DOM/window listeners or GPU resources must implement
  `destroy()`. `Match.destroy()` calls it on every module between matches.
- **Coordinates.** World pixels on the wire and in client code, except fields ending in `Tile`.
- **Large-file ratchet.** Do not churn large files just to reduce line count. Prefer extracting a
  focused helper/collaborator and update checker baselines or allowlists only with a reason.
- **Programmatic coverage.** UI refactors need contract coverage where practical: descriptor/DOM
  coverage for command cards, targeted checks for rendering changes, and client smoke for rendered
  behavior.

## Future client change checklist
See [docs/design/client-ui.md](../design/client-ui.md) §4.3 for the full checklist. In short:
cover teardown for new listeners/resources, prefer dependency injection for cross-area wiring, add
command-card/rendering coverage for visible behavior, and update protocol/config mirrors with the
owning server docs when those files change.

## Suite selection
- `client/src/` changes select `client-architecture`, `js-protocol-contracts`,
  `node-minimap-input-contracts`, and `client-smoke`.
- Client transport/protocol changes also select `node-server-integration`.
- `scripts/check-client-architecture.mjs`, `tests/select-suites.mjs`, and
  `plans/archive/client-arch/*` select `client-architecture`.
- Verify selector rules with `node tests/select-suites.mjs --verify`.

## Cross-capsule triggers
- Adding/changing a wire field → [protocol.md](protocol.md) and update `server/src/protocol.rs`.
- Changing a number players see, such as cost, sight, or size → [balance.md](balance.md).
- Touching sim/lobby behavior behind a UI flow → [server-sim.md](server-sim.md).
