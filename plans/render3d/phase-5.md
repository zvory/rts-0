# Phase 5 - Playable Fog and Interaction Slice

## Phase Status

- [ ] Not started.

## Depends On

- Phase 4 merged with an opt-in Babylon Lab kernel, fixed-perspective semantic camera, and tested
  scene/projection conversion.

## Objective

Make an explicitly selected Babylon match playable enough to compare with Pixi. Implement the core
fog, entity, selection, and command-feedback spine, prove the no-leak invariant once end to end, and
then collect a real user playtest before expanding content.

## Work

- Render current/explored fog from the frame's revisioned snapshots. Render remembered buildings,
  below-fog intel, and explicit above-fog reveals only from their already-separated presentation
  layers; never look up a hidden source entity.
- Render every received current entity through a shared truthful generic shape when no asset exists.
  Preserve team identity, facing, construction state, selection, HP/progress, and clear placeholder
  diagnostics without creating per-entity materials or meshes.
- Use the selected Babylon camera's `ProjectionSnapshotV1` and existing `SelectionSceneV1` for
  clicks, marquee, entity targets, and nullable ground commands. Meshes never determine targets.
- Add only feedback needed for a basic match: screen marquee, selection/HP, move destination/line,
  entity-target marker, and building-placement footprint. Reuse shared HUD, minimap, audio, and
  control-group behavior.
- Add one real server-projected two-recipient fixture with a never-authorized sentinel. Assert it is
  absent from the presentation frame, Babylon scene, picking candidates, and diagnostics.
- Enable explicit Babylon live and Lab routes after the focused checks pass. Leave replay and
  spectator routes on Pixi.
- Use `lab-interact` to capture and inspect one fogged playable scene, expose a Tailscale preview
  when useful, and obtain a user playtest.

## Keep Small

- No effect catalog, retained event history, deterministic multi-offset capture, GLB content,
  performance budgets, pooling, vegetation, shadows, quality tiers, replay/spectator parity, or
  default switch.
- One strong secrecy fixture replaces a matrix covering every route/reset/capture combination.

## Acceptance

- An explicit Babylon live/Lab match shows authoritative fog categories and truthful generic
  entities without hidden-data leakage.
- Click, marquee, basic entity targeting, ground moves, and placement use the same perspective
  semantics as the displayed scene.
- Pixi remains the default and retains current behavior.
- The handoff records an actual playtest and whether Phase 6's representative asset/effect is still
  the right next step.

## Verification

Run focused fog, interaction, route, and real two-recipient contracts added by the phase, then:

    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test and Handoff

Play a short Babylon match: navigate, select, marquee, issue move/entity-target commands, place a
building, and cross a fog edge. Compare the same actions with Pixi. Report known visual gaps,
secrecy evidence, interaction results, capture/preview path, and the first concrete playtest
limitation; do not manufacture additional phases from the deferred backlog.
