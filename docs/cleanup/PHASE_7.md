# Phase 7 - Client Renderer and Input Decomposition

Goal: split the largest client modules into small dependency-injected collaborators while preserving
the no-build-step PixiJS client.

## Target Components

For `client/src/renderer.js`:

- `renderer/index.js`: `Renderer` facade, Pixi app ownership, resize, destroy, and render
  orchestration.
- `renderer/layers.js`: layer names, layer creation, draw order, and pooling primitives.
- `renderer/terrain.js`: static terrain render texture construction.
- `renderer/entities.js`: per-entity reconciliation and shared entity draw dispatch.
- `renderer/units.js`: infantry, tanks, selection rings, hp bars, setup visuals, and weapon recoil.
- `renderer/buildings.js`: buildings, construction scaffolding, rally markers if rendered here.
- `renderer/resources.js`: steel/oil nodes and depletion visuals.
- `renderer/fog.js`: fog overlay drawing owned by renderer, distinct from simulation fog data.
- `renderer/feedback.js`: attack tracers, placement ghost, drag box, and transient overlays.
- `renderer/palette.js`: renderer-local colors or visual constants that do not belong in
  `config.js`.

For `client/src/input.js`:

- `input/index.js`: `Input` facade and lifecycle.
- `input/selection.js`: hit testing, drag selection, selectable filtering.
- `input/commands.js`: command construction and command-mode transitions.
- `input/placement.js`: building placement preview, footprint validation, and tile conversion.
- `input/camera_controls.js`: pan/zoom keyboard and pointer handling.

For `client/src/main.js`:

- `app.js`: lobby/app shell lifecycle.
- `match.js`: match lifecycle and dependency wiring.
- `alerts.js`: toasts, under-attack alerts, viewport alert behavior.
- `bootstrap.js`: DOM lookup and application startup.

For `client/styles.css`:

- Split only if plain CSS imports are acceptable for the served client.
- Candidate files: `base.css`, `lobby.css`, `hud.css`, `game.css`, `dialogs.css`,
  `scoreboard.css`.

## Design Notes

The client has no JS build step. Use plain ES modules and browser-supported CSS only. PixiJS remains
the global `PIXI`; do not introduce a bundler or framework as part of cleanup.

Keep module dependencies explicit through constructor arguments and method parameters. Client
modules should not silently import each other to reach live state.

## Tests

- Run contract tests that cover client protocol/config behavior.
- Run the client smoke test after renderer, input, or match lifecycle changes.
- Manually inspect a local match if visual drawing code moves.

## Done

- `Renderer`, `Input`, and match bootstrap remain simple facades.
- Rendering layers and input modes have clear ownership.
- The served client still works without a build step.

