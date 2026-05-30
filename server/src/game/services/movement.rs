use crate::config;
use crate::game::entity::{
    BuildPhase, Entity, EntityKind, EntityStore, GatherPhase, MovePhase, Order,
};
use crate::game::map::{Map, MobilityClass};
use crate::game::pathfinding::Passability;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;

/// World pixels at which a unit is considered "arrived" at a waypoint / target point.
const ARRIVE_EPS: f32 = 2.0;

/// Largest unit collision radius in the game (tank, see `config::unit_stats`). Used to size
/// the broad-phase bounding-box query in [`resolve_collisions`] so the spatial index never
/// misses an overlapping neighbor.
const MAX_UNIT_RADIUS_PX: f32 = 26.0;

/// Extra slack added to the broad-phase query so small per-pass position drift never causes a
/// missed pair. One tile is generous: the largest per-tick displacement is bounded by speed
/// (~2 px) plus a single push (≤ overlap distance), both well under a tile.
const COLLISION_SEARCH_SLACK_PX: f32 = config::TILE_SIZE as f32;

/// Maximum number of pair-resolution passes per tick. Each pass pushes overlapping pairs apart
/// by the full violation; with the 50/50 split, two-body cases converge in one pass and dense
/// clusters typically converge in 2–3.
const COLLISION_PASSES: usize = 4;

/// Pairs whose center distance is at least `sum_radii - COLLISION_EPS_PX` are considered
/// non-overlapping. Avoids endless micro-pushes from floating-point noise.
const COLLISION_EPS_PX: f32 = 0.001;

/// Advance every moving unit along its waypoint path at its speed. Clamps the final landing
/// tile to passable terrain (soft overlap with other units is allowed, so we don't resolve
/// unit-unit collisions here). Arriving at the last waypoint of a plain Move clears the order.
pub(crate) fn movement_system(map: &Map, entities: &mut EntityStore, occ: &Occupancy) {
    for id in entities.ids() {
        // Pull the data we need, then mutate.
        let (speed, mut x, mut y, class) = {
            let e = match entities.get(id) {
                Some(e) if e.is_unit() && !e.path_is_empty() => e,
                _ => continue,
            };
            let speed = config::unit_stats(e.kind).map(|s| s.speed).unwrap_or(0.0);
            let class = MobilityClass::from_kind(e.kind);
            (speed, e.pos_x, e.pos_y, class)
        };
        if speed <= 0.0 {
            continue;
        }

        let mut budget = speed;
        let mut new_facing = None;
        // Consume waypoints (stored reversed, next = last element) within this tick's budget.
        loop {
            let next = {
                let Some(e) = entities.get(id) else { break };
                e.next_waypoint()
            };
            let Some((wx, wy)) = next else { break };
            let dx = wx - x;
            let dy = wy - y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= ARRIVE_EPS {
                // Reached this waypoint exactly; drop it and continue with the remaining budget.
                if let Some(e) = entities.get_mut(id) {
                    e.pop_waypoint();
                    e.mark_move_phase(MovePhase::Moving);
                }
                x = wx;
                y = wy;
                continue;
            }
            new_facing = Some(dy.atan2(dx));
            if dist <= budget {
                // We can reach this waypoint this tick.
                x = wx;
                y = wy;
                budget -= dist;
                if let Some(e) = entities.get_mut(id) {
                    e.pop_waypoint();
                    e.mark_move_phase(MovePhase::Moving);
                }
            } else {
                // Partial step toward the waypoint.
                let nx = x + dx / dist * budget;
                let ny = y + dy / dist * budget;
                // Clamp landing to a passable tile for this unit's class.
                if tile_passable_at(occ, map, class, nx, ny) {
                    x = nx;
                    y = ny;
                }
                break;
            }
        }

        if let Some(e) = entities.get_mut(id) {
            e.pos_x = x.clamp(0.0, map.world_size_px() - 0.01);
            e.pos_y = y.clamp(0.0, map.world_size_px() - 0.01);
            if let Some(f) = new_facing {
                e.set_facing(f);
            }
            // A plain Move with an empty path has arrived → go idle.
            if e.path_is_empty() {
                e.mark_move_phase(MovePhase::Arrived);
                if matches!(e.order(), Order::Move(_)) {
                    e.set_order(Order::Idle);
                }
            }
        }
    }
}

