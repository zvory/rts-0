# Phase 5 - Client State, Controls, Rendering, And Lab Inspection

## Phase Status

Status: done.

Completion notes:

- Client-side Scout Plane owner-state parsing and compact protocol vocabulary were already present
  from earlier phases; this phase added the hidden dismiss command builder and UI dispatch.
- The selected Scout Plane command card exposes only move/retarget and Dismiss. Mixed selections
  keep ordinary land-unit commands scoped to land units while Scout Planes receive retarget/dismiss
  commands only.
- Lab unit spawning now exposes the hidden Scout Plane for inspection without adding it to City
  Centre production. Phase 6 still owns normal production exposure.
- The shipped visual is a rough client-native SVG live rig: a team-tinted, FW 189-inspired
  twin-boom silhouette with a separate shadow and distinct minimap blip. No audio was added.

## Objective

Teach the client to understand, inspect, select, command, and render hidden Scout Plane entities
before normal production exposure. A reviewer should be able to spawn or load a hidden plane in a
lab/dev flow, see that it is an aircraft, retarget its orbit, dismiss it, and verify that the client
does not offer unsupported commands.

## Scope

- Read [docs/context/client-ui.md](../../docs/context/client-ui.md) before changing rendering, HUD,
  input, match modules, hotkeys, lab, or teardown behavior.
- Parse all Scout Plane snapshot fields, command vocabulary, and public state added by Phases 2-4.
- Add selection and control behavior for active friendly planes:
  - direct selection, box selection, and control groups include the plane.
  - selected plane command card exposes move/retarget and cancel/dismiss.
  - selected plane command card does not expose attack, attack-move, hold position, stop, train,
    build, harvest, repair, setup, rally, or autocast commands.
  - mixed selections preserve normal land-unit commands while issuing only approved aerial
    retarget/dismiss commands to planes.
  - queued move commands append later orbit centers using existing queued-command UX where practical.
- Add client command routing for manual dismiss if Phase 4 introduced a new command shape.
- Add a rough but deliberate visual treatment:
  - FW 189-inspired twin-boom silhouette or equivalent clearly aerial shape.
  - distinguishable from Scout Car, Tank, Command Car, and support weapons at normal zoom.
  - lightweight flying/orbit motion treatment.
  - readable team tint, selection ring, hover, health bar, and minimap blip.
  - placeholder/final-art status documented if rough vector/client-native art ships.
- Add lab or dev-scenario inspection support so humans can spawn or load a plane without playing a
  full tech path.
- Keep normal City Centre production hidden until Phase 6.
- Ensure teardown remains clean for any new listeners, timers, textures, generated graphics, sounds,
  or GPU resources.
- Do not add audio unless the phase explicitly chooses intentional audio and documents it.

## Expected Touch Points

- `client/src/protocol.js`
- `client/src/protocol_constants.js`
- `client/src/protocol_snapshot.js`
- `client/src/state.js`
- `client/src/frame_entity_views.js`
- `client/src/hud.js`
- `client/src/hud_command_card.js`
- `client/src/hud_command_dom.js`
- `client/src/hud_unit_commands.js`
- `client/src/input/commands.js`
- `client/src/input/*.js`
- `client/src/command_composer.js`
- `client/src/client_intent.js`
- `client/src/config.js`
- `client/src/config/*.js`
- `client/src/renderer/`
- `client/src/minimap.js`
- `client/src/match*.js`
- `server/src/lab_scenarios.rs`
- `server/crates/sim/src/game/setup/dev_scenarios.rs`
- `tests/client_contracts/*.mjs`
- `tests/rig_runtime.mjs` if the rig pipeline is used
- `docs/design/client-ui.md` if exported client module contracts change

## Edge Cases To Cover

- A hidden plane in a snapshot parses without throwing and renders with a non-missing visual.
- Friendly direct selection, box selection, and control groups can include the plane.
- Enemy projection only allows selection/hover behavior appropriate for visible enemy entities.
- The plane command card contains only move/retarget and dismiss.
- Right-click and command-card move retarget the selected plane's orbit center.
- Shift-queued move retargeting preserves the existing queued-command UX.
- Mixed selections issue normal land commands to land units and only valid aerial commands to planes.
- Dismiss command sends the correct server command and clears local command intent safely.
- The plane can be inspected from lab/dev without enabling normal City Centre production.
- Rendering remains readable at normal zoom and does not overlap incoherently with health bars,
  selection rings, fog, minimap, or command feedback.
- Match teardown and rematch do not leak listeners, Pixi graphics, textures, or WebGL resources.

## Verification

- Focused client protocol/state tests for Scout Plane parsing and safe fallback behavior.
- Focused command-card tests for plane-only controls and hidden City Centre production.
- Focused input tests for right-click move, command-card move, queued retargeting, dismiss, and mixed
  selections.
- Focused renderer or rig runtime tests for a non-missing visual and pool cleanup.
- Focused minimap tests if the blip shape or visibility differs from ordinary units.
- `node scripts/check-client-architecture.mjs`.
- `tests/run-all.sh --no-rust` if the change reaches enough client rendering/input/HUD surface to
  need live smoke coverage.
- `git diff --check`.

## Manual Test Focus

Start the inspection lab/dev scenario and create a Scout Plane. Select it directly and with a box,
put it in a control group, retarget its orbit, queue a second retarget, dismiss it, and rematch to
check teardown. Also inspect a mixed land-unit plus plane selection to confirm land units still obey
ordinary ground commands.

## Handoff Expectations

Name the final client parser fields, command-card affordances, input routing behavior, visual asset
or rough-art path, lab/dev inspection id, manual test URL, and any placeholder art/audio follow-up.
Tell Phase 6 exactly what remains to expose normal production and the City Centre select/pan
behavior.
