# Long-Term Fixes

These are not immediate MVP blockers based on the current review: the core loop is coherent, the
simulation tests pass, and none of these issues prevent continuing product iteration. They are
worth tracking because they sit near important server-authoritative boundaries: placement,
pathing, fog/event visibility, deterministic replay, and module ownership. Most should be fixed
when touching the adjacent system anyway, or before scaling match complexity, larger maps, more
units, or competitive/multiplayer hardening.

## Findings

- [ ] P0: Same-tick builds can overlap.
  `systems.rs` builds occupancy/spatial once before applying all queued commands. A successful
  `Build` command spawns a construction entity, but later `Build` commands in the same tick still
  validate against the pre-command indexes. Add tick-local placement reservations or refresh/overlay
  placement state after each successful build. Add a regression for two overlapping builds issued in
  one tick.

- [ ] P0: `footprint_placeable` can miss overlaps with existing buildings.
  Placement uses a spatial query over entity centers inside the proposed footprint rectangle. A
  building can overlap the proposed footprint while its center is outside that rectangle. Validate
  against an occupancy grid or expand the query by max footprint radius before checking exact
  footprint intersections. Add partial-overlap placement tests.

- [ ] P0: Pathing can snap final waypoints into blocked target tiles.
  The pathfinder retargets blocked goals to a nearby passable tile, but callers overwrite the final
  waypoint with the original target point. No-path fallbacks can also create a direct waypoint to a
  blocked goal. Add an explicit "approach target" pathing API that returns a passable interaction
  point near buildings/nodes/units, and make movement validate full-step/arrival landings.

- [ ] P1: Separation can push units into impassable terrain or building footprints.
  Movement checks passability for partial waypoint steps, but the separation pass only clamps world
  bounds. Apply separation through the same terrain/occupancy guard, or reject/scale pushes whose
  destination tile is blocked.

- [ ] P1: Fog/event visibility has an edge case when weapon range exceeds sight.
  Buildings acquire targets out to weapon range, and owned attackers can expose `target_id` even if
  the target is outside the viewer's current fog. Either constrain acquisition by sight/fog, or
  require target visibility before serializing `target_id`/events. Add a bunker range-vs-sight
  regression.

- [ ] P1: Produced units can spawn into blocked tiles.
  Production always spawns below the building without consulting terrain, occupancy, or nearby
  buildings. Search nearby passable spawn tiles around the footprint, or hold completed production
  until space opens.

- [ ] P2: Replay determinism is not fully enforced around path-cache eviction.
  Path cache eviction chooses the oldest `last_used`, but equal-age ties depend on `HashMap`
  iteration order. Use a deterministic tie-break such as `(last_used, key)` or a stable LRU
  structure.

- [ ] P2: The intended `Game` seam is not enforced by Rust visibility.
  Several game internals are public modules even though the lobby/networking layer is supposed to
  interact only through `Game`. Make internals `pub(crate)` or private where possible, leaving only
  the intended external test/replay surface public.

- [ ] P3: Snapshot cadence config is dead.
  `SNAPSHOT_EVERY_N_TICKS` exists but the lobby currently sends snapshots every tick. Either wire
  the cadence into the room tick loop or delete the constant.

## Recommended Fix Order

- [ ] Fix same-tick build reservations and robust footprint overlap validation.
- [ ] Add pathing "approach target" semantics and harden movement passability.
- [ ] Tighten fog/target visibility rules for combat acquisition and serialized events.
- [ ] Add production spawn placement search.
- [ ] Make path-cache eviction deterministic.
- [ ] Clean up game module visibility around the `Game` API seam.
- [ ] Wire or remove snapshot cadence configuration.