/// Whether a world point lands on a passable tile for the given mobility class
/// (terrain + building footprint).
fn tile_passable_at(occ: &Occupancy, map: &Map, class: MobilityClass, x: f32, y: f32) -> bool {
    let (tx, ty) = map.tile_of(x, y);
    map.is_passable_for(class, tx as i32, ty as i32) && occ.passable(tx as i32, ty as i32)
}

/// Resolve unit-unit overlaps with iterative pair-wise pushes so units do not stack on top of
/// each other. For each overlapping pair the push is taken along the line connecting their
/// centers; both sides move by half the overlap.
///
/// **Mining-worker exception (PLAN §4.3).** Workers in [`GatherPhase::Harvesting`] are latched
/// onto their resource node and are *fully exempt* from collision: they neither push nor are
/// pushed. This is intentional — walking units (workers en route to other patches, soldiers
/// transiting through a mining cluster) must be able to pass through a harvester without
/// being kicked backward each tick, which would deadlock the economy. The exemption rides on
/// the worker's gather-order state machine, never on the entity kind, so movement code stays
/// free of kind-specific hacks.
///
/// Pushes that would land on impassable terrain or a building footprint are skipped, so a
/// unit cornered by terrain may keep a small residual overlap. The invariant
/// [`Game::assert_invariants`] tolerates ≤ `OVERLAP_TOLERANCE_PX` of overlap to absorb this
/// and floating-point noise.
///
/// Pair iteration is deterministic (sorted ids, then spatial-index order, both stable per
/// tick), which is required by the replay harness.
pub(crate) fn resolve_collisions(
    entities: &mut EntityStore,
    spatial: &SpatialIndex,
    map: &Map,
    occ: &Occupancy,
) {
    let world_max = map.world_size_px() - 0.01;

    for _pass in 0..COLLISION_PASSES {
        let mut moved_any = false;
        let ids = entities.ids();

        for &a in &ids {
            // Skip anchored units entirely (PLAN §4.3 mining-worker exception): they neither
            // push nor are pushed. Other units can transit through their position freely.
            let (ar, a_class) = match entities.get(a) {
                Some(e) if e.is_unit() && !is_collision_anchored(e) => {
                    (e.radius(), MobilityClass::from_kind(e.kind))
                }
                _ => continue,
            };
            let (ax_idx, ay_idx) = match entities.get(a) {
                Some(e) => (e.pos_x, e.pos_y),
                None => continue,
            };

            // Broad-phase: collect candidate neighbor ids using the (possibly stale) spatial
            // index plus a one-tile slack so small intra-tick drift never hides an overlap.
            let search_r = ar + MAX_UNIT_RADIUS_PX + COLLISION_SEARCH_SLACK_PX;
            let candidates: Vec<u32> = spatial
                .ids_in_circle_bbox(ax_idx, ay_idx, search_r)
                .filter(|&b| b > a)
                .collect();

            for b in candidates {
                let (br, b_class, bx, by) = match entities.get(b) {
                    Some(e) if e.is_unit() && !is_collision_anchored(e) => (
                        e.radius(),
                        MobilityClass::from_kind(e.kind),
                        e.pos_x,
                        e.pos_y,
                    ),
                    _ => continue,
                };
                // Re-read A so we account for displacement applied by earlier pairs in this pass.
                let (ax, ay) = match entities.get(a) {
                    Some(e) => (e.pos_x, e.pos_y),
                    None => break,
                };

                let dx = bx - ax;
                let dy = by - ay;
                let min_d = ar + br;
                let d2 = dx * dx + dy * dy;
                if d2 + COLLISION_EPS_PX >= min_d * min_d {
                    continue;
                }

                let (nx, ny, dist) = if d2 < 1.0e-4 {
                    // Exactly coincident centers: pick a deterministic axis so the resolution
                    // is reproducible across runs and replays.
                    (1.0, 0.0, 0.0)
                } else {
                    let d = d2.sqrt();
                    (dx / d, dy / d, d)
                };
                // Both sides are non-anchored at this point: take a symmetric half-overlap
                // push along the connecting line. If one side's push lands on impassable
                // terrain or a building footprint, the other side absorbs the full overlap
                // so the pair still separates rather than getting stuck.
                let overlap = min_d - dist;
                let a_target = (ax - nx * overlap * 0.5, ay - ny * overlap * 0.5);
                let b_target = (bx + nx * overlap * 0.5, by + ny * overlap * 0.5);
                let a_ok = stays_on_passable(map, occ, a_class, a_target.0, a_target.1);
                let b_ok = stays_on_passable(map, occ, b_class, b_target.0, b_target.1);

                let (a_push, b_push) = match (a_ok, b_ok) {
                    (true, true) => (Some(a_target), Some(b_target)),
                    (true, false) => {
                        let a_full = (ax - nx * overlap, ay - ny * overlap);
                        (
                            if stays_on_passable(map, occ, a_class, a_full.0, a_full.1) {
                                Some(a_full)
                            } else {
                                Some(a_target)
                            },
                            None,
                        )
                    }
                    (false, true) => {
                        let b_full = (bx + nx * overlap, by + ny * overlap);
                        (
                            None,
                            if stays_on_passable(map, occ, b_class, b_full.0, b_full.1) {
                                Some(b_full)
                            } else {
                                Some(b_target)
                            },
                        )
                    }
                    (false, false) => (None, None),
                };

                if let Some((nax, nay)) = a_push {
                    if let Some(e) = entities.get_mut(a) {
                        e.pos_x = nax.clamp(0.0, world_max);
                        e.pos_y = nay.clamp(0.0, world_max);
                        moved_any = true;
                    }
                }
                if let Some((nbx, nby)) = b_push {
                    if let Some(e) = entities.get_mut(b) {
                        e.pos_x = nbx.clamp(0.0, world_max);
                        e.pos_y = nby.clamp(0.0, world_max);
                        moved_any = true;
                    }
                }
            }
        }

        if !moved_any {
            break;
        }
    }
}

