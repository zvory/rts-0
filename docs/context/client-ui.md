# Capsule: client UI

Use when changing rendering, input, HUD, lobby UI, or any module under `client/src/`.

## Read first in `DESIGN.md`
- §1 High-level architecture (just the client paragraph)
- §4 JS client — modules & exported APIs
- §4.1 Module export contracts
- §4.2 Rendering & look (PixiJS, procedural art)

## Code map
- `client/src/main.js` — entry point; starts `App`
- `client/src/app.js` / `client/src/match.js` — app shell and match dependency injection
- `client/src/renderer/` and `client/src/input/` — decomposed renderer/input collaborators;
  `renderer/index.js` and `input/index.js` are the facades
- `client/src/protocol.js` — wire mirror (treat as read-only unless touching protocol)
- `client/src/config.js` — UI/render/fog subset of balance
- One small class per file under `client/src/`

## Invariants
- **No framework, no JS build step.** Plain ES2020 modules. PixiJS v7 is the global `PIXI` — do
  not `import` it.
- **No cross-imports between modules.** Collaborators come via DI from `main.js`. The only allowed
  cross-imports are `protocol.js` and `config.js`.
- **Teardown.** Any module that holds DOM/window listeners or GPU resources must implement
  `destroy()`. `Match.destroy()` calls it on every module between matches — omitting it leaks
  listeners/WebGL contexts across rematches.
- **Coordinates.** World pixels on the wire and in client code, except fields ending in `Tile`.

## Cross-capsule triggers
- Adding/changing a wire field → [protocol.md](protocol.md) (and update `server/src/protocol.rs`).
- Changing a number players see (cost, sight, size) → [balance.md](balance.md).
