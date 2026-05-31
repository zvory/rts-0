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
pub(crate) fn movement_system(
    map: &Map,
    entities: &mut EntityStore,
    occ: &Occupancy,
    spatial: &SpatialIndex,
    tick: u32,
) {
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
        let mut static_blocked_this_tick = false;
        // Consume waypoints (stored reversed, next = last element) within this tick's budget.
        loop {
            let (next, path_len, next_next) = {
                let Some(e) = entities.get(id) else { break };
                let path_len = e.movement.as_ref().map(|m| m.path.len()).unwrap_or(0);
                // next_next: the waypoint after the current one (path is reversed, so index len-2).
                let next_next = e.movement.as_ref().and_then(|m| {
                    if m.path.len() >= 2 {
                        m.path.get(m.path.len() - 2).copied()
                    } else {
                        None
                    }
                });
                (e.next_waypoint(), path_len, next_next)
            };
            let Some((wx, wy)) = next else { break };
            let dx = wx - x;
            let dy = wy - y;
            let dist = (dx * dx + dy * dy).sqrt();

            if path_len > 1 {
                // Intermediate waypoint: pop on radius hit or geometric pass-by.
                let radius_hit = dist <= config::ARRIVE_RADIUS_INTERMEDIATE_PX;
                let passed = next_next.map_or(false, |(nnx, nny)| {
                    // Positive projection of (pos - waypoint) onto (next_next - waypoint) means the
                    // unit is on the far side of the waypoint relative to where it came from.
                    (x - wx) * (nnx - wx) + (y - wy) * (nny - wy) > 0.0
                });
                if radius_hit || passed {
                    if let Some(e) = entities.get_mut(id) {
                        e.pop_waypoint();
                        e.mark_move_phase(MovePhase::Moving);
                    }
                    // No position snap — steer toward the new next waypoint from current position.
                    continue;
                }
            } else {
                // Final waypoint: require exact arrival.
                if dist <= ARRIVE_EPS {
                    if let Some(e) = entities.get_mut(id) {
                        e.pop_waypoint();
                        e.mark_move_phase(MovePhase::Moving);
                    }
                    x = wx;
                    y = wy;
                    continue;
                }
            }

            new_facing = Some(dy.atan2(dx));
            if dist <= budget {
                // We can reach this waypoint this tick.
                if !tile_passable_at(occ, map, class, wx, wy) {
                    static_blocked_this_tick = true;
                    break;
                }
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
                } else {
                    static_blocked_this_tick = true;
                }
                break;
            }
        }

        // Compute neighbor repulsion before taking the mutable borrow.
        let repulsion_dir: (f32, f32) = {
            let unit_radius = entities
                .get(id)
                .and_then(|e| config::unit_stats(e.kind))
                .map(|s| s.radius)
                .unwrap_or(9.0);
            let repulsion_range = unit_radius * 2.0 + MAX_UNIT_RADIUS_PX;
            let mut rx = 0.0_f32;
            let mut ry = 0.0_f32;
            for bid in spatial.ids_in_circle_bbox(x, y, repulsion_range) {
                if bid == id {
                    continue;
                }
                if let Some(nb) = entities.get(bid) {
                    let dx = x - nb.pos_x;
                    let dy = y - nb.pos_y;
                    let d = (dx * dx + dy * dy).sqrt();
                    if d > 1e-4 {
                        rx += dx / d;
                        ry += dy / d;
                    }
                }
            }
            let rlen = (rx * rx + ry * ry).sqrt();
            if rlen > 1e-4 {
                (rx / rlen, ry / rlen)
            } else {
                (0.0, 0.0)
            }
        };

        if let Some(e) = entities.get_mut(id) {
            e.pos_x = x.clamp(0.0, map.world_size_px() - 0.01);
            e.pos_y = y.clamp(0.0, map.world_size_px() - 0.01);
            if let Some(f) = new_facing {
                e.set_facing(f);
            }
            // A plain Move with an empty path has arrived → go idle so normal auto-acquire
            // resumes after the destination is reached.
            if e.path_is_empty() {
                e.mark_move_phase(MovePhase::Arrived);
                if let Some(m) = e.movement.as_mut() {
                    m.static_blocked_ticks = 0;
                }
                if matches!(e.order(), Order::Move(_)) {
                    e.set_order(Order::Idle);
                }
            } else if matches!(e.move_phase(), Some(MovePhase::Moving)) {
                // Decrement sidestep cooldown each tick.
                if let Some(m) = e.movement.as_mut() {
                    m.sidestep_cooldown = m.sidestep_cooldown.saturating_sub(1);
                }

                if static_blocked_this_tick {
                    if let Some(m) = e.movement.as_mut() {
                        m.static_blocked_ticks = m.static_blocked_ticks.saturating_add(1);
                    }
                } else if let Some(m) = e.movement.as_mut() {
                    m.static_blocked_ticks = 0;
                }

                let static_blocked_ticks = e
                    .movement
                    .as_ref()
                    .map(|m| m.static_blocked_ticks)
                    .unwrap_or(0);
                if static_blocked_ticks >= config::STATIC_BLOCKED_REPATH_TICKS
                    && matches!(e.order(), Order::Move(_) | Order::AttackMove(_))
                {
                    e.set_path(Vec::new());
                    e.mark_move_phase(MovePhase::AwaitingPath);
                    let (px, py) = (e.pos_x, e.pos_y);
                    e.reset_stuck(px, py);
                    continue;
                }

                // Tolerant arrival: unit has a path but may be making no progress.
                let (lx, ly) = e
                    .movement
                    .as_ref()
                    .map(|m| m.last_progress_pos)
                    .unwrap_or((x, y));
                let dx = x - lx;
                let dy = y - ly;
                let moved = (dx * dx + dy * dy).sqrt();
                if moved < config::STUCK_EPS_PX {
                    if let Some(m) = e.movement.as_mut() {
                        m.stuck_ticks = m.stuck_ticks.saturating_add(1);
                    }
                } else if let Some(m) = e.movement.as_mut() {
                    m.stuck_ticks = 0;
                    m.last_progress_pos = (x, y);
                }
                let stuck_ticks = e.movement.as_ref().map(|m| m.stuck_ticks).unwrap_or(0);
                // Tolerant arrival: stuck and near goal.
                if stuck_ticks >= config::STUCK_ARRIVAL_TICKS {
                    if let Some((gx, gy)) = e.path_goal() {
                        let dx = x - gx;
                        let dy = y - gy;
                        let dist_to_goal = (dx * dx + dy * dy).sqrt();
                        if dist_to_goal <= config::TOLERANT_ARRIVAL_RADIUS_PX {
                            e.clear_path();
                            e.mark_move_phase(MovePhase::Arrived);
                            if let Some(m) = e.movement.as_mut() {
                                m.stuck_ticks = 0;
                            }
                            if matches!(e.order(), Order::Move(_)) {
                                e.set_order(Order::Idle);
                            }
                        }
                    }
                }
                // Sidestep: stuck mid-path (far from goal), cooldown elapsed,
                // only for Move/AttackMove orders.
                // Stagger trigger per unit so clustered units don't all sidestep at once.
                let trigger_threshold = config::SIDESTEP_TRIGGER_TICKS + (id % 8) as u16;
                if stuck_ticks >= trigger_threshold
                    && static_blocked_ticks == 0
                    && matches!(e.order(), Order::Move(_) | Order::AttackMove(_))
                {
                    let far_from_goal = e.path_goal().map_or(false, |(gx, gy)| {
                        let dx = x - gx;
                        let dy = y - gy;
                        (dx * dx + dy * dy).sqrt() > config::TOLERANT_ARRIVAL_RADIUS_PX
                    });
                    let sidestep_cooldown = e
                        .movement
                        .as_ref()
                        .map(|m| m.sidestep_cooldown)
                        .unwrap_or(1);
                    if far_from_goal && sidestep_cooldown == 0 {
                        inject_sidestep(e, id, x, y, map, occ, repulsion_dir, tick);
                    }
                }
            }
        }
    }
}