/// Whether this unit is currently latched to a fixed point and must not be pushed by
/// collision. The only anchored case today is a worker actively harvesting a resource node;
/// the exemption rides on the worker's gather-order state machine so movement code never
/// needs to special-case the kind (PLAN §4.3).
pub(crate) fn is_collision_anchored(e: &Entity) -> bool {
    if e.kind == EntityKind::Worker && e.gather_phase() == Some(GatherPhase::Harvesting) {
        return true;
    }
    // A worker constructing a building is similarly latched: it must hold position next to
    // the site to keep advancing construction. Without this exemption, other workers passing
    // through can shove the builder out of `interact_range` of the site and the building
    // never finishes.
    if e.kind == EntityKind::Worker
        && matches!(e.build_phase(), Some(BuildPhase::Constructing { .. }))
    {
        return true;
    }
    false
}

/// Whether a world point lies on a tile that's passable terrain for `class` and free of
/// building footprints, i.e. the kind of place a unit may legally stand after a push.
fn stays_on_passable(map: &Map, occ: &Occupancy, class: MobilityClass, x: f32, y: f32) -> bool {
    let (tx, ty) = map.tile_of(x, y);
    map.is_passable_for(class, tx as i32, ty as i32) && occ.passable(tx as i32, ty as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::EntityStore;
    use crate::game::map::Map;
    use crate::game::services::move_coordinator::MoveCoordinator;
    use crate::game::services::pathing::PathingService;

    /// Distance (px) between two entity centers.
    fn dist(entities: &EntityStore, a: u32, b: u32) -> f32 {
        let ea = entities.get(a).unwrap();
        let eb = entities.get(b).unwrap();
        let dx = ea.pos_x - eb.pos_x;
        let dy = ea.pos_y - eb.pos_y;
        (dx * dx + dy * dy).sqrt()
    }

    /// A grass-only test map: the seeded generator scatters obstacles, so for clean
    /// movement/collision experiments we flatten the terrain after generation.
    fn flat_map(player_count: usize) -> Map {
        let mut map = Map::generate(player_count, 0xC0FF_EE01);
        for v in &mut map.terrain {
            *v = crate::protocol::terrain::GRASS;
        }
        map
    }

    /// Two riflemen spawned right on top of each other are pushed apart to non-overlap by
    /// a single tick of `resolve_collisions`.
    #[test]
    fn coincident_units_are_separated_in_one_tick() {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        // Spawn both units at the exact same position so the resolver must use its
        // deterministic-axis fallback.
        let (cx, cy) = map.tile_center(20, 20);
        let a = entities
            .spawn_unit(1, EntityKind::Rifleman, cx, cy)
            .unwrap();
        let b = entities
            .spawn_unit(1, EntityKind::Rifleman, cx, cy)
            .unwrap();

        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        resolve_collisions(&mut entities, &spatial, &map, &occ);

        let ra = entities.get(a).unwrap().radius();
        let rb = entities.get(b).unwrap().radius();
        let d = dist(&entities, a, b);
        assert!(
            d + COLLISION_EPS_PX >= ra + rb,
            "expected at least {:.1}px separation after collision, got {:.3}",
            ra + rb,
            d
        );
    }

    /// Slightly-overlapping units (centers closer than radius sum) are pushed apart in one
    /// tick — both move by half the overlap.
    #[test]
    fn overlapping_units_are_pushed_apart_symmetrically() {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        // Riflemen have radius 9, so spawning at 10 px apart leaves an 8 px overlap.
        let (cx, cy) = map.tile_center(20, 20);
        let a = entities
            .spawn_unit(1, EntityKind::Rifleman, cx - 5.0, cy)
            .unwrap();
        let b = entities
            .spawn_unit(1, EntityKind::Rifleman, cx + 5.0, cy)
            .unwrap();

        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        resolve_collisions(&mut entities, &spatial, &map, &occ);

        let ra = entities.get(a).unwrap().radius();
        let rb = entities.get(b).unwrap().radius();
        let d = dist(&entities, a, b);
        assert!(
            d + COLLISION_EPS_PX >= ra + rb,
            "expected at least {:.1}px separation after collision, got {:.3}",
            ra + rb,
            d
        );
        // Each unit moved roughly half the overlap (4 px each from the 8 px violation).
        let ax = entities.get(a).unwrap().pos_x;
        let bx = entities.get(b).unwrap().pos_x;
        assert!(
            ax < cx - 5.0 && bx > cx + 5.0,
            "expected both units pushed outward (a {:.2}, b {:.2}, center {:.2})",
            ax,
            bx,
            cx
        );
    }

    /// A harvesting worker is fully exempt from collision (PLAN §4.3): it must not be pushed
    /// by another unit overlapping it, *and* it must not push other units away. Walking units
    /// have to be able to transit through a harvester's space without being kicked back —
    /// otherwise the economy deadlocks as miners stack up around resource patches.
    #[test]
    fn harvesting_worker_is_fully_exempt_from_collision() {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        let (cx, cy) = map.tile_center(20, 20);
        let node = entities.spawn_node(EntityKind::Steel, cx, cy).unwrap();
        let worker = entities.spawn_unit(1, EntityKind::Worker, cx, cy).unwrap();
        // Latch the worker as if it were actively harvesting the node.
        {
            let w = entities.get_mut(worker).unwrap();
            w.set_order(Order::gather(node));
            w.mark_gather_phase(GatherPhase::Harvesting);
        }
        // Tank overlaps the worker.
        let tank_x = cx + 4.0;
        let tank_y = cy;
        let tank = entities
            .spawn_unit(2, EntityKind::Tank, tank_x, tank_y)
            .unwrap();
        let worker_before = (
            entities.get(worker).unwrap().pos_x,
            entities.get(worker).unwrap().pos_y,
        );

        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        resolve_collisions(&mut entities, &spatial, &map, &occ);

        let worker_after = (
            entities.get(worker).unwrap().pos_x,
            entities.get(worker).unwrap().pos_y,
        );
        let tank_after = (
            entities.get(tank).unwrap().pos_x,
            entities.get(tank).unwrap().pos_y,
        );
        assert_eq!(
            worker_before, worker_after,
            "harvesting worker must not be displaced by collision"
        );
        assert_eq!(
            (tank_x, tank_y),
            tank_after,
            "tank must not be pushed by an anchored harvester — anchored units are fully exempt"
        );
    }

    /// Two walking workers stacked on a harvester are still separated from each other even
    /// though they both pass through the harvester. This pins down the boundary of the
    /// exception: anchored units are skipped, every other pair is still resolved.
    #[test]
    fn walking_workers_separate_around_anchored_harvester() {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        let (cx, cy) = map.tile_center(20, 20);
        let node = entities.spawn_node(EntityKind::Steel, cx, cy).unwrap();
        let harvester = entities.spawn_unit(1, EntityKind::Worker, cx, cy).unwrap();
        {
            let w = entities.get_mut(harvester).unwrap();
            w.set_order(Order::gather(node));
            w.mark_gather_phase(GatherPhase::Harvesting);
        }
        let walker_a = entities
            .spawn_unit(1, EntityKind::Worker, cx - 4.0, cy)
            .unwrap();
        let walker_b = entities
            .spawn_unit(1, EntityKind::Worker, cx + 4.0, cy)
            .unwrap();

        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        resolve_collisions(&mut entities, &spatial, &map, &occ);

        let ra = entities.get(walker_a).unwrap().radius();
        let rb = entities.get(walker_b).unwrap().radius();
        let d = dist(&entities, walker_a, walker_b);
        assert!(
            d + COLLISION_EPS_PX >= ra + rb,
            "walking workers should be separated even when sharing a harvester's tile (dist {:.2}, min {:.1})",
            d,
            ra + rb
        );
    }

    /// A group move where every unit is ordered to the same point still ends with all units
    /// at non-overlapping positions. Drives the full tick pipeline (path → movement → collision).
    #[test]
    fn group_move_to_one_point_settles_without_overlap() {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        let mut ids = Vec::new();
        for i in 0..8u32 {
            // Spread the starting positions across one row so the initial layout has no overlap.
            let (sx, sy) = map.tile_center(8 + i, 20);
            ids.push(
                entities
                    .spawn_unit(1, EntityKind::Rifleman, sx, sy)
                    .unwrap(),
            );
        }

        let (gx, gy) = map.tile_center(30, 30);
        let mut pathing = PathingService::new(8_192, 256);

        // Run enough ticks for everyone to reach the cluster and settle. Movement speed for a
        // rifleman is 1.6 px/tick and the goal is well inside the map.
        for tick in 1..=400 {
            pathing.advance_tick(tick);
            let occ = Occupancy::build(&map, &entities);
            let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, tick);
            if tick == 1 {
                coordinator.order_group_move(&mut entities, 1, &ids, (gx, gy), false);
                coordinator.process_awaiting_paths(&mut entities);
            }
            movement_system(&map, &mut entities, &occ);
            let spatial = SpatialIndex::build(&entities, map.size);
            resolve_collisions(&mut entities, &spatial, &map, &occ);
        }

        // After settling, no pair of units overlaps by more than the invariant tolerance.
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a = ids[i];
                let b = ids[j];
                let ra = entities.get(a).unwrap().radius();
                let rb = entities.get(b).unwrap().radius();
                let d = dist(&entities, a, b);
                assert!(
                    d + 2.0 >= ra + rb,
                    "group-move settle: units {} and {} overlap by {:.2}px",
                    a,
                    b,
                    ra + rb - d
                );
            }
        }
    }

    /// Even when the ordered goal is occupied by another unit, the move order must still
    /// resolve cleanly: the mover arrives near the goal and the two non-anchored units do
    /// not stack on top of each other.
    #[test]
    fn move_to_occupied_tile_does_not_stack() {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        let (gx, gy) = map.tile_center(25, 25);
        let stationary = entities
            .spawn_unit(1, EntityKind::Rifleman, gx, gy)
            .unwrap();
        let (sx, sy) = map.tile_center(15, 25);
        let mover = entities
            .spawn_unit(1, EntityKind::Rifleman, sx, sy)
            .unwrap();

        let mut pathing = PathingService::new(8_192, 256);
        for tick in 1..=300 {
            pathing.advance_tick(tick);
            let occ = Occupancy::build(&map, &entities);
            let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, tick);
            if tick == 1 {
                coordinator.order_group_move(&mut entities, 1, &[mover], (gx, gy), false);
                coordinator.process_awaiting_paths(&mut entities);
            }
            movement_system(&map, &mut entities, &occ);
            let spatial = SpatialIndex::build(&entities, map.size);
            resolve_collisions(&mut entities, &spatial, &map, &occ);
        }

        let d = dist(&entities, mover, stationary);
        let ra = entities.get(mover).unwrap().radius();
        let rb = entities.get(stationary).unwrap().radius();
        assert!(
            d + 2.0 >= ra + rb,
            "mover and stationary unit must not stack (dist {:.2}, min {:.1})",
            d,
            ra + rb
        );
    }
}
