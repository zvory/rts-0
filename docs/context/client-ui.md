# Capsule: client UI

Use when changing rendering, input, HUD, lobby UI, or any module under `client/src/`.

## Read first
- [docs/design/architecture.md](../design/architecture.md) — just the client paragraph
- [docs/design/client-ui.md](../design/client-ui.md) — JS client modules & exported APIs
- §4.1 Module export contracts
- §4.3 Client architecture workflow
- §4.2 Rendering & look (PixiJS, procedural art)

## Code map
- `app-shell`: `main.js`, `app.js`, `match.js`, `match_health.js`, `replay_controls.js`,
  `replay_viewer.js` — app lifecycle, dependency injection, replay shell.
- `model`: `state.js`, `command_composer.js`, `prediction_controller.js` — client snapshot state,
  command-target lifetime, and local command prediction bookkeeping.
- `transport`: `net.js`, `protocol.js` — WebSocket wrapper and wire mirror.
- `rules-mirror`: `config.js` — UI/render/fog subset of mirrored balance.
- `ui`: `hud.js`, `hud_command_card.js`, `lobby.js`, `match_history.js`, `status_badge.js`,
  `minimap.js`, `branch_staging.js`, `settings_container.js`, `settings_panels.js` —
  DOM/HUD/lobby/minimap/settings surfaces.
- `input`: `input/` plus `replay_camera_input.js` — input facade and area-local collaborators.
- `renderer`: `renderer/` — Pixi facade, layers, terrain, entity, fog, feedback, art helpers.
- `platform`: `bootstrap.js`, `audio.js`, `combat_audio.js`, `alerts.js`, `fog.js`, `camera.js`
  — browser/platform services and camera/fog helpers.

## Invariants
- **No framework, no JS build step.** Plain ES2020 modules. PixiJS v7 is the global `PIXI` — do
  not `import` it.
- **Client architecture check.** Run `node scripts/check-client-architecture.mjs` for client
  module or architecture-policy changes. It classifies every `client/src/**/*.js` module and reports
  large-file and fan-in/out baselines.
- **Cross-area imports are constrained.** `protocol.js` and `config.js` are shared mirrors, same-area
  imports are allowed, and `app-shell` may compose other areas. Non-shell cross-area imports should
  use dependency injection through `Match`/`App`; if an import is intentional, update
  `ALLOWED_CROSS_AREA_IMPORTS` in `scripts/check-client-architecture.mjs` with a reason.
- **Teardown.** Any module that holds DOM/window listeners or GPU resources must implement
  `destroy()`. `Match.destroy()` calls it on every module between matches — omitting it leaks
  listeners/WebGL contexts across rematches.
- **Coordinates.** World pixels on the wire and in client code, except fields ending in `Tile`.
- **Large-file ratchet.** Do not churn large files just to reduce line count. When adding behavior,
  prefer extracting a focused helper/collaborator; update checker baselines or allowlists only with
  a reason.
- **Programmatic coverage.** UI refactors need contract coverage where practical: descriptor/DOM
  coverage for command cards, targeted checks for rendering changes, and client smoke for rendered
  behavior.

## Future client change checklist
- Did this add a listener, timer, WebSocket handler, Pixi object, texture, or GPU resource?
  Add/update `destroy()` and the owning teardown call.
- Did this add a non-shell cross-area import? Prefer DI through `Match`/`App`, or update the checker
  allowlist with a reason.
- Did this change command-card behavior? Add descriptor or DOM contract coverage.
- Did this change rendering? Run client smoke and add a targeted check where possible.
- Did this touch `protocol.js` or `config.js`? Update the mirrored server file and the relevant
  design/context docs.

## Suite selection
- `client/src/` changes select `client-architecture`, `js-protocol-contracts`,
  `node-minimap-input-contracts`, and `client-smoke`.
- Client transport/protocol changes also select `node-server-integration`.
- `scripts/check-client-architecture.mjs`, `tests/select-suites.mjs`, and `plans/client-arch/*`
  select `client-architecture`.
- Verify selector rules with `node tests/select-suites.mjs --verify`.

## Cross-capsule triggers
- Adding/changing a wire field → [protocol.md](protocol.md) (and update `server/src/protocol.rs`).
- Changing a number players see (cost, sight, size) → [balance.md](balance.md).
