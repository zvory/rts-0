# Capsule: client UI

Use when changing rendering, input, HUD, lobby UI, or any module under `client/src/`.

## Read first
- [docs/design/architecture.md](../design/architecture.md) — just the client paragraph
- [docs/design/client-ui.md](../design/client-ui.md) — module contracts in §4.1 and current Pixi
  behavior in §4.2
- [client-rendering.md](../design/client-rendering.md) — renderer-neutral contracts and parity ledger
- [client-stress-tests.md](../design/client-stress-tests.md) — benchmark contract
- [../client-performance-optimization-lessons.md](../client-performance-optimization-lessons.md) —
  measured wins, rejected experiments, live rig counts, and recurring analytical mistakes

## Code map
- `app-shell`: `main.js`, `app.js`, `match*.js`, launch/Interact bridges, diagnostics,
  replay/spectator/Lab wiring, policies, room-time controls, and capabilities.
- `model`: `state.js`, `state_ground_decals.js`, `client_intent.js`,
  `command_interaction.js`, command composition/budget, prediction, and display state.
- `transport`: `net.js`, `protocol.js`, `lab_client.js`.
- `rules-mirror`: `config.js` facade + `config/`.
- `ui`: HUD/cards/hotkeys, lobby/history, minimap, Lab/Map Editor panels, scoreboard, and settings.
- `input`: `input/` plus `replay_camera_input.js`; shared command-free camera gestures live in
  `input/camera_navigation.js`.
- `renderer`: `renderer/` facade, layers, terrain/decals, entities, fog, feedback, rigs, palettes.
- `platform`: bootstrap, audio, alerts, fog, camera, prediction settings, and reports.

## Invariants
- **Pixi worker only.** `pixi_render_worker.js` imports v8.19.0; main-thread code never constructs Pixi.
- **Client architecture check.** Run `node scripts/check-client-architecture.mjs` for client changes.
- **Cross-area imports are constrained.** `protocol.js` and `config.js` are shared mirrors,
  same-area imports are allowed, and `app-shell` may compose other areas. Non-shell cross-area
  imports should use DI through `Match`/`App`, or be documented in
  `ALLOWED_CROSS_AREA_IMPORTS` with a reason.
- **Lab UI stays app-owned.** `App` owns `LabClient` and `LabPanel`; `Match` receives injected lab
  metadata/control policy and must not import the lab transport or panel modules directly.
- **Interact bridges.** Explicit Lab, isolated-game, and bounded dev-scenario launches install
  narrow bridges called only by `scripts/interact/`; none exposes app, match, transport, renderer,
  or state references.
- **Setup authoring.** `/lab` opens catalog or blank setups; its app-owned panel validates state
  and supports setup JSON export/import.
- **Map Editor boundary.** `/map-editor` owns Pixi/camera/session state without `Net`, `Match`,
  `GameState`, or a simulation room. One-use two-minute Lab handoffs carry only map data.
- **Room affordances are metadata-driven.** `room_capabilities.js` parses `startPayload.capabilities`
  and `startPayload.diagnostics`; shared controls must not be inferred from replay/dev/lab identity.
- **Client intent is explicit.** `Match` owns `ClientIntent` and injects it into HUD, input,
  minimap, and renderer feedback. Do not read or write placement, command targeting, command-card
  mode, active lab tools, Lab ruler, previews, or command feedback through `GameState` shims. Lab setup tools
  are armed through `Match` and consumed by input world clicks, not by panel-owned viewport
  listeners.
- **Commands and ownership are explicit.** `Match` injects one `CommandInteraction` into Input,
  HUD, and Minimap for issue-and-record, plus a frozen read-only policy projection into ownership
  consumers. `GameState` carries no policy; LabPanel gets mutable command-limit settings separately.
- **Teardown.** Any module that holds DOM/window listeners or GPU resources must implement
  `destroy()`. `Match.destroy()` calls it on every module between matches.
- **Coordinates.** World pixels on the wire and in client code, except fields ending in `Tile`.
- **Large-file ratchet.** Do not churn large files just to reduce line count. Prefer extracting a
  focused helper/collaborator and update checker baselines or allowlists only with a reason.
- **Programmatic coverage.** UI refactors need contract coverage where practical: descriptor/DOM
  coverage for command cards, targeted checks for rendering changes, and client smoke for rendered
  behavior.

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
