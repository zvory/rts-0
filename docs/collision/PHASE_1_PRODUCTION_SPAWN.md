# Phase 1 - Production Spawn Correctness

Goal: production never creates a unit at an illegal or occupied spawn point.

This phase fixes the tank factory failure directly, but does it by using the Phase 0 standability
authority instead of adding another local spawn heuristic.

## Dependencies

- Phase 0 standability helpers.

## Scope

In scope:

- Make spawn search check unit body clearance against terrain, buildings, and existing units.
- Remove forced fallback spawning into invalid space.
- Keep completed production queued when no valid spawn point exists.
- Add regression tests for repeated tank production and surrounded exits.

Out of scope:

- No production rally commands.
- No building placement changes.
- No pathfinding rewrite.
- No dynamic global obstacle maps.

## Files To Touch

- `server/src/game/services/production.rs`
- `server/src/game/services/move_coordinator.rs` or a new spawn helper under `standability.rs`
- `server/src/game/services/standability.rs`
- `server/src/game/entity.rs` only if production queue state needs a small accessor.
- `DESIGN.md` if production blocking semantics are documented.

## Design

Production should be a two-step transaction:

```text
if front item is complete:
    find legal spawn point with Spawn policy
    if found:
        remove queue item
        spawn unit
    else:
        keep front item complete
        retry next tick
```

Do not remove the queue item before finding a legal point. This prevents paid-for units from
disappearing when every exit is blocked.

Use `Spawn` standability:

- Unit body fits inside world bounds.
- Unit body does not intersect impassable terrain.
- Unit body does not intersect any building footprint.
- Unit body does not intersect any living unit body, including ghost workers.

## Spawn Search

Keep deterministic ring search around the producer footprint, but replace local checks with:

```rust
standability::unit_spawn_standable(map, occ, entities, spawned_kind, cx, cy)
```

Important details:

- Search around the actual building footprint, not only the center tile.
- Prefer ring order that is stable across runs.
- If multiple production buildings complete in the same tick, either:
  - spawn immediately so later buildings see the newly spawned unit through `entities`, or
  - maintain a per-production-tick reservation set.
- Do not rely on a stale spatial index for newly spawned units unless the index is rebuilt or the
  reservation set covers it.
- If no legal point is found, return `None`.

## Player Feedback

Avoid notice spam. A blocked production queue should not emit a notice every tick.

Acceptable options:

- No notice for now.
- Emit one notice when the item first becomes blocked, if queue state grows a blocked flag.

Keep this phase simple unless product direction asks for UI feedback.

## Tests

Add Rust tests:

- `tank_spawn_search_avoids_occupied_exit`: place a tank on the first factory exit and assert the
  next spawn point differs and is legal.
- `tank_production_waits_when_all_exits_blocked`: complete a tank queue with exits blocked and
  assert no unit is spawned and the queue item remains complete.
- `blocked_tank_production_spawns_after_exit_clears`: unblock an exit on a later tick and assert the
  queued tank appears.
- `multiple_factories_do_not_spawn_units_on_same_point_in_one_tick`: two completed factories near
  each other produce distinct legal unit positions.
- `spawn_search_rejects_body_clipping_adjacent_building`: radius clearance, not center tile only.

Run:

```bash
cd server && cargo fmt && cargo test production::tests move_coordinator::tests standability
cd server && cargo test
```

## Acceptance Criteria

- Production never births a tank on top of another unit.
- Production never forces a unit into terrain, map edges, or building body overlap.
- Blocked production queues recover automatically once space opens.
- Supply and cost reservation semantics stay unchanged.
- The tick path remains deterministic and panic-free.
