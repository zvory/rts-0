# Scout Plane Multi-Phase Plan

## Purpose

Implement the Scout Plane described in [requirements.md](requirements.md) as a normal Kriegsia
City Centre production option after its hidden contracts, server behavior, fog safety, and client
readability are complete. The unit is a paid, non-combat aerial scout that launches from a City
Centre, orbits a rally area, reveals fog through terrain and building blockers, consumes oil while
active, and is limited to one active or in-production plane per player. Phase 0 and Phase 1 are
already captured in [requirements.md](requirements.md); the implementation phases below should run
one at a time only after explicit approval to proceed beyond that requirements gate.

## Product Input

- [requirements.md](requirements.md) is the active Scout Plane brief, rules, balance, testing, patch
  note, and non-goal source.
- [docs/new-unit-checklist.md](../../docs/new-unit-checklist.md) remains the workflow checklist for
  adding the unit.

## Phase Summaries

### [Phase 2 - Hidden Vocabulary, Balance, And Protocol Contract](phase-2.md)

Add the shared Scout Plane vocabulary, hidden rules data, and protocol contract before any normal
match can train the unit. This phase should introduce the unit kind, mirrored client-visible numbers,
and any snapshot or command vocabulary needed by later launch, fuel, orbit, and dismiss work. It
must keep Scout Plane out of normal City Centre production, command cards, AI build plans, and
player-facing matches until server runtime and client readability are complete.

### [Phase 3 - Hidden Server Launch And Aerial Movement](phase-3.md)

Implement hidden authoritative server behavior for spawned or directly queued Scout Plane entities.
The server should own launch from a City Centre, direct movement toward the first rally point,
orbiting at the approved radius, queued orbit retargeting, non-colliding aerial movement, and
non-combat command filtering. This phase proves the plane can exist, move, orbit, accept move
orders, ignore ground blockers, and remain tick-safe before fog/upkeep and player UI exposure.

### [Phase 4 - Upkeep, Dismissal, Fog, And Projection](phase-4.md)

Add the oil upkeep, fuel reserve, manual dismiss, automatic dismiss, authoritative aerial vision,
and fog-safe projection rules for hidden Scout Planes. Scout Plane sight should ignore terrain and
building line-of-sight blockers while still respecting smoke where practical, and enemy projection
must reveal the plane only through current visibility. This phase also covers checkpoint, replay,
lab, spectator, and cleanup behavior for hidden server-spawned planes.

### [Phase 5 - Client State, Controls, Rendering, And Lab Inspection](phase-5.md)

Teach the client to parse, select, command, and render hidden Scout Plane entities well enough for
human inspection. Add the plane command card with move/retarget and dismiss affordances, mixed
selection command routing, rough FW 189-inspired visual treatment, minimap/readability support, and
a lab or dev-scenario inspection path. This phase should make a hidden server-created plane readable
and controllable locally, but still not trainable from normal City Centres.

### [Phase 6 - Production Exposure And City Centre Button Behavior](phase-6.md)

Expose the completed Scout Plane through normal Kriegsia City Centre production. The City Centre
command card should show the Scout Plane button in slot `Z`, unlock after completed Gun Works or
Vehicle Works, spend 50 Steel and 50 Oil, build for 600 ticks, enforce the one active or in-production
limit, and select/pan to the existing active plane instead of queueing another. This phase is the
first normal-match exposure point, so it must land only after Phases 2-5 have made the server and
client behavior shippable.

### [Phase 7 - Integration Regression And Playtest Readiness](phase-7.md)

Add the final focused regression matrix for the fully integrated Scout Plane. Cover production,
resource/supply handling, launch, rally targeting, queued retargeting, movement speed, orbit radius,
oil drain/refill, fog through blockers, smoke blocking, enemy visibility, command filtering,
checkpoint/replay/lab/spectator projection, and client UI behavior. This phase should leave reliable
local and CI checks plus a short manual playtest path for judging whether the approved numbers need
later tuning.

### [Phase 8 - Docs, Patch Notes, And Review Package](phase-8.md)

Close documentation drift and prepare the final review package for the Scout Plane feature. Contract
docs still belong in the same phase as each contract change, so this phase is for final alignment,
patch-note cleanup, generated surfaces, known deferred art/audio items, and review ergonomics. A
reviewer should be able to answer what changed, how to try it, what was tested, and what remains
intentionally deferred without reconstructing the feature from code.

## Phase Index

2. [Phase 2 - Hidden Vocabulary, Balance, And Protocol Contract](phase-2.md)
3. [Phase 3 - Hidden Server Launch And Aerial Movement](phase-3.md)
4. [Phase 4 - Upkeep, Dismissal, Fog, And Projection](phase-4.md)
5. [Phase 5 - Client State, Controls, Rendering, And Lab Inspection](phase-5.md)
6. [Phase 6 - Production Exposure And City Centre Button Behavior](phase-6.md)
7. [Phase 7 - Integration Regression And Playtest Readiness](phase-7.md)
8. [Phase 8 - Docs, Patch Notes, And Review Package](phase-8.md)

## Overall Constraints

