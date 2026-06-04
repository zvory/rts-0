# Cleanup - Multi-Phase Component Plan

This plan breaks up the largest source files without changing gameplay, protocol behavior, or the
server-authoritative architecture. The goal is not to chase line counts alone. Each phase should
extract cohesive components, preserve public seams, and leave the code easier to reason about under
the invariants in `DESIGN.md`.

## Current Hotspots

Measured source files that are large enough to justify cleanup planning:

- `server/src/game/ai_core/decision.rs` - AI decision policy, expansion, panic defense, raids,
  production, resources, placement, and tests.
- `server/src/game/selfplay.rs` - live self-play, replay artifacts, scripted players, milestone
  assertions, and tests.
- `server/src/game/services/movement.rs` - waypoint advancement, tank drive, local steering,
  collision resolution, terrain standability, and tests.
- `server/src/game/services/combat.rs` - acquisition, chase/standoff, weapon setup, damage, shot
  projection, fog-safe events, and tests.
- `client/src/renderer.js` - Pixi app ownership, terrain, resources, buildings, units, fog,
  feedback, placement, visual helpers, and pooling.
- `server/src/game/mod.rs` - `Game` public API seam, setup, command enqueueing, snapshots, scoring,
  and tests.
- `server/src/game/entity.rs` - entity kind, order state machines, grouped state, entity helpers,
  store, and tests.
- `server/src/lobby.rs` - connection writers, lobby room lifecycle, match loop, snapshots, dev
  replay/self-play, crash replay, and tests.
- `server/src/game/services/move_coordinator.rs` - movement request coordination, staging goals,
  formation spreading, path setup, and tests.

## Architecture Rules

- Keep `DESIGN.md` as the source of truth. Update it in the same implementation change when a
  phase changes a contract, module boundary, or public seam.
- Preserve the `Game` API as the simulation seam for `lobby.rs` and `main.rs`.
- Prefer mechanical extraction first: move cohesive private helpers into sibling modules with
  narrow `pub(super)` or `pub(crate)` APIs before rewriting behavior.
- Keep tests close to the behavior they protect. Large moved test modules are acceptable during
  extraction, but each phase should leave test names and coverage discoverable.
- Do not add circular knowledge between services. Movement, combat, economy, construction, and
  production should stay orchestrated by `systems.rs` and shared through explicit inputs.
- Keep wire protocol and mirrored config changes out of cleanup phases unless a phase explicitly
  requires a contract update.
- Preserve panic-free tick behavior. Extraction must not introduce `unwrap`, unchecked indexing, or
  assumptions about live entity ids on tick paths.
- Treat client modules as dependency-injected collaborators. Do not make client modules cross-import
  each other except for shared protocol/config/style utilities.

## Phases

- [Phase 0 - Baseline and extraction guardrails](PHASE_0.md)
- [Phase 1 - Self-play harness decomposition](PHASE_1.md)
- [Phase 2 - AI decision decomposition](PHASE_2.md)
- [Phase 3 - Entity model decomposition](PHASE_3.md)
- [Phase 4 - Movement service decomposition](PHASE_4.md)
- [Phase 5 - Combat service decomposition](PHASE_5.md)
- [Phase 6 - Game and lobby seam cleanup](PHASE_6.md)
- [Phase 7 - Client renderer and input decomposition](PHASE_7.md)
- [Phase 8 - Final hardening and documentation audit](PHASE_8.md)

## Suggested Order

Start with Phase 0, then Phase 1. `selfplay.rs` is mostly test harness code and is a lower-risk
place to prove the extraction pattern. After that, split server simulation model files before client
files:

1. `selfplay.rs`
2. `ai_core/decision.rs`
3. `entity.rs`
4. `movement.rs` and `move_coordinator.rs`
5. `combat.rs`
6. `game/mod.rs` and `lobby.rs`
7. `renderer.js`, `input.js`, `main.js`, and CSS

Do not run multiple phases in the same change unless the phase is purely mechanical and the diff is
still easy to review. Each implementation PR or direct commit should have one clear ownership
boundary.