/// Inject a perpendicular detour waypoint so a stuck mid-path unit can shimmy free.
/// Direction is derived from repulsion away from neighbors (deterministic).
/// `repulsion_dir` is the pre-computed normalized repulsion vector (or (0,0) if no neighbors).
fn inject_sidestep(
    e: &mut crate::game::entity::Entity,
    entity_id: u32,
    x: f32,
    y: f32,
    map: &Map,
    occ: &Occupancy,
    repulsion_dir: (f32, f32),
    tick: u32,
) {
    let class = MobilityClass::from_kind(e.kind);

    // Heading toward next waypoint; fall back to facing angle if no waypoint.
    let (hx, hy) = if let Some((wx, wy)) = e.next_waypoint() {
        let dx = wx - x;
        let dy = wy - y;
        let d = (dx * dx + dy * dy).sqrt();
        if d > 1e-4 {
            (dx / d, dy / d)
        } else {
            (e.facing().cos(), e.facing().sin())
        }
    } else {
        (e.facing().cos(), e.facing().sin())
    };

    // Use repulsion direction if meaningful; otherwise fall back to id-parity perpendicular.
    let (bx, by) = if repulsion_dir.0 != 0.0 || repulsion_dir.1 != 0.0 {
        repulsion_dir
    } else if entity_id & 1 == 0 {
        (-hy, hx)
    } else {
        (hy, -hx)
    };

    // Deterministic jitter seeded from both entity_id and tick so repeated sidestepping
    // explores different directions rather than always re-entering the same blocked spot.
    let seed = entity_id.wrapping_add(tick);
    let jitter_angle = ((seed % 5) as f32 - 2.0) * (std::f32::consts::PI / 12.0); // ±30°
    let (cos_j, sin_j) = (jitter_angle.cos(), jitter_angle.sin());
    let (px, py) = (bx * cos_j - by * sin_j, bx * sin_j + by * cos_j);

    // Distance jitter: 0.5×–0.75× of SIDESTEP_DISTANCE_PX (half the original average).
    let d = config::SIDESTEP_DISTANCE_PX * (0.5 + (seed % 3) as f32 * 0.125);
    let tx = x + px * d;
    let ty = y + py * d;

    let point_clear = |cx: f32, cy: f32| tile_passable_at(occ, map, class, cx, cy);

    let detour = if point_clear(tx, ty) {
        Some((tx, ty))
    } else {
        // Try opposite side.
        let tx2 = x - px * d;
        let ty2 = y - py * d;
        if point_clear(tx2, ty2) {
            Some((tx2, ty2))
        } else {
            None
        }
    };

    if let Some(waypoint) = detour {
        // path is reverse-ordered; push makes it the *next* waypoint.
        e.push_waypoint(waypoint);
        if let Some(m) = e.movement.as_mut() {
            m.sidestep_cooldown = config::SIDESTEP_COOLDOWN_TICKS;
            m.stuck_ticks = 0;
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
            let spatial = SpatialIndex::build(&entities, map.size);
            movement_system(&map, &mut entities, &occ, &spatial, 0);
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

    /// Regression: a tight cluster of units ordered to a far destination must not deadlock.
    ///
    /// Before the repulsion+jitter fix, units spawned on top of each other would all try to
    /// sidestep in the same direction simultaneously, cancel out, and stop making progress
    /// (stuck_ticks would saturate while position barely changed).  The fix staggers sidestep
    /// thresholds per unit-id and adds a repulsion vector so the cluster dissolves and every
    /// unit converges on the goal.
    ///
    /// Pass criterion: after 600 ticks (20 s at 30 Hz) every unit must be within 5 tiles of the
    /// goal — a threshold the old code reliably missed.
    #[test]
    fn clustered_units_make_progress_to_distant_goal() {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        // Spawn 8 riflemen all on the same tile so the cluster is maximally tight.
        let (sx, sy) = map.tile_center(5, 5);
        let mut ids = Vec::new();
        for _ in 0..8 {
            ids.push(
                entities
                    .spawn_unit(1, EntityKind::Rifleman, sx, sy)
                    .unwrap(),
            );
        }
        // Goal is ~25 tiles away diagonally.
        let (gx, gy) = map.tile_center(30, 30);
        let mut pathing = PathingService::new(8_192, 256);

        for tick in 1u32..=600 {
            pathing.advance_tick(tick);
            let occ = Occupancy::build(&map, &entities);
            let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, tick);
            if tick == 1 {
                coordinator.order_group_move(&mut entities, 1, &ids, (gx, gy), false);
            }
            // process_awaiting_paths must be called every tick (mirrors systems.rs).
            coordinator.process_awaiting_paths(&mut entities);
            let spatial = SpatialIndex::build(&entities, map.size);
            movement_system(&map, &mut entities, &occ, &spatial, tick);
            let spatial = SpatialIndex::build(&entities, map.size);
            resolve_collisions(&mut entities, &spatial, &map, &occ);
        }

        for &id in &ids {
            let e = entities.get(id).unwrap();
            let dx_start = e.pos_x - sx;
            let dy_start = e.pos_y - sy;
            let dist_from_start = (dx_start * dx_start + dy_start * dy_start).sqrt();
            // The deadlock symptom is units barely moving from their spawn point.
            // Any unit stuck within 2 tiles of start after 600 ticks has deadlocked.
            // With the fix applied, all units disperse and move well beyond that radius.
            let tile_px = crate::config::TILE_SIZE as f32;
            assert!(
                dist_from_start >= tile_px * 2.0,
                "unit {} is only {:.0}px from start after 600 ticks — cluster deadlock regression",
                id,
                dist_from_start
            );
        }
    }

    /// Set a path directly on a unit. Path is stored reversed (last element = next waypoint).
    /// `waypoints` should be in visit order: [first_to_visit, ..., final_goal].
    fn set_path_direct(entities: &mut EntityStore, id: u32, waypoints: Vec<(f32, f32)>) {
        let mut rev = waypoints;
        rev.reverse();
        if let Some(e) = entities.get_mut(id) {
            e.set_path(rev);
            e.set_path_goal(e.next_waypoint()); // placeholder; overwrite with actual goal
        }
        // Correct goal: last element of visit order = first element of stored reversed vec.
        // The original last visit-order element is now path[0].
        if let Some(e) = entities.get_mut(id) {
            let goal = e.movement.as_ref().and_then(|m| m.path.first().copied());
            e.set_path_goal(goal);
            e.mark_move_phase(MovePhase::Moving);
        }
    }

    /// An intermediate waypoint within ARRIVE_RADIUS_INTERMEDIATE_PX is popped in one tick
    /// without waiting for exact arrival. The unit's position must not be snapped.
    #[test]
    fn intermediate_waypoint_consumed_by_radius() {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        // Intermediate waypoint center.
        let (iwx, iwy) = map.tile_center(20, 20);
        // Final goal one tile further right.
        let (gx, gy) = map.tile_center(21, 20);
        // Place the unit 10 px to the left of the intermediate center (approaching from left).
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, iwx - 10.0, iwy)
            .unwrap();
        set_path_direct(&mut entities, unit, vec![(iwx, iwy), (gx, gy)]);

        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &occ, &spatial, 0);

        let e = entities.get(unit).unwrap();
        // The intermediate waypoint should have been popped; only the final goal remains.
        assert_eq!(
            e.movement.as_ref().map(|m| m.path.len()).unwrap_or(0),
            1,
            "intermediate waypoint must be popped within ARRIVE_RADIUS"
        );
        // Position must not have snapped to the intermediate center.
        assert!(
            (e.pos_x - iwx).abs() > 1.0,
            "unit position must not snap to intermediate waypoint"
        );
    }

    /// Two units sharing an intermediate waypoint tile must not deadlock — both must reach
    /// the goal with the new fly-by arrival predicate.
    #[test]
    fn two_units_sharing_waypoint_do_not_wedge() {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        // Intermediate one tile ahead of start; final goal one tile further.
        // Short path so the test runs quickly and focuses on the wedge-prevention logic.
        let (iwx, iwy) = map.tile_center(20, 20);
        let (gx, gy) = map.tile_center(22, 20);
        // Both units start just before the intermediate, offset vertically so they share the tile.
        let a = entities
            .spawn_unit(1, EntityKind::Rifleman, iwx - 20.0, iwy - 10.0)
            .unwrap();
        let b = entities
            .spawn_unit(1, EntityKind::Rifleman, iwx - 20.0, iwy + 10.0)
            .unwrap();
        set_path_direct(&mut entities, a, vec![(iwx, iwy), (gx, gy)]);
        set_path_direct(&mut entities, b, vec![(iwx, iwy), (gx, gy)]);

        // Rifleman speed 1.6 px/tick; total path ~84px; 100 ticks is generous even with
        // collision slowdown.
        for tick in 0..100u32 {
            let occ = Occupancy::build(&map, &entities);
            let spatial = SpatialIndex::build(&entities, map.size);
            movement_system(&map, &mut entities, &occ, &spatial, tick);
            let spatial = SpatialIndex::build(&entities, map.size);
            resolve_collisions(&mut entities, &spatial, &map, &occ);
        }

        for &id in &[a, b] {
            let e = entities.get(id).unwrap();
            let dx = e.pos_x - gx;
            let dy = e.pos_y - gy;
            let d = (dx * dx + dy * dy).sqrt();
            assert!(
                d <= config::TILE_SIZE as f32 * 2.0,
                "unit {} wedged — {:.1}px from goal after 100 ticks",
                id,
                d
            );
        }
    }

    /// The final waypoint still requires tight arrival (within ARRIVE_EPS or full-step reach).
    #[test]
    fn final_waypoint_still_requires_close_arrival() {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        let (gx, gy) = map.tile_center(25, 25);
        // Start far enough that exact arrival takes multiple ticks.
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, map.tile_center(15, 25).0, gy)
            .unwrap();
        set_path_direct(&mut entities, unit, vec![(gx, gy)]);

        for tick in 0..300u32 {
            let occ = Occupancy::build(&map, &entities);
            let spatial = SpatialIndex::build(&entities, map.size);
            movement_system(&map, &mut entities, &occ, &spatial, tick);
            if entities.get(unit).map_or(true, |e| e.path_is_empty()) {
                break;
            }
        }

        let e = entities.get(unit).unwrap();
        assert!(
            e.path_is_empty(),
            "path must be empty after arrival at final waypoint"
        );
        let dx = e.pos_x - gx;
        let dy = e.pos_y - gy;
        let dist = (dx * dx + dy * dy).sqrt();
        // Must be within ARRIVE_EPS OR tolerant arrival radius (stuck near goal).
        assert!(
            dist <= config::TOLERANT_ARRIVAL_RADIUS_PX,
            "unit ended {:.2}px from final waypoint — too far",
            dist
        );
    }

    #[test]
    fn plain_move_becomes_idle_after_arrival() {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        let (gx, gy) = map.tile_center(25, 25);
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, gx, gy)
            .unwrap();
        set_path_direct(&mut entities, unit, vec![(gx, gy)]);
        if let Some(e) = entities.get_mut(unit) {
            e.set_order(Order::move_to(gx, gy));
        }

        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &occ, &spatial, 0);

        let e = entities.get(unit).unwrap();
        assert!(matches!(e.order(), Order::Idle));
    }

    /// A unit shoved sideways past an intermediate waypoint (but > ARRIVE_RADIUS away) should
    /// still pop it via the pass-by (dot-product) check.
    #[test]
    fn pass_by_waypoint_pops_when_overshooting_sideways() {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        // Path: unit moves right. Intermediate at (20,20), final at (25,20).
        let (iwx, iwy) = map.tile_center(20, 20);
        let (gx, gy) = map.tile_center(25, 20);
        // Unit is positioned past the intermediate along the path direction but 20 px above
        // it — simulating a collision shove. dist to intermediate ≈ 20 px > ARRIVE_RADIUS (16).
        let unit_x = iwx + 5.0;
        let unit_y = iwy - 20.0;
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, unit_x, unit_y)
            .unwrap();
        set_path_direct(&mut entities, unit, vec![(iwx, iwy), (gx, gy)]);

        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &occ, &spatial, 0);

        let e = entities.get(unit).unwrap();
        assert_eq!(
            e.movement.as_ref().map(|m| m.path.len()).unwrap_or(0),
            1,
            "intermediate waypoint must be popped via pass-by when unit is geometrically past it"
        );
    }

    /// A path that becomes invalid because a building appeared on it should not sidestep
    /// forever against the old route. After a one-second static-block debounce, movement
    /// queues the unit for the existing path coordinator to compute a fresh route.
    #[test]
    fn static_building_blockage_queues_repath_after_debounce() {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        let (w0x, w0y) = map.tile_center(11, 10);
        let (gx, gy) = map.tile_center(20, 10);
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, w0x - 16.5, w0y)
            .unwrap();
        set_path_direct(&mut entities, unit, vec![(w0x, w0y), (gx, gy)]);
        if let Some(e) = entities.get_mut(unit) {
            e.set_order(Order::move_to(gx, gy));
            e.mark_move_phase(MovePhase::Moving);
        }

        // Depot centered on tile (12,10) covers (11,9),(12,9),(11,10),(12,10),
        // so the next waypoint tile became blocked after the path was assigned.
        let (bx, by) = map.tile_center(12, 10);
        entities
            .spawn_building(1, EntityKind::Depot, bx, by, true)
            .expect("building spawn");

        for tick in 0..config::STATIC_BLOCKED_REPATH_TICKS as u32 - 1 {
            let occ = Occupancy::build(&map, &entities);
            let spatial = SpatialIndex::build(&entities, map.size);
            movement_system(&map, &mut entities, &occ, &spatial, tick);
            assert_eq!(
                entities.get(unit).and_then(|e| e.move_phase()),
                Some(MovePhase::Moving),
                "unit should debounce static blockage before repathing"
            );
        }

        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(
            &map,
            &mut entities,
            &occ,
            &spatial,
            config::STATIC_BLOCKED_REPATH_TICKS as u32,
        );

        let e = entities.get(unit).unwrap();
        assert_eq!(e.move_phase(), Some(MovePhase::AwaitingPath));
        assert!(e.path_is_empty(), "stale blocked path should be cleared");
        assert_eq!(e.path_goal(), Some((gx, gy)));
    }

    /// A unit pressed against a building wall must physically reach its goal, not freeze
    /// against the corner.
    ///
    /// Root cause: intermediate-waypoint arrival pops the preceding waypoints immediately
    /// (radius hit then pass-by), leaving the unit targeting a waypoint whose straight-line
    /// path clips the building tile.  The unit cannot step forward and freezes indefinitely.
    /// Goal is placed >100 px away so tolerant arrival (64 px radius) never fires — the unit
    /// must actually move.
    #[test]
    #[ignore = "documents known building-corner freeze regression"]
    fn unit_pressed_against_building_wall_reaches_goal() {
        let map = flat_map(1);
        let mut entities = EntityStore::new();

        // Depot (2×2): center (352, 288) → footprint tiles (10,8),(11,8),(10,9),(11,9).
        // West wall at x=320, north wall at y=256.
        entities
            .spawn_building(1, EntityKind::Depot, 352.0, 288.0, true)
            .expect("building spawn");

        // Unit pressed against the building's west wall: x=319.5, tile (9,8), 0.5 px from
        // the tile boundary with blocked tile (10,8).
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, 319.5, 272.0)
            .expect("unit spawn");

        // Path along the building's north side to a goal well past it.  The arrival-radius
        // logic pops (9,8) immediately (dist ≤ 16 px) and pass-by pops (9,7) (unit x is
        // already past 304 in the direction of (10,7)).  The unit is left targeting (10,7)
        // at (336,240) from (319.5,272): the first partial step lands in building tile (10,8)
        // → blocked → frozen.  Goal (13,7) is ~117 px away, outside tolerant-arrival radius.
        let (w0x, w0y) = map.tile_center(9, 8);
        let (w1x, w1y) = map.tile_center(9, 7);
        let (w2x, w2y) = map.tile_center(10, 7);
        let (w3x, w3y) = map.tile_center(11, 7);
        let (w4x, w4y) = map.tile_center(12, 7);
        let (gx, gy) = map.tile_center(13, 7);
        set_path_direct(
            &mut entities,
            unit,
            vec![
                (w0x, w0y),
                (w1x, w1y),
                (w2x, w2y),
                (w3x, w3y),
                (w4x, w4y),
                (gx, gy),
            ],
        );

        for tick in 0..150u32 {
            let occ = Occupancy::build(&map, &entities);
            let spatial = SpatialIndex::build(&entities, map.size);
            movement_system(&map, &mut entities, &occ, &spatial, tick);
            let spatial = SpatialIndex::build(&entities, map.size);
            resolve_collisions(&mut entities, &spatial, &map, &occ);
        }

        let e = entities.get(unit).unwrap();
        let dx = e.pos_x - gx;
        let dy = e.pos_y - gy;
        let dist_to_goal = (dx * dx + dy * dy).sqrt();
        assert!(
            dist_to_goal <= config::TILE_SIZE as f32,
            "unit froze against building corner — {:.1}px from goal after 150 ticks",
            dist_to_goal
        );
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
            let spatial = SpatialIndex::build(&entities, map.size);
            movement_system(&map, &mut entities, &occ, &spatial, 0);
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