- Keep [requirements.md](requirements.md) as the product behavior source. If implementation
  discovers a conflict with the requirements, stop as blocked instead of inventing new Scout Plane
  behavior.
- Do not expose Scout Plane in normal production until Phase 6. Earlier phases may add hidden
  vocabulary, tests, internal spawn paths, lab/dev inspection support, parser support, and renderer
  support, but normal players should not be able to train a half-finished plane.
- Preserve the approved unit contract: City Centre production after completed Gun Works or Vehicle
  Works, hotkeys `Z` grid and `S` RTS classic, 50 Steel / 50 Oil, 0 supply, 600-tick build time, 40
  HP, 12-tile sight, 2 px/tick movement, 4-tile orbit radius, one active or in-production plane per
  player, and no combat.
- Keep server authority. The server owns production admission, launch, movement, orbit retargeting,
  queued move semantics, oil upkeep, fuel reserve, dismissal, non-targetability, collision immunity,
  fog stamping, projection, replay/checkpoint state, and AI exclusion.
- Keep `Game::tick()` panic-free. Stale ids, destroyed launch buildings, missing rally points, dead
  planes, empty production queues, invalid coordinates, zero oil, disconnected clients, and malformed
  commands must be safe no-ops or recoverable errors.
- Maintain fog guarantees. Scout Plane vision may reveal terrain and entities through terrain and
  building blockers to the owner/team, but it must not reveal hidden target ids, enemy private order
  data, enemy resources, hidden queued commands, or hidden positions to enemies.
- Keep smoke meaningful. Scout Plane sight should still be blocked by smoke using the existing
  smoke-blocking policy where practical; if implementation needs a narrowed interpretation, update
  [requirements.md](requirements.md) or stop for product review.
- Keep protocol mirrors synchronized across `server/crates/protocol/src/lib.rs`,
  `server/src/protocol.rs`, `server/crates/sim/src/protocol.rs`, `client/src/protocol.js`, compact
  snapshot metadata if touched, and [docs/design/protocol.md](../../docs/design/protocol.md).
- Keep balance and catalog mirrors synchronized across Rust rules, faction catalog exports,
  `client/src/config.js` and its internal mirrors, generated wiki/stats surfaces, and
  [docs/design/balance.md](../../docs/design/balance.md).
- Respect client architecture. New client state, renderer, HUD, input, hotkey, lab, and audio paths
  should use existing dependency-injection patterns and must clean up listeners, timers, textures,
  sounds, and GPU resources.
- Do not add aircraft combat, anti-air weapons, crashes, repair, veterancy, transport, bombing,
  movement blocking, pathfinding collision, or AI production/management unless
  [requirements.md](requirements.md) is explicitly revised first.
- Audio is deferred by the requirements. Do not add accidental, misleading, reused combat, or
  debug-only sounds; if an implementation phase adds intentional audio, document the change and
  patch-note impact.
- Collect factual patch-note bullets during gameplay phases: unlock source, cost, build time, oil
  upkeep, active limit, launch/rally/orbit behavior, fog behavior, smoke interaction, command
  affordances, and expected playtest watch points.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite merge with the phase head reachable from `origin/main`
  before the next phase starts.
- After implementing each phase, the implementing agent must provide a handoff message describing
  what changed, what the next agent should do, and what should be manually tested. Manual testing
  notes should name core gameplay scenarios, not an exhaustive test matrix.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Required Verification Themes

Each phase should run the smallest relevant subset of:

- focused Rust tests for rules definitions, faction catalogs, production prerequisites, one-active
  limits, launch, movement, orbiting, queued retargeting, upkeep, fuel drain/refill, dismissal,
  target rejection, collision immunity, fog projection, checkpoint/replay cleanup, and AI exclusion
- `node tests/protocol_parity.mjs` and focused protocol contract tests after unit-kind, snapshot,
  event, compact transport, command, or parser changes
- `node scripts/check-faction-catalog-parity.mjs` and `node scripts/check-wiki.mjs` after visible
  rules, catalog, command-card, generated stats, or wiki surface changes
- focused client contract tests for HUD command cards, hotkeys, config mirrors, state parsing,
  selection, input command routing, rendering feedback, lab spawn/inspection, minimap behavior, and
  dismiss controls
- `node scripts/check-client-architecture.mjs` after client module, renderer, HUD, input, lab, or
  audio wiring changes
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` after
  sim service boundary or `rts-sim::game` module-ownership changes
- `node scripts/check-docs-health.mjs` for docs-heavy phases
- `git diff --check`

## Suggested Execution

Implement one phase at a time from a clean worktree. Do not start a later phase from an assumed
merge; wait for the owned PR to merge and verify reachability from `origin/main`.

```bash
scripts/phase-runner.sh --plan scoutplane 2 --pr --wait
scripts/phase-runner.sh --plan scoutplane 3 --pr --wait
scripts/phase-runner.sh --plan scoutplane 4 --pr --wait
scripts/phase-runner.sh --plan scoutplane 5 --pr --wait
scripts/phase-runner.sh --plan scoutplane 6 --pr --wait
scripts/phase-runner.sh --plan scoutplane 7 --pr --wait
scripts/phase-runner.sh --plan scoutplane 8 --pr --wait
```
