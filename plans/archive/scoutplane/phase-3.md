# Phase 3 - Hidden Server Launch And Aerial Movement

## Phase Status

Status: done.

## Objective

Implement authoritative hidden server behavior for Scout Plane creation, launch, direct aerial
movement, orbiting, and move retargeting. The result should be a server-owned entity that can be
spawned or directly queued in tests, fly without ground pathing, circle a rally/orbit point, and
ignore collision without being exposed to normal players.

## Scope

- Read [docs/context/server-sim.md](../../../docs/context/server-sim.md) before changing sim state,
  services, orders, production, checkpoint, replay, or dev scenario behavior.
- Add durable Scout Plane runtime state under the appropriate `GameState`/entity owner:
  - launch state or active orbit state as needed.
  - current orbit center.
  - orbit phase/angle if needed for deterministic circling.
  - direct movement target while flying from launch point to orbit center.
  - queued future orbit centers using existing queued move semantics where practical.
- Add hidden creation support for tests and later production:
  - A completed plane launches from the producing City Centre's world position.
  - The first launch target is the producing City Centre's first rally point.
  - If no rally point exists, the initial orbit center is the producing City Centre's world position.
  - The active plane persists if the launching City Centre is later destroyed.
  - If the producing City Centre is destroyed before completion, existing production interruption
    behavior remains the rule; do not add special refunds.
- Implement direct aerial movement:
  - 2 world pixels per tick.
  - no ground pathfinding.
  - no terrain, building, unit, or tank-trap collision.
  - no occupancy reservation and no blocking of any ground movement.
  - deterministic orbiting at 4-tile radius once the plane reaches the rally area.
- Implement command filtering for hidden Scout Plane entities:
  - move commands retarget the orbit center.
  - queued move commands append later orbit centers using existing queued-order caps.
  - attack, attack-move, gather, build, repair, setup, hold-position, rally, train, and research
    semantics do not apply to the plane.
  - mixed unit command behavior should be server-safe even before client routing is refined.
- Keep the plane non-combat:
  - no target acquisition.
  - no weapon cooldowns.
  - no projectiles or attack events.
  - no combat target ids in projection.
- Keep all new tick-path logic panic-free and stale-id safe.
- Do not add oil upkeep, fog stamping, enemy projection changes, normal production exposure,
  command-card UI, final rendering, or audio in this phase.

## Expected Touch Points

- `server/crates/sim/src/game/state.rs`
- `server/crates/sim/src/game/derived_state.rs`
- `server/crates/sim/src/game/entity/`
- `server/crates/sim/src/game/services/production.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/order_planner.rs`
- `server/crates/sim/src/game/services/order_execution.rs`
- `server/crates/sim/src/game/services/movement.rs`
- `server/crates/sim/src/game/services/move_coordinator.rs`
- `server/crates/sim/src/game/services/occupancy.rs`
- `server/crates/sim/src/game/services/standability.rs`
- `server/crates/sim/src/game/checkpoint*.rs`
- `server/crates/sim/src/game/replay*.rs`
- `docs/design/server-sim.md` if state ownership, checkpoint, or `Game` API contracts change
- `plans/scoutplane/requirements.md` only if implementation discovers a requirement ambiguity

## Edge Cases To Cover

- Plane launched from a City Centre with no rally point orbits above the City Centre.
- Plane launched from a City Centre with a rally point flies toward that point before orbiting.
- Destroying the launch City Centre after launch does not remove or retarget the active plane.
- Destroying the producing City Centre before completion follows existing production behavior.
- The plane can cross water, stone, buildings, units, and tank traps without pathing or collision.
- Ground units can path through the plane and are not blocked by its body.
- Move retargeting works while the plane is en route and while it is orbiting.
- Queued move retargeting preserves only the approved queue semantics and caps.
- Attack, gather, build, setup, hold-position, and attack-move commands on the plane are safe no-ops
  or filtered into the approved movement behavior.
- Mixed selections cannot use the plane to make land units ignore normal land command rules.
- Stale ids, dead planes, non-finite command coordinates, and removed queued targets do not panic.

## Verification

- Focused Rust tests for hidden spawn/launch from City Centre, no-rally orbit, rally-target launch,
  2 px/tick movement, 4-tile orbit radius, queued retargeting, City Centre destruction after launch,
  production interruption before completion, and collision immunity.
- Focused command tests for move retargeting and rejection/filtering of non-plane commands.
- Focused occupancy/pathing tests proving ground movement ignores the plane.
- Checkpoint/replay round-trip tests if durable state is added.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if sim
  service boundaries or state ownership change.
- `git diff --check`.

## Manual Test Focus

Manual gameplay is optional because the unit remains hidden. If a dev or lab scenario exists after
this phase, inspect one hidden plane crossing blockers, retarget it while en route, queue a second
retarget, and confirm nearby ground units do not path around it.

## Handoff Expectations

Name the final server state shape, creation helper, launch path, orbit math, command filtering
policy, and any intentionally deferred edge cases. Tell Phase 4 exactly where to attach upkeep,
dismissal, fog stamping, projection, checkpoint, and replay behavior.
