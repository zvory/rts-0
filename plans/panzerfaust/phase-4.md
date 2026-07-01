# Phase 4 - Client State, Visuals, And Lab Inspection

## Phase Status

Status: done.

## Objective

Make hidden Panzerfaust entities understandable in the client and easy to inspect manually. This
phase should add state parsing, rendering, and a controlled inspection path without exposing normal
Barracks training yet.

## Scope

- Update client protocol/state handling for the Panzerfaust kind and any Phase 2/3 events:
  - Launch, travel, impact, recovery, conversion, or attack feedback fields.
  - Same-id conversion into Rifleman through normal snapshot updates.
  - Safe fallback behavior for missing or older event fields if protocol compatibility requires it.
- Add a distinguishable loaded Panzerfaust visual:
  - Infantry-sized silhouette.
  - Visible carried Panzerfaust.
  - Team-color readability at normal zoom.
  - Clear difference from Rifleman and Machine Gunner without changing final art direction more than
    necessary.
- Render fog-safe one-shot feedback:
  - Launch or muzzle cue only for recipients allowed by server projection.
  - Short travel/tracer cue if the protocol exposes one.
  - Impact cue only where the server permits it.
  - No client-side fog stamping from visual-only events.
- Add a lab or dev-scenario inspection path so a reviewer can create or view Panzerfaust units
  without playing the full tech path.
- If lab spawn catalogs are used for this path, intentionally expose Panzerfaust there in this phase
  and test that this exposure does not imply normal production exposure. Otherwise add a
  server-authored dev scenario under `/dev/scenarios`.
- Ensure selected Panzerfaust units can still receive normal movement, direct Attack, Attack Move,
  Stop, Hold Position, and queued commands through existing client command routing.
- Keep normal production exposure disabled until Phase 5.
- Add client contract or rendering tests for Panzerfaust state handling and visual feedback.
- Do not add final audio polish in this phase unless a small sound placeholder is required for
  correctness; Phase 6 owns intentional audio treatment.

## Expected Touch Points

- `client/src/protocol.js`
- `client/src/state.js`
- `client/src/state_ground_decals.js` if visual events use decal buffers
- `client/src/config.js`
- `client/src/config/*.js`
- `client/src/renderer/entities.js`
- `client/src/renderer/units.js`
- `client/src/renderer/feedback.js`
- `client/src/renderer/feedback_view_model.js`
- `client/src/renderer/rigs/infantry_svg.js`
- `client/src/renderer/rigs/live_routing.js`
- `client/src/renderer/rigs/runtime.js`
- `client/src/lab*.js`
- `server/crates/sim/src/game/setup/dev_scenarios.rs`
- `server/src/lab_scenarios.rs`
- `tests/client_contracts/*.mjs`
- `tests/rig_runtime.mjs`
- `docs/design/client-ui.md`
- `docs/design/testing.md` if scenario guidance changes

## Edge Cases To Cover

- A Panzerfaust entity that converts to Rifleman with the same id updates in place instead of
  leaving stale loaded visuals.
- Renderer rig/runtime caches that key by entity id are invalidated or rebuilt when an entity's
  kind changes, so same-id Panzerfaust-to-Rifleman conversion cannot keep a stale loaded rig.
- Launch or impact events received without the target entity visible do not reveal hidden target
  identity or position beyond server-projected payload data.
- Visual-only events do not extend client fog or create fake visibility.
- Selection rings, health bars, range overlays, minimap dots, hover labels, and command feedback
  remain stable across conversion.
- Lab/dev spawn controls do not expose arbitrary client-side spawning or bypass server validation.
- New renderer resources or listeners are cleaned up on `Match.destroy()`.

## Verification

- Focused client contract tests for protocol parsing, `GameState` event handling, same-id conversion
  state, and fog-safe feedback buffering.
- `node tests/rig_runtime.mjs` or focused rig tests for the Panzerfaust infantry visual.
- Focused renderer feedback tests for launch/travel/impact cues if events are added.
- `node scripts/check-client-architecture.mjs`.
- `node tests/protocol_parity.mjs` if protocol mirrors are adjusted.
- `node tests/client_contracts/lab_contracts.mjs` if lab spawn or scenario controls change.
- `git diff --check`.

## Manual Test Focus

Open the lab/dev inspection path and spawn or load a Panzerfaust near a Tank. Confirm the loaded
unit is visually distinct, direct Attack and Attack Move can trigger the one-shot behavior, feedback
is visible only where expected, conversion leaves the same unit selectable as a Rifleman with a
Rifleman rig, and normal Barracks production still does not expose the unit.

## Handoff Expectations

Name the manual inspection scenario or lab flow, the client event/state fields handled, and the
visual assets or rig routes added. Tell Phase 5 whether the unit is ready to expose in normal
production or which client readability issue must be fixed first.
