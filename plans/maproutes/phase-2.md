# Phase 2: Spectator diagnostics transport and overlay shell

Status: Done

## Goal

Make static map-analysis data visible to humans in AI-vs-AI spectator diagnostics before it drives
AI decisions.

## Scope

- Add a spectator-only diagnostics payload for map-analysis debug primitives.
- Extend the existing AI diagnostics panel with a map or terrain section and layer toggles.
- Render server-provided overlay primitives on the game world, starting with component/region fills,
  base markers, resource cluster markers, and labels from Phase 1.
- Keep the overlay passive: it must not affect simulation, commands, selection, prediction, or fog.
- Gate the payload to spectator/observer diagnostics audiences consistent with existing AI
  diagnostics behavior.

## Non-goals

- Do not implement final region/choke extraction if Phase 1 only provides components.
- Do not change AI decisions or command emission.
- Do not create a separate client-side analyzer; the client only draws server/AI-provided data.

## Expected touch points

- `server/src/lobby/live_tick.rs`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- Existing observer/AI diagnostics client modules under `client/src/`
- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/design/ai.md`

## Verification

- Run focused protocol/client smoke or targeted JS tests if available for diagnostics payloads.
- Run focused Rust tests covering payload construction and spectator-only availability.
- Check protocol mirrors compile and serialize/deserialize the added payload shape.

## Manual testing focus

Start an AI-vs-AI spectator game and confirm the diagnostics panel can toggle map-analysis layers.
Verify overlays align with terrain/base locations, labels remain readable, and toggling layers does
not interfere with camera, selection, or existing AI diagnostics.

## Handoff

The handoff must include screenshots or a clear description of what layers render correctly, plus
any readability issues the Phase 3 implementer should account for when adding real chokes/routes.
