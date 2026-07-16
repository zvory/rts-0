# Playable Babylon Catch-Up

## Goal

Make Babylon the fastest path for normal live-game development by importing the existing
gameplay-relevant Pixi presentation with deliberately crude 3D primitives. "Playable" means a
player can read the battlefield, use every current command and ability, and understand the result
without opening the Pixi renderer. It does not mean attractive art, exact Pixi parity, or a new
renderer feature.

The difficult foundations are already complete: server authority and fog filtering, detached
presentation frames, semantic camera/projection, mesh-independent selection, centralized
world/scene conversion, one Match-owned frame loop, and renderer teardown. Preserve those seams;
do not redesign them while catching up.

## Definition of Playable

A Babylon live match is playable when all of the following are true:

- terrain, obstacles, resource sites, units, buildings, ownership, facing, health, construction,
  production, setup and active ability/weapon status, fuel starvation, trenches, smoke, and ability
  objects are readable at ordinary play distance, even if they are only flat colors, boxes,
  labels, rings, and lines;
- the existing HUD, minimap, audio, selection, control groups, economy, construction, production,
  rally, movement, attack, support-weapon, resource, and targeted-ability flows work without blind
  clicks or unexplained state changes;
- every normal-player gameplay feedback record already supplied to Pixi has a generic Babylon
  representation, including launches/projectiles, targets, impacts, ranges/arcs, placement and
  setup validity, and command results;
- a real live-player match covers economy, construction, production, rallying, mixed-force combat,
  an ability or support weapon, fog transitions, and leave/re-enter without a page, frame, or
  renderer error; and
- existing secrecy, projection, selection, single-loop, and teardown contracts stay green, and a
  required Babylon live browser canary covers the selected route.

## Constraints

- Port existing behavior only. Do not add new gameplay, speculative renderer architecture, or new
  visual design during this plan.
- Primitive representations are the intended result. Prefer a readable ugly marker over an asset
  pipeline, animation system, bespoke effect, or generalized rendering abstraction.
- Babylon must not receive the Pixi compatibility `sources` bag or mutable `GameState`,
  `ClientIntent`, visual-profile, or transport callbacks. Its constructor inputs are limited to
  the Babylon dependency, canvas parent, an intentionally scoped instrumentation hook if retained,
  and the smallest detached static-map delivery seam needed for terrain and resource sites;
  ordinary rendering stays frame-driven.
- Renderer objects never determine selection, commands, ownership, visibility, pathing, or
  simulation state. Missing visuals fall back to bounded generic markers.
- Pixi remains available as an explicit rollback path through the cutover. Replay and spectator
  routes may remain on Pixi.
- Do not create follow-up phases for visual polish. Stop after the live cutover and let actual
  development expose the next useful improvement.

## Phases

### [Phase 1 - Crude World Readability](phase-1.md)

Close the construction boundary leak and deliver detached static terrain/resource data to Babylon.
Render terrain classes, resource sites, and every received entity kind with a small primitive and
label taxonomy that makes identity, owner, facing, weapon/setup state, health, and progress
readable. End with an authoritative mixed-world capture; do not load a GLB or build an asset system.

### [Phase 2 - Crude Gameplay Feedback](phase-2.md)

Render the gameplay-significant world records Babylon currently ignores: trenches, smoke, ability
objects, rallies, ranges/arcs, setup and targeting previews, and the existing command/combat event
catalog. Use a tiny shared vocabulary of lines, rings, wedges, billboards, projectiles, and flashes,
regardless of how Pixi styles each case. End with a short authoritative scenario that exercises
building, movement, combat, and a targeted ability across fog.

### [Phase 3 - Playtest and Live Cutover](phase-3.md)

Stop after Phase 2 and obtain a real user live-player playtest against the definition of playable;
Phase 3 begins only after the user reports blockers or approves cutover. Fix only concrete blockers,
add a required no-selector Babylon browser canary, and make Babylon the normal live-player and Lab
renderer while retaining explicit Pixi rollback and Pixi replay/spectator fallback that does not
load Babylon. Stop the catch-up plan after the cutover rather than starting visual-parity work.

## Explicitly Deferred

- GLB assets, asset descriptors, faction art, Pixi rig or sprite parity, locomotion and recoil
  animation, bespoke weapon particles, shadows, vegetation, lighting polish, and quality tiers;
- cosmetic ground decals, visual-sample tooling, observer analysis, debug overlays, and exact-pixel
  parity unless the Phase 3 playtest proves one blocks normal live development;
- Babylon replay/spectator routes and removal of the Pixi rollback path;
- benchmark schemas, performance certification, pools, registries, reference counting, context-loss
  recovery, WebGPU, device matrices, and release hardening unless measured failure requires them.

## Execution and Handoff

Implement each phase from current `origin/main` in its own clean branch and owned PR, arm
auto-merge, wait for the PR to merge, and verify the phase head is reachable from `origin/main`
before starting the next phase. Mark the phase document done in its implementation commit.

Phases 1 and 2 may run as the initial implementation chain. Stop after Phase 2, share the Babylon
live route, and obtain the user's playtest result; do not launch Phases 1 through 3 as one
unattended range. Phase 3 requires explicit user approval to fix the reported blockers and perform
the default cutover.

After every phase, provide a handoff that names what changed, what the next agent should do, the
focused checks and inspected Interact evidence, and the core manual test for the next phase. The
manual test should cover the phase's main player-facing behavior, not an exhaustive matrix.
