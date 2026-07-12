# Phase 5 - Playable Fogged Vertical Slice

## Phase Status

- [ ] Not started.

## Depends On

- Phase 4 merged with an opt-in Lab Babylon kernel that consumes the shared presentation frame.

## Objective

Make one explicitly requested Babylon match playable enough to compare with Pixi. Cover the
minimum player-facing spine—authoritative current/explored fog, visible generic entities, semantic
selection, marquee, and basic movement feedback—then stop for a real playtest.

## Work

- Render received `fogGatedWorld` records with truthful generic shapes and team identity. A missing
  model is a visible placeholder, not a reason to create an asset catalog or a bespoke GLB pipeline.
- Render the frame's authoritative current/explored fog. Remembered buildings, below-fog intel,
  reveals, particles, shadows, and long-tail overlays remain absent unless one is required by this
  narrow slice.
- Use the selected Babylon semantic camera's perspective projection and `SelectionSceneV1` for
  click, marquee, and ground move interaction. Do not reuse Pixi's orthographic projection when
  Babylon perspective is visible. Babylon meshes neither choose targets nor alter command
  coordinates.
- Add only the feedback needed to understand basic selection and a move order. Reuse the shared
  HUD/minimap/audio surfaces; do not reproduce their Pixi world decorations.
- Add one focused real two-recipient test with a never-authorized entity/position sentinel. It must
  be absent from Babylon rendering, picking, and diagnostics. This is the security check that earns
  an opt-in live path; it is not an excuse for a broad fog/replay certification program.
- Enable an explicit Babylon live player route and retain the explicit Lab route after the above
  passes. Pixi remains default. Replay and spectator routes stay Pixi unless their support falls out
  naturally without delaying the slice.
- Capture one fogged authoritative Lab scene with `lab-interact`, expose a Tailscale preview when a
  local visual review is useful, and obtain a user playtest before proposing more work.

## Explicit Exclusions

- No default switch, Pixi deletion, faction conversion, final art, or parity claim.
- No retained-event replay/capture, effect catalog, full overlay catalog, remembered/reveal visuals,
  automated ten-cycle lifecycle gate, pool/registry architecture, benchmark budgets, vegetation,
  shadows, or quality tiers.
- No forced replay/spectator support or device rollout matrix.

## Acceptance

- An explicitly selected live/Lab Babylon match shows only authoritative visible data and current/
  explored fog, with generic units/buildings clearly distinguishable by team.
- Selection, marquee, and basic move commands retain the existing semantic behavior under the
  perspective camera; the focused two-recipient sentinel test passes.
- Pixi remains default and functional. Babylon is described honestly as an experimental pre-alpha
  renderer with known visual gaps.
- A user can play or observe the slice and identify the next concrete limitation.

## Expected Touch Points

- Babylon ground/fog/generic-entity and minimal feedback modules
- backend route gate and existing input/selection integration through public semantic APIs
- focused client and real two-recipient integration coverage
- durable rendering/parity docs and this phase status

## Verification

Run focused fog, selection, backend-route, and two-recipient checks added with the implementation,
then:

    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Play a short explicit Babylon live match: pan, select, marquee-select, issue moves, and cross a fog
edge. Compare the same actions with Pixi, then leave/re-enter once. Review the captured Lab scene
and collect the first player-facing limitation rather than expanding scope during the phase.

## Handoff Expectations

Report the supported Babylon path, known visual gaps, no-leak assertion, interaction evidence,
capture/preview path, and the playtest result. This plan ends here: any next work needs a new,
small plan based on the observed limitation.
