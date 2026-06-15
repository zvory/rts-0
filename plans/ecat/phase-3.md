# Phase 3 - Client Ability Object Surface

Status: Not Started.

## Goal

Add client-side storage, basic rendering, and preview support for projected ability world objects.
The visuals can be simple placeholders, but they must use the normal client module boundaries and
teardown rules.

## Scope

- Store decoded ability objects in `GameState`, similar to `state.smokes` but with a distinct
  ability-object field.
- Render basic placeholders in the existing renderer layer order:
  - return markers as visible ground marks
  - anchors as persistent ground objects
  - projectile/debug objects only if Phase 2 exposes them as active objects
- Keep projected objects non-selectable unless a later phase explicitly adds targetability.
- Add an ability-specific preview helper that can draw:
  - existing range rings for ordinary world-point abilities
  - a return marker preview when a dash is armed
  - multiple projectile origins for anchor-enhanced line shots
  - line or path previews from each origin to the cursor
- Ensure minimap and HUD behavior do not accidentally treat ability objects as entities.
- Update `docs/design/client-ui.md` if exported state or renderer contracts change.

## Expected Deliverables

- Client state decodes and retains the latest projected ability object list.
- Renderer displays readable placeholder return markers and anchors without overlapping selection
  rings, health bars, or fog in an incoherent way.
- Ability target preview code can represent multiple origins without changing command authority.
- Client teardown and architecture checks remain clean.

## Out of Scope

- Implementing actual dash, projectile, or anchor gameplay.
- Adding click/attack targeting for anchors.
- WASM prediction or local authoritative simulation.
- Final art, sound, or animation polish.

## Verification

- Run `node scripts/check-client-architecture.mjs` if client modules or imports change.
- Add or update focused client contract tests for decoded ability objects and preview descriptors.
- Run a narrow client smoke or renderer-focused check if rendering code changes enough to risk a
  blank or broken canvas.

## Manual Testing Focus

Start a local Ekat match or debug fixture and confirm ability objects render only when they are in
the authoritative snapshot. Arm the relevant ability previews and confirm the cursor preview draws
multiple origins without sending commands by itself.

## Handoff Expectations

The handoff must describe the client state field, renderer entry point, preview descriptor shape,
tests added, and any placeholder visual limitations Phase 5 or Phase 8 should improve.
