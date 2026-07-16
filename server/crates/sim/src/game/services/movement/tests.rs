use super::*;
use crate::config;
use crate::game::entity::{
    BuildPhase, EntityKind, EntityStore, GatherPhase, MovePhase, Order, WeaponSetup,
};
use crate::game::map::Map;
use crate::game::pathfinding::Passability;
use crate::game::services::geometry::{
    building_rect_for_footprint, tile_rect, unit_body_for_entity, unit_body_intersects_rect,
    unit_body_overlap, unit_body_with_facing, CircleBody, OrientedBoxBody, OrientedCapsuleBody,
    UnitBody,
};
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::occupancy::footprint_center;
use crate::game::services::pathing::PathingService;
use crate::game::services::standability;
use crate::game::{PlayerState, ScoreState};
use crate::protocol::NoticeSeverity;
use rayon::prelude::*;

use super::collision::COLLISION_EPS_PX;
use super::pivot_drive::{
    pivot_drive_desired_path_point, ANTI_TANK_GUN_BODY_TURN_RATE_RAD_PER_TICK,
    PIVOT_VEHICLE_LOOKAHEAD_PX, VEHICLE_REVERSE_GOAL_DISTANCE_PX,
};
use super::scout_car::{
    scout_car_desired_path_point, SCOUT_CAR_MIN_TURN_RADIUS_PX, SCOUT_CAR_ROUTE_LOOKAHEAD_PX,
};
mod vehicle_traffic;

/// Distance (px) between two entity centers.
fn dist(entities: &EntityStore, a: u32, b: u32) -> f32 {
    let ea = entities.get(a).unwrap();
    let eb = entities.get(b).unwrap();
    let dx = ea.pos_x - eb.pos_x;
    let dy = ea.pos_y - eb.pos_y;
    (dx * dx + dy * dy).sqrt()
}

fn pos(entities: &EntityStore, id: u32) -> (f32, f32) {
    let e = entities.get(id).unwrap();
    (e.pos_x, e.pos_y)
}

fn moved_distance(from: (f32, f32), to: (f32, f32)) -> f32 {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    (dx * dx + dy * dy).sqrt()
}

fn body_overlap_depth(entities: &EntityStore, a: u32, b: u32) -> f32 {
    let a = entities.get(a).expect("a should exist");
    let b = entities.get(b).expect("b should exist");
    let Some(a_body) = unit_body_for_entity(a) else {
        return 0.0;
    };
    let Some(b_body) = unit_body_for_entity(b) else {
        return 0.0;
    };
    unit_body_overlap(a_body, b_body).map_or(0.0, |overlap| overlap.depth)
}

fn mark_moving(entities: &mut EntityStore, id: u32, goal: (f32, f32)) {
    if let Some(e) = entities.get_mut(id) {
        e.set_order(Order::move_to(goal.0, goal.1));
        e.set_path(vec![goal]);
        e.set_path_goal(Some(goal));
        e.mark_move_phase(MovePhase::Moving);
    }
}

/// A grass-only test map: the authored map contains obstacles, so for clean
/// movement/collision experiments we flatten the terrain after loading.
fn flat_map(player_count: usize) -> Map {
    let mut map = Map::generate(player_count, 0xC0FF_EE01);
    for v in &mut map.terrain {
        *v = crate::protocol::terrain::GRASS;
    }
    map
}

fn player_with_oil(id: u32, oil: u32) -> PlayerState {
    PlayerState {
        id,
        team_id: id,
        faction_id: "kriegsia".to_string(),
        name: format!("p{id}"),
        color: "#ffffff".to_string(),
        start_tile: (0, 0),
        steel: 0,
        oil,
        supply_used: 0,
        is_ai: false,
        score: ScoreState::default(),
        upgrades: Default::default(),
        ability_cooldowns: Default::default(),
    }
}
#[derive(Debug, Clone, Copy)]
struct TankMovementBaseline {
    travel_ticks: u32,
    path_length_px: f32,
    final_error_px: f32,
    facing_change_rad_per_sec: f32,
    stuck_ticks: u32,
    repath_count: u32,
    collision_displacement_px: f32,
    oil_burned: f32,
}

#[derive(Debug, Clone, Copy)]
struct ScoutCarMovementBaseline {
    travel_ticks: u32,
    path_length_px: f32,
    final_error_px: f32,
    min_static_clearance_px: f32,
    static_blocked_step_attempts: u32,
    reverse_recovery_activations: u32,
    repath_count: u32,
    max_consecutive_no_progress_ticks: u32,
    accepted_static_illegal_pose: bool,
    collision_displacement_px: f32,
}

impl ScoutCarMovementBaseline {
    fn assert_reference_envelope(&self, name: &str) {
        assert!(
            self.travel_ticks > 0,
            "{name}: fixture never advanced: {:?}",
            self
        );
        assert!(
            self.path_length_px > 16.0,
            "{name}: path_length_px should prove the fixture moved: {:?}",
            self
        );
        assert!(
            self.final_error_px <= config::TILE_SIZE as f32 * 1.5,
            "{name}: scout car ended too far from goal: {:?}",
            self
        );
        assert!(
            self.min_static_clearance_px.is_finite() && self.min_static_clearance_px >= 0.0,
            "{name}: accepted pose clipped static terrain/buildings: {:?}",
            self
        );
        assert!(
            !self.accepted_static_illegal_pose,
            "{name}: movement accepted a statically illegal scout-car pose: {:?}",
            self
        );
        assert!(
            self.max_consecutive_no_progress_ticks < 120,
            "{name}: scout car spent too long making no progress: {:?}",
            self
        );
    }
}

impl TankMovementBaseline {
    fn assert_reference_envelope(&self, name: &str) {
        assert!(
            self.travel_ticks > 0 && self.travel_ticks <= 1_200,
            "{name}: travel_ticks out of phase-0 envelope: {:?}",
            self
        );
        assert!(
            self.path_length_px > 16.0,
            "{name}: path_length_px should prove the fixture moved: {:?}",
            self
        );
        assert!(
            self.final_error_px <= config::TILE_SIZE as f32 * 1.5,
            "{name}: tank ended too far from goal: {:?}",
            self
        );
        assert!(
            self.facing_change_rad_per_sec.is_finite() && self.facing_change_rad_per_sec <= 2.0,
            "{name}: facing changed implausibly fast: {:?}",
            self
        );
        assert!(
            self.oil_burned > 0.0,
            "{name}: moving tank should burn oil: {:?}",
            self
        );
    }
}

fn measure_scout_car_fixture(
    name: &str,
    map: &Map,
    entities: &mut EntityStore,
    scout: u32,
    goal: (f32, f32),
    max_ticks: u32,
    order_via_coordinator: bool,
) -> ScoutCarMovementBaseline {
    let mut pathing = PathingService::new(8_192, 256);
    let start = pos(entities, scout);
    let mut last = start;
    let mut path_length_px = 0.0;
    let mut static_blocked_step_attempts = 0;
    let mut reverse_recovery_activations = 0;
    let mut repath_count = 0;
    let mut was_awaiting_path = false;
    let mut consecutive_no_progress = 0;
    let mut max_consecutive_no_progress_ticks = 0;
    let mut accepted_static_illegal_pose = false;
    let mut min_static_clearance_px = f32::INFINITY;
    let mut collision_displacement_px = 0.0;
    let mut travel_ticks = max_ticks;

    if !order_via_coordinator {
        set_path_direct(entities, scout, vec![goal]);
        if let Some(e) = entities.get_mut(scout) {
            e.set_order(Order::move_to(goal.0, goal.1));
            e.reset_stuck(start.0, start.1);
        }
    }

    for tick in 1..=max_ticks {
        pathing.advance_tick(tick);
        let occ = Occupancy::build(map, entities);
        let mut coordinator = MoveCoordinator::new(&mut pathing, map, &occ, tick);
        if tick == 1 && order_via_coordinator {
            coordinator.order_group_move(entities, 1, &[scout], goal, false);
        }
        coordinator.process_awaiting_paths(entities);

        let before = pos(entities, scout);
        let before_static_blocked = entities
            .get(scout)
            .and_then(|e| e.movement.as_ref())
            .map(|m| m.static_blocked_ticks)
            .unwrap_or(0);
        let before_reverse = entities
            .get(scout)
            .and_then(|e| e.movement.as_ref())
            .and_then(|m| m.scout_car_reverse_waypoint);

        let spatial = SpatialIndex::build(entities, map.size);
        movement_system(map, entities, &mut [], &occ, &spatial, tick);
        let after_movement = pos(entities, scout);
        let spatial = SpatialIndex::build(entities, map.size);
        resolve_collisions(entities, &spatial, map, &occ);
        let after_collision = pos(entities, scout);

        path_length_px += moved_distance(last, after_collision);
        collision_displacement_px += moved_distance(after_movement, after_collision);
        let moved_this_tick = moved_distance(before, after_collision);
        last = after_collision;

        let e = entities.get(scout).expect("scout car should still exist");
        let after_static_blocked = e
            .movement
            .as_ref()
            .map(|m| m.static_blocked_ticks)
            .unwrap_or(0);
        if after_static_blocked > before_static_blocked {
            static_blocked_step_attempts += 1;
        }
        let after_reverse = e
            .movement
            .as_ref()
            .and_then(|m| m.scout_car_reverse_waypoint);
        if before_reverse.is_none() && after_reverse.is_some() {
            reverse_recovery_activations += 1;
        }

        let awaiting_path = e.move_phase() == Some(MovePhase::AwaitingPath);
        if awaiting_path && !was_awaiting_path {
            repath_count += 1;
        }
        was_awaiting_path = awaiting_path;

        if moved_this_tick < config::STUCK_EPS_PX && !e.path_is_empty() {
            consecutive_no_progress += 1;
            max_consecutive_no_progress_ticks =
                max_consecutive_no_progress_ticks.max(consecutive_no_progress);
        } else {
            consecutive_no_progress = 0;
        }

        let clearance = scout_car_static_clearance_px(map, &occ, (e.pos_x, e.pos_y), e.facing());
        if clearance < 0.0 {
            accepted_static_illegal_pose = true;
        } else {
            min_static_clearance_px = min_static_clearance_px.min(clearance);
        }

        if e.path_is_empty() {
            travel_ticks = tick;
            break;
        }
    }

    let final_error_px = moved_distance(pos(entities, scout), goal);
    let baseline = ScoutCarMovementBaseline {
        travel_ticks,
        path_length_px,
        final_error_px,
        min_static_clearance_px,
        static_blocked_step_attempts,
        reverse_recovery_activations,
        repath_count,
        max_consecutive_no_progress_ticks,
        accepted_static_illegal_pose,
        collision_displacement_px,
    };
    println!("SCOUT_CAR_PHASE0_BASELINE {name}: {baseline:?}");
    baseline
}

fn measure_tank_fixture(
    name: &str,
    map: &Map,
    entities: &mut EntityStore,
    tank: u32,
    goal: (f32, f32),
    max_ticks: u32,
    order_via_coordinator: bool,
) -> TankMovementBaseline {
    let mut pathing = PathingService::new(8_192, 256);
    let mut players = vec![player_with_oil(1, 10_000)];
    let start = pos(entities, tank);
    let mut last = start;
    let mut last_facing = entities.get(tank).expect("tank should exist").facing();
    let mut path_length_px = 0.0;
    let mut facing_change = 0.0;
    let mut stuck_ticks = 0;
    let mut repath_count = 0;
    let mut was_awaiting_path = false;
    let mut collision_displacement_px = 0.0;
    let mut travel_ticks = max_ticks;

    if !order_via_coordinator {
        set_path_direct(entities, tank, vec![goal]);
        if let Some(e) = entities.get_mut(tank) {
            e.set_order(Order::move_to(goal.0, goal.1));
        }
    }

    for tick in 1..=max_ticks {
        pathing.advance_tick(tick);
        let occ = Occupancy::build(map, entities);
        let mut coordinator = MoveCoordinator::new(&mut pathing, map, &occ, tick);
        if tick == 1 && order_via_coordinator {
            coordinator.order_group_move(entities, 1, &[tank], goal, false);
        }
        coordinator.process_awaiting_paths(entities);

        let before = pos(entities, tank);
        let spatial = SpatialIndex::build(entities, map.size);
        movement_system(map, entities, &mut players, &occ, &spatial, tick);
        let after_movement = pos(entities, tank);
        let spatial = SpatialIndex::build(entities, map.size);
        resolve_collisions(entities, &spatial, map, &occ);
        let after_collision = pos(entities, tank);

        path_length_px += moved_distance(last, after_collision);
        collision_displacement_px += moved_distance(after_movement, after_collision);
        let moved_this_tick = moved_distance(before, after_collision);
        let e = entities.get(tank).expect("tank should still exist");
        facing_change += angle_delta(e.facing(), last_facing).abs();
        last_facing = e.facing();
        last = after_collision;

        let awaiting_path = e.move_phase() == Some(MovePhase::AwaitingPath);
        if awaiting_path && !was_awaiting_path {
            repath_count += 1;
        }
        was_awaiting_path = awaiting_path;

        if moved_this_tick <= 0.01 && !e.path_is_empty() {
            stuck_ticks += 1;
        }
        if e.path_is_empty() {
            travel_ticks = tick;
            break;
        }
    }

    let final_pos = pos(entities, tank);
    let final_error_px = moved_distance(final_pos, goal);
    let seconds = (travel_ticks.max(1) as f32) / config::TICK_HZ as f32;
    let oil_burned = entities
        .get(tank)
        .and_then(|e| e.lifetime_oil_used())
        .unwrap_or(0.0);
    let baseline = TankMovementBaseline {
        travel_ticks,
        path_length_px,
        final_error_px,
        facing_change_rad_per_sec: facing_change / seconds,
        stuck_ticks,
        repath_count,
        collision_displacement_px,
        oil_burned,
    };
    println!("TANK_PHASE0_BASELINE {name}: {baseline:?}");
    baseline
}

fn two_tile_wide_horizontal_corridor_map() -> Map {
    let size = 40;
    let mut terrain = vec![crate::protocol::terrain::ROCK; size * size];
    for y in 10..=11 {
        for x in 2..=36 {
            terrain[y * size + x] = crate::protocol::terrain::GRASS;
        }
    }
    Map {
        size: size as u32,
        terrain,
        starts: vec![],
        base_sites: vec![],
    }
}

fn tank_body_half_len() -> f32 {
    config::TANK_BODY_LENGTH_PX * 0.5 + config::TANK_BODY_CLEARANCE_PX
}

fn tank_body_half_width() -> f32 {
    config::TANK_BODY_WIDTH_PX * 0.5 + config::TANK_BODY_CLEARANCE_PX
}

fn anti_tank_gun_body_radius() -> f32 {
    config::ANTI_TANK_GUN_BODY_WIDTH_PX * 0.5 + config::ANTI_TANK_GUN_BODY_CLEARANCE_PX
}

fn scout_car_body_half_width() -> f32 {
    config::SCOUT_CAR_BODY_WIDTH_PX * 0.5 + config::SCOUT_CAR_BODY_CLEARANCE_PX
}

fn body_tile_range_for_test(body: UnitBody) -> impl Iterator<Item = (i32, i32)> {
    let ts = config::TILE_SIZE as f32;
    let eps = 0.001;
    let aabb = body.aabb();
    let min_tx = ((aabb.min_x - eps) / ts).floor() as i32;
    let min_ty = ((aabb.min_y - eps) / ts).floor() as i32;
    let max_tx = ((aabb.max_x + eps) / ts).ceil() as i32 - 1;
    let max_ty = ((aabb.max_y + eps) / ts).ceil() as i32 - 1;

    (min_ty..=max_ty).flat_map(move |ty| (min_tx..=max_tx).map(move |tx| (tx, ty)))
}

fn expanded_body(body: UnitBody, extra_px: f32) -> UnitBody {
    match body {
        UnitBody::Circle(body) => UnitBody::Circle(CircleBody {
            x: body.x,
            y: body.y,
            radius: body.radius + extra_px,
        }),
        UnitBody::OrientedCapsule(body) => UnitBody::OrientedCapsule(OrientedCapsuleBody {
            x: body.x,
            y: body.y,
            half_segment: body.half_segment,
            radius: body.radius + extra_px,
            facing: body.facing,
        }),
        UnitBody::OrientedBox(body) => UnitBody::OrientedBox(OrientedBoxBody {
            x: body.x,
            y: body.y,
            half_len: body.half_len + extra_px,
            half_width: body.half_width + extra_px,
            facing: body.facing,
        }),
    }
}

fn body_hits_static_blocker(map: &Map, occ: &Occupancy, body: UnitBody) -> bool {
    let aabb = body.aabb();
    let world_size = map.world_size_px();
    if aabb.min_x < 0.0 || aabb.min_y < 0.0 || aabb.max_x > world_size || aabb.max_y > world_size {
        return true;
    }

    for (tx, ty) in body_tile_range_for_test(body) {
        if !map.in_bounds(tx, ty) {
            return true;
        }
        if (!map.is_passable(tx, ty) || !occ.passable_for_kind(tx, ty, EntityKind::ScoutCar))
            && unit_body_intersects_rect(body, tile_rect(tx, ty))
        {
            return true;
        }
    }
    false
}

fn scout_car_static_clearance_px(map: &Map, occ: &Occupancy, pos: (f32, f32), facing: f32) -> f32 {
    let Some(body) = unit_body_with_facing(EntityKind::ScoutCar, pos.0, pos.1, facing) else {
        return -1.0;
    };
    if body_hits_static_blocker(map, occ, body) {
        return -1.0;
    }

    let mut clearance = 0.0;
    while clearance <= config::TILE_SIZE as f32 * 2.0 {
        let expanded = expanded_body(body, clearance + 1.0);
        if body_hits_static_blocker(map, occ, expanded) {
            return clearance;
        }
        clearance += 1.0;
    }
    clearance
}

fn tank_standable_at_entity_facing(
    map: &Map,
    occ: &Occupancy,
    entities: &EntityStore,
    id: u32,
) -> bool {
    let e = entities.get(id).expect("tank should exist");
    standability::unit_static_standable_with_facing(map, occ, e.kind, e.pos_x, e.pos_y, e.facing())
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

/// Slightly-overlapping soft units (centers closer than radius sum) are pushed apart in one
/// tick — both move by half the overlap.
#[test]
fn soft_units_still_split_push_evenly() {
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
    mark_moving(&mut entities, a, (cx - 64.0, cy));
    mark_moving(&mut entities, b, (cx + 64.0, cy));
    let a_before = pos(&entities, a);
    let b_before = pos(&entities, b);

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
    let a_after = pos(&entities, a);
    let b_after = pos(&entities, b);
    let a_moved = moved_distance(a_before, a_after);
    let b_moved = moved_distance(b_before, b_after);
    assert!(
        (a_moved - b_moved).abs() <= 0.01,
        "expected equal soft-unit push, got a {:.3}px and b {:.3}px",
        a_moved,
        b_moved
    );
    assert!(
        a_after.0 < a_before.0 && b_after.0 > b_before.0,
        "expected both units pushed outward (a {:.2}, b {:.2}, center {:.2})",
        a_after.0,
        b_after.0,
        cx
    );
}

#[test]
fn tank_pushes_soft_infantry_more_than_it_moves() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (cx, cy) = map.tile_center(20, 20);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, cx - 10.0, cy)
        .unwrap();
    let rifleman = entities
        .spawn_unit(2, EntityKind::Rifleman, cx + 10.0, cy)
        .unwrap();
    mark_moving(&mut entities, rifleman, (cx + 64.0, cy));
    let tank_before = pos(&entities, tank);
    let rifleman_before = pos(&entities, rifleman);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    resolve_collisions(&mut entities, &spatial, &map, &occ);

    let tank_moved = moved_distance(tank_before, pos(&entities, tank));
    let rifleman_moved = moved_distance(rifleman_before, pos(&entities, rifleman));
    assert!(
        rifleman_moved > tank_moved * 6.0,
        "expected tank to displace rifleman much more than itself (tank {:.3}px, rifleman {:.3}px)",
        tank_moved,
        rifleman_moved
    );
    assert!(
        body_overlap_depth(&entities, tank, rifleman) <= COLLISION_EPS_PX,
        "tank and rifleman should resolve body overlap"
    );
}

#[test]
fn tank_infantry_overlap_resolves_from_oriented_hull() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (cx, cy) = map.tile_center(20, 20);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, cx, cy)
        .expect("tank spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(0.0);
    }
    let rifle_radius = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman stats")
        .radius;
    let rifleman = entities
        .spawn_unit(
            2,
            EntityKind::Rifleman,
            cx + tank_body_half_len() + rifle_radius - 4.0,
            cy,
        )
        .expect("rifleman spawn");
    mark_moving(&mut entities, rifleman, (cx + 128.0, cy));
    let tank_before = pos(&entities, tank);
    let rifleman_before = pos(&entities, rifleman);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    resolve_collisions(&mut entities, &spatial, &map, &occ);

    let tank_after = pos(&entities, tank);
    let rifleman_after = pos(&entities, rifleman);
    assert!(
        body_overlap_depth(&entities, tank, rifleman) <= COLLISION_EPS_PX,
        "oriented tank hull and infantry circle should separate"
    );
    assert!(
        (tank_after.1 - tank_before.1).abs() <= 0.001,
        "front collision should not sidestep the tank sideways"
    );
    assert!(
        rifleman_after.0 > rifleman_before.0,
        "soft infantry should absorb the tank-front overlap"
    );
}

#[test]
fn anti_tank_gun_infantry_overlap_resolves_from_circular_body() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (cx, cy) = map.tile_center(20, 20);
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, cx, cy)
        .expect("anti-tank gun spawn");
    if let Some(e) = entities.get_mut(anti_tank_gun) {
        e.set_facing(0.0);
    }
    let rifle_radius = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman stats")
        .radius;
    let rifleman = entities
        .spawn_unit(
            2,
            EntityKind::Rifleman,
            cx + anti_tank_gun_body_radius() + rifle_radius - 4.0,
            cy,
        )
        .expect("rifleman spawn");
    assert_eq!(
        unit_body_with_facing(EntityKind::AntiTankGun, cx, cy, 0.0),
        unit_body_with_facing(EntityKind::AntiTankGun, cx, cy, std::f32::consts::FRAC_PI_2),
        "anti-tank gun collision body should not change with facing"
    );
    mark_moving(&mut entities, rifleman, (cx + 128.0, cy));
    let anti_tank_gun_before = pos(&entities, anti_tank_gun);
    let rifleman_before = pos(&entities, rifleman);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    resolve_collisions(&mut entities, &spatial, &map, &occ);

    let anti_tank_gun_after = pos(&entities, anti_tank_gun);
    let rifleman_after = pos(&entities, rifleman);
    assert!(
        body_overlap_depth(&entities, anti_tank_gun, rifleman) <= COLLISION_EPS_PX,
        "circular anti-tank gun body and infantry circle should separate"
    );
    assert!(
        (anti_tank_gun_after.1 - anti_tank_gun_before.1).abs() <= 0.001,
        "head-on collision should not sidestep the anti-tank gun sideways"
    );
    assert!(
        rifleman_after.0 > rifleman_before.0,
        "soft infantry should absorb the anti-tank gun overlap"
    );
}

#[test]
fn tank_tank_head_on_conflict_resolves_without_side_slide() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (cx, cy) = map.tile_center(20, 20);
    let overlap_px = 8.0;
    let left = entities
        .spawn_unit(
            1,
            EntityKind::Tank,
            cx - tank_body_half_len() + overlap_px * 0.5,
            cy,
        )
        .expect("left tank spawn");
    let right = entities
        .spawn_unit(
            2,
            EntityKind::Tank,
            cx + tank_body_half_len() - overlap_px * 0.5,
            cy,
        )
        .expect("right tank spawn");
    if let Some(e) = entities.get_mut(left) {
        e.set_facing(0.0);
    }
    if let Some(e) = entities.get_mut(right) {
        e.set_facing(std::f32::consts::PI);
    }

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    resolve_collisions(&mut entities, &spatial, &map, &occ);

    let left_after = pos(&entities, left);
    let right_after = pos(&entities, right);
    assert!(
        body_overlap_depth(&entities, left, right) <= COLLISION_EPS_PX,
        "head-on tanks should separate along their hulls"
    );
    assert!(
        (left_after.1 - cy).abs() <= 0.001 && (right_after.1 - cy).abs() <= 0.001,
        "head-on tank conflict should stop/reverse along the lane, not slide sideways"
    );
    assert!(
        left_after.0 < cx - tank_body_half_len() + overlap_px * 0.5
            && right_after.0 > cx + tank_body_half_len() - overlap_px * 0.5,
        "both tanks should back out of the head-on hull overlap"
    );
}

#[test]
fn braced_machine_gunner_holds_ground_against_soft_unit() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (cx, cy) = map.tile_center(20, 20);
    let mg = entities
        .spawn_unit(1, EntityKind::MachineGunner, cx - 5.0, cy)
        .unwrap();
    let rifleman = entities
        .spawn_unit(2, EntityKind::Rifleman, cx + 5.0, cy)
        .unwrap();
    if let Some(e) = entities.get_mut(mg) {
        e.set_weapon_setup(WeaponSetup::Deployed);
    }
    mark_moving(&mut entities, rifleman, (cx + 64.0, cy));
    let mg_before = pos(&entities, mg);
    let rifleman_before = pos(&entities, rifleman);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    resolve_collisions(&mut entities, &spatial, &map, &occ);

    let mg_moved = moved_distance(mg_before, pos(&entities, mg));
    let rifleman_moved = moved_distance(rifleman_before, pos(&entities, rifleman));
    assert!(
        rifleman_moved > mg_moved * 5.0,
        "expected braced MG to hold ground against soft rifleman (mg {:.3}px, rifleman {:.3}px)",
        mg_moved,
        rifleman_moved
    );
}

#[test]
fn firing_rifleman_is_firmer_than_moving_rifleman() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (cx, cy) = map.tile_center(20, 20);
    let firing = entities
        .spawn_unit(1, EntityKind::Rifleman, cx - 5.0, cy)
        .unwrap();
    let moving = entities
        .spawn_unit(2, EntityKind::Rifleman, cx + 5.0, cy)
        .unwrap();
    if let Some(e) = entities.get_mut(firing) {
        e.set_target_id(Some(moving));
    }
    mark_moving(&mut entities, moving, (cx + 64.0, cy));
    let firing_before = pos(&entities, firing);
    let moving_before = pos(&entities, moving);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    resolve_collisions(&mut entities, &spatial, &map, &occ);

    let firing_moved = moved_distance(firing_before, pos(&entities, firing));
    let moving_moved = moved_distance(moving_before, pos(&entities, moving));
    assert!(
            moving_moved > firing_moved * 2.0,
            "expected firing rifleman to be firmer than moving rifleman (firing {:.3}px, moving {:.3}px)",
            firing_moved,
            moving_moved
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

#[test]
fn worker_traveling_to_gather_ignores_unit_collision() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (cx, cy) = map.tile_center(20, 20);
    let node = entities
        .spawn_node(EntityKind::Steel, cx + 96.0, cy)
        .unwrap();
    let worker = entities.spawn_unit(1, EntityKind::Worker, cx, cy).unwrap();
    {
        let w = entities.get_mut(worker).unwrap();
        w.set_order(Order::gather(node));
        w.mark_gather_phase(GatherPhase::ToNode);
    }
    let blocker = entities
        .spawn_unit(2, EntityKind::Tank, cx + 4.0, cy)
        .unwrap();
    let worker_before = pos(&entities, worker);
    let blocker_before = pos(&entities, blocker);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    resolve_collisions(&mut entities, &spatial, &map, &occ);

    assert_eq!(
        worker_before,
        pos(&entities, worker),
        "worker on a gather order should not be pushed while traveling to the node"
    );
    assert_eq!(
        blocker_before,
        pos(&entities, blocker),
        "worker on a gather order should not push blocking units while traveling to the node"
    );
}

#[test]
fn worker_traveling_to_build_ignores_unit_collision() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (cx, cy) = map.tile_center(20, 20);
    let worker = entities.spawn_unit(1, EntityKind::Worker, cx, cy).unwrap();
    {
        let w = entities.get_mut(worker).unwrap();
        w.set_order(Order::build(EntityKind::Depot, 24, 20));
        w.mark_build_phase(BuildPhase::ToSite);
    }
    let blocker = entities
        .spawn_unit(2, EntityKind::Tank, cx + 4.0, cy)
        .unwrap();
    let worker_before = pos(&entities, worker);
    let blocker_before = pos(&entities, blocker);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    resolve_collisions(&mut entities, &spatial, &map, &occ);

    assert_eq!(
        worker_before,
        pos(&entities, worker),
        "worker on a build order should not be pushed while traveling to the site"
    );
    assert_eq!(
        blocker_before,
        pos(&entities, blocker),
        "worker on a build order should not push blocking units while traveling to the site"
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
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);
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
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
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

#[test]
fn tank_routes_around_diagonal_pinch_between_offset_factories() {
    // Two factories arranged so their corners are diagonally opposite across a
    // single-tile gap column. Before the diagonal-pinch fix, the tank would
    // permanently stall trying to thread (3, 3)-style tiles in this gap.
    let map = flat_map(1);
    let mut entities = EntityStore::new();

    // Factory A at top-left tile (10, 10) -> covers (10..=12, 10..=12).
    // Factory B at top-left tile (14, 13) -> covers (14..=16, 13..=15).
    // Gap column x = 13; diagonal pinch at tile (13, 13).
    let (ax, ay) = footprint_center(&map, EntityKind::Factory, 10, 10);
    entities
        .spawn_building(2, EntityKind::Factory, ax, ay, true)
        .expect("factory A spawn");
    let (bx, by) = footprint_center(&map, EntityKind::Factory, 14, 13);
    entities
        .spawn_building(2, EntityKind::Factory, bx, by, true)
        .expect("factory B spawn");

    let start = map.tile_center(8, 8);
    let goal = map.tile_center(18, 17);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank spawn");

    let mut pathing = PathingService::new(8_192, 256);
    let mut stuck_ticks = 0u32;
    let mut last_pos = start;

    for tick in 1u32..=900 {
        pathing.advance_tick(tick);
        let occ = Occupancy::build(&map, &entities);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, tick);
        if tick == 1 {
            coordinator.order_group_move(&mut entities, 1, &[tank], goal, false);
        }
        coordinator.process_awaiting_paths(&mut entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
        let spatial = SpatialIndex::build(&entities, map.size);
        resolve_collisions(&mut entities, &spatial, &map, &occ);

        let e = entities.get(tank).expect("tank should still exist");
        let now = (e.pos_x, e.pos_y);
        let moved = moved_distance(last_pos, now);
        if moved <= 0.01 && !e.path_is_empty() {
            stuck_ticks += 1;
        } else if moved > 0.01 {
            stuck_ticks = 0;
        }
        assert!(
            stuck_ticks < 200,
            "tank deadlocked at ({:.1}, {:.1}) at tick {tick} — diagonal-pinch regression",
            now.0,
            now.1
        );
        last_pos = now;

        if e.path_is_empty() {
            break;
        }
    }

    let e = entities.get(tank).expect("tank should still exist");
    let dx = e.pos_x - goal.0;
    let dy = e.pos_y - goal.1;
    let dist_to_goal = (dx * dx + dy * dy).sqrt();
    assert!(
        dist_to_goal <= config::TOLERANT_ARRIVAL_RADIUS_PX,
        "tank failed to route around the diagonal pinch; ended {:.1}px from goal at ({:.1}, {:.1})",
        dist_to_goal,
        e.pos_x,
        e.pos_y
    );
}

#[test]
fn tank_moves_through_long_two_tile_wide_corridor() {
    let map = two_tile_wide_horizontal_corridor_map();
    let mut entities = EntityStore::new();
    let start = map.tile_center(3, 10);
    let goal = map.tile_center(35, 10);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank should spawn in corridor");
    let mut pathing = PathingService::new(8_192, 256);

    for tick in 1u32..=900 {
        pathing.advance_tick(tick);
        let occ = Occupancy::build(&map, &entities);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, tick);
        if tick == 1 {
            coordinator.order_group_move(&mut entities, 1, &[tank], goal, false);
        }
        coordinator.process_awaiting_paths(&mut entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
        let spatial = SpatialIndex::build(&entities, map.size);
        resolve_collisions(&mut entities, &spatial, &map, &occ);

        let e = entities.get(tank).expect("tank should still exist");
        assert!(
            standability::unit_static_standable_with_facing(
                &map,
                &occ,
                EntityKind::Tank,
                e.pos_x,
                e.pos_y,
                e.facing()
            ),
            "tank body became illegal at tick {tick}: ({:.1}, {:.1})",
            e.pos_x,
            e.pos_y
        );
    }

    let e = entities.get(tank).expect("tank should still exist");
    let dx = e.pos_x - goal.0;
    let dy = e.pos_y - goal.1;
    let dist_to_goal = (dx * dx + dy * dy).sqrt();
    assert!(
        dist_to_goal <= config::TILE_SIZE as f32,
        "tank did not traverse the long 2-tile corridor; ended {:.1}px from goal at ({:.1}, {:.1})",
        dist_to_goal,
        e.pos_x,
        e.pos_y
    );
}

#[test]
fn tank_phase0_baseline_open_ground() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let start = map.tile_center(12, 12);
    let goal = map.tile_center(28, 12);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank should spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(0.0);
    }

    let baseline = measure_tank_fixture("open_ground", &map, &mut entities, tank, goal, 600, false);

    baseline.assert_reference_envelope("open_ground");
    assert_eq!(
        baseline.stuck_ticks, 0,
        "open-ground tank should not spend ticks stuck: {:?}",
        baseline
    );
    assert_eq!(
        baseline.repath_count, 0,
        "direct open-ground fixture should not request a repath: {:?}",
        baseline
    );
    assert!(
        baseline.collision_displacement_px <= 0.01,
        "single tank on open ground should not be collision-displaced: {:?}",
        baseline
    );
}

#[test]
fn tank_phase0_baseline_building_corner() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Depot, 18, 18);
    entities
        .spawn_building(2, EntityKind::Depot, bx, by, true)
        .expect("depot spawn");
    let rect = building_rect_for_footprint(EntityKind::Depot, 18, 18).expect("depot rect");
    let tank_radius = config::unit_stats(EntityKind::Tank)
        .expect("tank stats")
        .radius;
    let start = (
        rect.min_x - tank_radius - config::TILE_SIZE as f32,
        rect.max_y + tank_radius + config::TILE_SIZE as f32,
    );
    let goal = (
        rect.max_x + tank_radius + config::TILE_SIZE as f32 * 2.0,
        rect.min_y - tank_radius - config::TILE_SIZE as f32,
    );
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank should spawn");

    let baseline = measure_tank_fixture(
        "building_corner",
        &map,
        &mut entities,
        tank,
        goal,
        900,
        true,
    );

    baseline.assert_reference_envelope("building_corner");
    assert!(
        baseline.path_length_px > moved_distance(start, goal),
        "corner route should be longer than the blocked straight line: {:?}",
        baseline
    );
}

#[test]
fn tank_phase0_baseline_two_tile_corridor() {
    let map = two_tile_wide_horizontal_corridor_map();
    let mut entities = EntityStore::new();
    let start = map.tile_center(3, 10);
    let goal = map.tile_center(35, 10);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank should spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(0.0);
    }

    let baseline = measure_tank_fixture(
        "two_tile_corridor",
        &map,
        &mut entities,
        tank,
        goal,
        900,
        true,
    );

    baseline.assert_reference_envelope("two_tile_corridor");
    assert!(
        baseline.collision_displacement_px <= 0.01,
        "corridor fixture has no traffic, so collision displacement should stay zero: {:?}",
        baseline
    );
}

#[test]
fn tank_phase0_baseline_traffic_cluster() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let start = map.tile_center(12, 24);
    let goal = map.tile_center(28, 24);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank should spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(0.0);
    }
    for (dx, dy) in [(96.0, -18.0), (118.0, 16.0), (144.0, 0.0), (170.0, -14.0)] {
        entities
            .spawn_unit(2, EntityKind::Rifleman, start.0 + dx, start.1 + dy)
            .expect("traffic spawn");
    }

    let baseline = measure_tank_fixture(
        "traffic_cluster",
        &map,
        &mut entities,
        tank,
        goal,
        900,
        true,
    );

    baseline.assert_reference_envelope("traffic_cluster");
    assert!(
        baseline.collision_displacement_px <= 0.01,
        "phase-4 tank traffic should avoid collision shoving the tank sideways: {:?}",
        baseline
    );
}

fn spawn_ordered_scout_car(
    entities: &mut EntityStore,
    start: (f32, f32),
    goal: (f32, f32),
    facing: f32,
) -> u32 {
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, start.0, start.1)
        .expect("scout car should spawn");
    if let Some(e) = entities.get_mut(scout) {
        e.set_facing(facing);
        e.set_order(Order::move_to(goal.0, goal.1));
        e.reset_stuck(start.0, start.1);
    }
    scout
}

fn block_rect_tiles(map: &mut Map, min_x: u32, min_y: u32, max_x: u32, max_y: u32) {
    for ty in min_y..=max_y {
        for tx in min_x..=max_x {
            let idx = map.index(tx, ty);
            map.terrain[idx] = crate::protocol::terrain::ROCK;
        }
    }
}

fn carve_rect_tiles(map: &mut Map, min_x: u32, min_y: u32, max_x: u32, max_y: u32) {
    for ty in min_y..=max_y {
        for tx in min_x..=max_x {
            let idx = map.index(tx, ty);
            map.terrain[idx] = crate::protocol::terrain::GRASS;
        }
    }
}

fn carve_horizontal_corridor(map: &mut Map, min_x: u32, max_x: u32, center_y: u32) {
    carve_rect_tiles(map, min_x, center_y - 1, max_x, center_y + 1);
}

fn carve_vertical_corridor(map: &mut Map, center_x: u32, min_y: u32, max_y: u32) {
    carve_rect_tiles(map, center_x - 1, min_y, center_x + 1, max_y);
}

#[derive(Debug, Clone)]
struct SnakingCorridorTiming {
    unit: EntityKind,
    unit_count: usize,
    clear_ticks: Option<u32>,
    clear_seconds: Option<f32>,
    final_state: Vec<String>,
}

#[derive(Debug, Clone)]
struct SnakingCorridorUnitTiming {
    unit: EntityKind,
    unit_count: usize,
    clear_ticks: Vec<Option<u32>>,
    final_state: Vec<String>,
}

#[derive(Debug, Clone)]
struct WallChokepointTiming {
    unit: EntityKind,
    unit_count: usize,
    clear_ticks: Vec<Option<u32>>,
}

const SNAKING_CORRIDOR_REGRESSION_BASELINES: &[(EntityKind, usize, u32)] = &[
    (EntityKind::Worker, 1, 2_295),
    (EntityKind::Worker, 4, 2_322),
    (EntityKind::Rifleman, 1, 2_295),
    (EntityKind::Rifleman, 4, 2_322),
    (EntityKind::MachineGunner, 1, 2_883),
    (EntityKind::MachineGunner, 4, 2_927),
    (EntityKind::AntiTankGun, 1, 3_169),
    (EntityKind::AntiTankGun, 4, 3_318),
    (EntityKind::ScoutCar, 1, 1_577),
    (EntityKind::ScoutCar, 4, 1_647),
    (EntityKind::Tank, 1, 1_953),
    (EntityKind::Tank, 4, 2_102),
];

const SNAKING_CORRIDOR_VEHICLE_SLOT_BASELINES: &[(EntityKind, usize, u32)] = &[
    (EntityKind::Tank, 0, 1_939),
    (EntityKind::Tank, 1, 1_993),
    (EntityKind::Tank, 2, 2_049),
    (EntityKind::Tank, 3, 2_102),
    (EntityKind::AntiTankGun, 0, 3_139),
    (EntityKind::AntiTankGun, 1, 3_201),
    (EntityKind::AntiTankGun, 2, 3_259),
    (EntityKind::AntiTankGun, 3, 3_318),
];

const WALL_CHOKEPOINT_MAX_TICKS: u32 = 3_000;
const WALL_CHOKEPOINT_SCORE_BASELINES: &[(EntityKind, usize, u32)] = &[
    (EntityKind::AntiTankGun, 3, 332),
    (EntityKind::AntiTankGun, 5, 392),
    (EntityKind::AntiTankGun, 6, 368),
    (EntityKind::AntiTankGun, 10, 432),
    (EntityKind::AntiTankGun, 15, 528),
    (EntityKind::ScoutCar, 3, 156),
    (EntityKind::ScoutCar, 5, 176),
    (EntityKind::ScoutCar, 6, 175),
    (EntityKind::ScoutCar, 10, 203),
    (EntityKind::ScoutCar, 15, 234),
    (EntityKind::Tank, 3, 240),
    (EntityKind::Tank, 5, 330),
    (EntityKind::Tank, 6, 429),
];

fn scout_car_snaking_corridor_map() -> (Map, u32, f32, (f32, f32), (f32, f32)) {
    let mut map = flat_map(1);
    let stone_min_y = 15u32;
    let stone_max_y = 75u32;
    let exit_x = 36u32;
    let first_left_x = 26u32;
    let right_x = 56u32;
    let lower_lane_y = 68u32;
    let middle_lane_y = 64u32;
    let upper_lane_y = 60u32;

    let stone_max_x = map.size - 1;
    block_rect_tiles(&mut map, 0, stone_min_y, stone_max_x, stone_max_y);

    carve_vertical_corridor(&mut map, exit_x, lower_lane_y, stone_max_y);
    carve_horizontal_corridor(&mut map, first_left_x, exit_x, lower_lane_y);
    carve_vertical_corridor(&mut map, first_left_x, middle_lane_y, lower_lane_y);
    carve_horizontal_corridor(&mut map, first_left_x, right_x, middle_lane_y);
    carve_vertical_corridor(&mut map, right_x, upper_lane_y, middle_lane_y);
    carve_horizontal_corridor(&mut map, exit_x, right_x, upper_lane_y);
    carve_vertical_corridor(&mut map, exit_x, stone_min_y, upper_lane_y);

    let ts = config::TILE_SIZE as f32;
    let start = map.tile_center(exit_x, stone_max_y + 5);
    let exit = map.tile_center(exit_x, stone_min_y - 1);
    let goal = (exit.0 + ts * 10.0, exit.1 - ts * 10.0);
    let exit_clear_y = stone_min_y as f32 * ts;

    (map, exit_x, exit_clear_y, start, goal)
}

fn spawn_snaking_corridor_units(
    entities: &mut EntityStore,
    unit: EntityKind,
    unit_count: usize,
    start: (f32, f32),
) -> Vec<u32> {
    let north = -std::f32::consts::FRAC_PI_2;
    let (x_spacing, y_spacing) = snaking_corridor_spawn_spacing(unit);
    let positions: Vec<(f32, f32)> = match unit_count {
        1 => vec![start],
        4 => {
            vec![
                (start.0 - x_spacing * 0.5, start.1 - y_spacing * 0.5),
                (start.0 + x_spacing * 0.5, start.1 - y_spacing * 0.5),
                (start.0 - x_spacing * 0.5, start.1 + y_spacing * 0.5),
                (start.0 + x_spacing * 0.5, start.1 + y_spacing * 0.5),
            ]
        }
        _ => panic!("unsupported snaking corridor unit count {unit_count}"),
    };

    positions
        .into_iter()
        .map(|(x, y)| {
            let spawned = entities
                .spawn_unit(1, unit, x, y)
                .expect("unit should spawn");
            if let Some(e) = entities.get_mut(spawned) {
                e.set_facing(north);
            }
            spawned
        })
        .collect()
}

fn scout_car_wall_chokepoint_map(
    unit: EntityKind,
    unit_count: usize,
) -> (Map, f32, Vec<(f32, f32)>, (f32, f32)) {
    let mut map = flat_map(1);
    let center_x = map.size / 2;
    let wall_y = map.size - 18;
    let gap_left_x = center_x - 1;
    let gap_right_x = center_x;
    let max_tile = map.size - 1;

    block_rect_tiles(&mut map, 0, wall_y, max_tile, wall_y);
    carve_rect_tiles(&mut map, gap_left_x, wall_y, gap_right_x, wall_y);

    let ts = config::TILE_SIZE as f32;
    let center_world_x = gap_right_x as f32 * ts;
    let start_y = (wall_y as f32 + 10.5) * ts;
    let spacing = match unit {
        EntityKind::AntiTankGun => {
            config::ANTI_TANK_GUN_BODY_WIDTH_PX + config::ANTI_TANK_GUN_BODY_CLEARANCE_PX * 4.0
        }
        EntityKind::ScoutCar => {
            config::SCOUT_CAR_BODY_WIDTH_PX + config::SCOUT_CAR_BODY_CLEARANCE_PX * 4.0
        }
        EntityKind::Tank => config::TANK_BODY_WIDTH_PX + config::TANK_BODY_CLEARANCE_PX * 4.0,
        _ => panic!("wall chokepoint only supports vehicles"),
    };
    let center_index = (unit_count.saturating_sub(1)) as f32 * 0.5;
    let starts = (0..unit_count)
        .map(|i| {
            let offset = (i as f32 - center_index) * spacing;
            (center_world_x + offset, start_y)
        })
        .collect();
    let goal_y = (wall_y as f32 + 0.5) * ts - ts * 10.0;
    let goal = (center_world_x, goal_y);
    let exit_clear_y = wall_y as f32 * ts;

    (map, exit_clear_y, starts, goal)
}

fn spawn_wall_chokepoint_units(
    entities: &mut EntityStore,
    unit: EntityKind,
    starts: Vec<(f32, f32)>,
) -> Vec<u32> {
    let north = -std::f32::consts::FRAC_PI_2;
    starts
        .into_iter()
        .map(|(x, y)| {
            let spawned = entities
                .spawn_unit(1, unit, x, y)
                .expect("unit should spawn");
            if let Some(e) = entities.get_mut(spawned) {
                e.set_facing(north);
            }
            spawned
        })
        .collect()
}

fn wall_chokepoint_units_clear_exit(
    entities: &EntityStore,
    unit: EntityKind,
    units: &[u32],
    exit_clear_y: f32,
) -> bool {
    let radius = config::unit_stats(unit).expect("unit stats").radius;
    units.iter().all(|&id| {
        entities
            .get(id)
            .is_some_and(|e| e.pos_y + radius < exit_clear_y)
    })
}

fn measure_wall_chokepoint_clear_times(
    unit: EntityKind,
    unit_count: usize,
) -> WallChokepointTiming {
    let (map, exit_clear_y, starts, goal) = scout_car_wall_chokepoint_map(unit, unit_count);
    let mut entities = EntityStore::new();
    let units = spawn_wall_chokepoint_units(&mut entities, unit, starts);
    let mut clear_ticks = vec![None; units.len()];
    let mut pathing = PathingService::new(65_536, 512);

    for tick in 1..=WALL_CHOKEPOINT_MAX_TICKS {
        pathing.advance_tick(tick);
        let occ = Occupancy::build(&map, &entities);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, tick);
        if tick == 1 {
            coordinator.order_group_move(&mut entities, 1, &units, goal, false);
        }
        coordinator.process_awaiting_paths(&mut entities);

        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
        let spatial = SpatialIndex::build(&entities, map.size);
        resolve_collisions(&mut entities, &spatial, &map, &occ);

        for (slot, id) in units.iter().copied().enumerate() {
            if clear_ticks[slot].is_none()
                && unit_clears_wall_chokepoint_exit(&entities, unit, id, exit_clear_y)
            {
                clear_ticks[slot] = Some(tick);
            }
        }

        if clear_ticks.iter().all(Option::is_some) {
            break;
        }
    }

    WallChokepointTiming {
        unit,
        unit_count,
        clear_ticks,
    }
}

fn unit_clears_wall_chokepoint_exit(
    entities: &EntityStore,
    unit: EntityKind,
    id: u32,
    exit_clear_y: f32,
) -> bool {
    let radius = config::unit_stats(unit).expect("unit stats").radius;
    entities
        .get(id)
        .is_some_and(|e| e.pos_y + radius < exit_clear_y)
}

fn describe_units(entities: &EntityStore, units: &[u32]) -> Vec<String> {
    units
        .iter()
        .filter_map(|&id| {
            let e = entities.get(id)?;
            let movement = e.movement.as_ref();
            Some(format!(
                "#{id}: pos=({:.1},{:.1}) facing={:.3} phase={:?} path_len={} next={:?} goal={:?} stuck={} blocked={} recovery_cd={}",
                e.pos_x,
                e.pos_y,
                e.facing(),
                e.move_phase(),
                movement.map(|m| m.path.len()).unwrap_or(0),
                e.next_waypoint(),
                e.path_goal(),
                movement.map(|m| m.stuck_ticks).unwrap_or(0),
                movement.map(|m| m.static_blocked_ticks).unwrap_or(0),
                movement
                    .map(|m| m.scout_car_recovery_cooldown)
                    .unwrap_or(0),
            ))
        })
        .collect()
}

fn wall_chokepoint_timing_summary(results: &[WallChokepointTiming]) -> Vec<String> {
    results
        .iter()
        .map(|result| {
            let clear_ticks: Vec<String> = result
                .clear_ticks
                .iter()
                .map(|ticks| {
                    ticks
                        .map(|ticks| ticks.to_string())
                        .unwrap_or_else(|| "timeout".to_string())
                })
                .collect();
            format!(
                "{} count={}: [{}]",
                result.unit,
                result.unit_count,
                clear_ticks.join(", ")
            )
        })
        .collect()
}

fn wall_chokepoint_case_score(result: &WallChokepointTiming) -> u32 {
    let total_score = result.clear_ticks.iter().fold(0u64, |total, clear_ticks| {
        total.saturating_add(clear_ticks.unwrap_or(WALL_CHOKEPOINT_MAX_TICKS) as u64)
    });
    (total_score / result.clear_ticks.len() as u64) as u32
}

fn wall_chokepoint_score_baseline(unit: EntityKind, unit_count: usize) -> u32 {
    WALL_CHOKEPOINT_SCORE_BASELINES
        .iter()
        .find_map(|(baseline_unit, baseline_count, score)| {
            (*baseline_unit == unit && *baseline_count == unit_count).then_some(*score)
        })
        .expect("baseline should exist for each wall chokepoint scenario")
}

fn snaking_corridor_spawn_spacing(unit: EntityKind) -> (f32, f32) {
    match unit {
        EntityKind::AntiTankGun => (
            config::ANTI_TANK_GUN_BODY_WIDTH_PX * 1.5,
            config::ANTI_TANK_GUN_BODY_LENGTH_PX * 1.5,
        ),
        EntityKind::ScoutCar => (
            config::SCOUT_CAR_BODY_WIDTH_PX * 1.5,
            config::SCOUT_CAR_BODY_LENGTH_PX * 1.5,
        ),
        EntityKind::Tank => (
            config::TANK_BODY_WIDTH_PX * 1.5,
            config::TANK_BODY_LENGTH_PX * 1.5,
        ),
        _ => {
            let radius = config::unit_stats(unit).expect("unit stats").radius;
            let spacing = radius * 3.0;
            (spacing, spacing)
        }
    }
}

fn units_clear_tunnel_exit(
    entities: &EntityStore,
    unit: EntityKind,
    units: &[u32],
    exit_clear_y: f32,
) -> bool {
    let radius = config::unit_stats(unit).expect("unit stats").radius;
    units.iter().all(|&id| {
        entities
            .get(id)
            .is_some_and(|e| e.pos_y + radius < exit_clear_y)
    })
}

fn unit_clears_tunnel_exit(
    entities: &EntityStore,
    unit: EntityKind,
    id: u32,
    exit_clear_y: f32,
) -> bool {
    let radius = config::unit_stats(unit).expect("unit stats").radius;
    entities
        .get(id)
        .is_some_and(|e| e.pos_y + radius < exit_clear_y)
}

fn measure_snaking_corridor_unit_clear_times(
    unit: EntityKind,
    unit_count: usize,
    required_slots: &[usize],
) -> SnakingCorridorUnitTiming {
    let (map, exit_x, exit_clear_y, start, goal) = scout_car_snaking_corridor_map();
    let mut entities = EntityStore::new();
    let units = spawn_snaking_corridor_units(&mut entities, unit, unit_count, start);
    let occ = Occupancy::build(&map, &entities);
    for &spawned in &units {
        let e = entities.get(spawned).expect("unit should exist");
        assert!(standability::unit_static_standable_with_facing(
            &map,
            &occ,
            unit,
            e.pos_x,
            e.pos_y,
            e.facing()
        ));
    }
    assert_eq!(
        map.tile_of(start.0, start.1).0,
        exit_x,
        "units should start vertically aligned with the tunnel exit"
    );

    let mut clear_ticks = vec![None; units.len()];
    let mut pathing = PathingService::new(65_536, 512);
    let max_ticks = 12_000u32;
    for tick in 1..=max_ticks {
        pathing.advance_tick(tick);
        let occ = Occupancy::build(&map, &entities);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, tick);
        if tick == 1 {
            coordinator.order_group_move(&mut entities, 1, &units, goal, false);
        }
        coordinator.process_awaiting_paths(&mut entities);

        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
        let spatial = SpatialIndex::build(&entities, map.size);
        resolve_collisions(&mut entities, &spatial, &map, &occ);

        for (slot, id) in units.iter().copied().enumerate() {
            if clear_ticks[slot].is_none()
                && unit_clears_tunnel_exit(&entities, unit, id, exit_clear_y)
            {
                clear_ticks[slot] = Some(tick);
            }
        }

        if required_slots
            .iter()
            .all(|&slot| clear_ticks.get(slot).is_some_and(Option::is_some))
        {
            break;
        }
    }

    SnakingCorridorUnitTiming {
        unit,
        unit_count,
        clear_ticks,
        final_state: describe_snaking_corridor_state(&entities, &units),
    }
}

fn measure_snaking_corridor_clear_time(
    unit: EntityKind,
    unit_count: usize,
) -> SnakingCorridorTiming {
    let (map, exit_x, exit_clear_y, start, goal) = scout_car_snaking_corridor_map();
    let mut entities = EntityStore::new();
    let units = spawn_snaking_corridor_units(&mut entities, unit, unit_count, start);
    let occ = Occupancy::build(&map, &entities);
    for &spawned in &units {
        let e = entities.get(spawned).expect("unit should exist");
        assert!(standability::unit_static_standable_with_facing(
            &map,
            &occ,
            unit,
            e.pos_x,
            e.pos_y,
            e.facing()
        ));
    }
    assert_eq!(
        map.tile_of(start.0, start.1).0,
        exit_x,
        "units should start vertically aligned with the tunnel exit"
    );

    let mut pathing = PathingService::new(65_536, 512);
    let max_ticks = 12_000u32;
    for tick in 1..=max_ticks {
        pathing.advance_tick(tick);
        let occ = Occupancy::build(&map, &entities);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, tick);
        if tick == 1 {
            coordinator.order_group_move(&mut entities, 1, &units, goal, false);
        }
        coordinator.process_awaiting_paths(&mut entities);

        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
        let spatial = SpatialIndex::build(&entities, map.size);
        resolve_collisions(&mut entities, &spatial, &map, &occ);

        if units_clear_tunnel_exit(&entities, unit, &units, exit_clear_y) {
            return SnakingCorridorTiming {
                unit,
                unit_count,
                clear_ticks: Some(tick),
                clear_seconds: Some(tick as f32 / config::TICK_HZ as f32),
                final_state: describe_snaking_corridor_state(&entities, &units),
            };
        }
    }

    SnakingCorridorTiming {
        unit,
        unit_count,
        clear_ticks: None,
        clear_seconds: None,
        final_state: describe_snaking_corridor_state(&entities, &units),
    }
}

fn describe_snaking_corridor_state(entities: &EntityStore, units: &[u32]) -> Vec<String> {
    units
        .iter()
        .filter_map(|&id| {
            let e = entities.get(id)?;
            Some(format!(
                "#{id}: pos=({:.1},{:.1}) facing={:.3} phase={:?} path_len={} next={:?} goal={:?}",
                e.pos_x,
                e.pos_y,
                e.facing(),
                e.move_phase(),
                e.movement.as_ref().map(|m| m.path.len()).unwrap_or(0),
                e.next_waypoint(),
                e.path_goal(),
            ))
        })
        .collect()
}

#[test]
fn scout_car_snaking_corridor_clear_times() {
    let results = [
        measure_snaking_corridor_clear_time(EntityKind::ScoutCar, 1),
        measure_snaking_corridor_clear_time(EntityKind::ScoutCar, 4),
    ];
    let baselines = [(1usize, 1_577u32), (4usize, 1_654u32)];

    println!("SCOUT_CAR_SNAKING_CORRIDOR_CLEAR_TIMES");
    println!("count | clear_ticks | clear_seconds | final_state");
    for result in &results {
        match (result.clear_ticks, result.clear_seconds) {
            (Some(ticks), Some(seconds)) => println!(
                "{:>5} | {:>11} | {:>13.2} | {:?}",
                result.unit_count, ticks, seconds, result.final_state
            ),
            _ => println!(
                "{:>5} | {:>11} | {:>13} | {:?}",
                result.unit_count, "timeout", "timeout", result.final_state
            ),
        }
    }

    for result in &results {
        let baseline = baselines
            .iter()
            .find_map(|(count, ticks)| (*count == result.unit_count).then_some(*ticks))
            .expect("baseline should exist for each scout-car scenario");
        let clear_ticks = result.clear_ticks.unwrap_or_else(|| {
            panic!(
                "scout-car snaking corridor count={} timed out; final_state={:?}",
                result.unit_count, result.final_state
            )
        });
        assert!(
            clear_ticks.saturating_mul(10) <= baseline.saturating_mul(11),
            "scout-car snaking corridor count={} regressed: {} ticks vs baseline {} (>10% slower); final_state={:?}",
            result.unit_count,
            clear_ticks,
            baseline,
            result.final_state
        );
    }
}

#[test]
fn snaking_corridor_completed_unit_clear_times_regression() {
    let scenarios = [
        (EntityKind::Worker, 1usize),
        (EntityKind::Worker, 4usize),
        (EntityKind::Rifleman, 1usize),
        (EntityKind::Rifleman, 4usize),
        (EntityKind::MachineGunner, 1usize),
        (EntityKind::MachineGunner, 4usize),
        (EntityKind::AntiTankGun, 1usize),
        (EntityKind::AntiTankGun, 4usize),
        (EntityKind::ScoutCar, 1usize),
        (EntityKind::ScoutCar, 4usize),
        (EntityKind::Tank, 1usize),
        (EntityKind::Tank, 4usize),
    ];
    let results: Vec<_> = scenarios
        .par_iter()
        .map(|&(unit, count)| measure_snaking_corridor_clear_time(unit, count))
        .collect();

    println!("SNAKING_CORRIDOR_COMPLETED_UNIT_CLEAR_TIMES");
    println!("unit | count | clear_ticks | clear_seconds | final_state");
    for result in &results {
        match (result.clear_ticks, result.clear_seconds) {
            (Some(ticks), Some(seconds)) => println!(
                "{:>14} | {:>5} | {:>11} | {:>13.2} | {:?}",
                result.unit, result.unit_count, ticks, seconds, result.final_state
            ),
            _ => println!(
                "{:>14} | {:>5} | {:>11} | {:>13} | {:?}",
                result.unit, result.unit_count, "timeout", "timeout", result.final_state
            ),
        }
    }

    for result in &results {
        let baseline = SNAKING_CORRIDOR_REGRESSION_BASELINES
            .iter()
            .find_map(|(unit, count, ticks)| {
                (*unit == result.unit && *count == result.unit_count).then_some(*ticks)
            })
            .expect("baseline should exist for each completed snaking-corridor scenario");
        let clear_ticks = result.clear_ticks.unwrap_or_else(|| {
            panic!(
                "snaking corridor unit={} count={} timed out; final_state={:?}",
                result.unit, result.unit_count, result.final_state
            )
        });
        assert!(
            clear_ticks.saturating_mul(10) <= baseline.saturating_mul(11),
            "snaking corridor unit={} count={} regressed: {} ticks vs baseline {} (>10% slower); final_state={:?}",
            result.unit,
            result.unit_count,
            clear_ticks,
            baseline,
            result.final_state
        );
    }
}

#[test]
fn snaking_corridor_vehicle_slot_clear_times_regression() {
    let scenarios = [
        (EntityKind::Tank, 4usize),
        (EntityKind::AntiTankGun, 4usize),
    ];
    let results: Vec<_> = scenarios
        .par_iter()
        .map(|&(unit, count)| measure_snaking_corridor_unit_clear_times(unit, count, &[0, 1, 2, 3]))
        .collect();

    println!("SNAKING_CORRIDOR_VEHICLE_SLOT_CLEAR_TIMES");
    println!("unit | count | slot | clear_ticks | final_state");
    for &(unit, slot, _) in SNAKING_CORRIDOR_VEHICLE_SLOT_BASELINES {
        let result = results
            .iter()
            .find(|result| result.unit == unit)
            .expect("result should exist for each vehicle unit kind");
        let clear_ticks = result.clear_ticks.get(slot).copied().flatten();
        println!(
            "{:>14} | {:>5} | {:>4} | {:>11} | {:?}",
            result.unit,
            result.unit_count,
            slot,
            clear_ticks
                .map(|ticks| ticks.to_string())
                .unwrap_or_else(|| "timeout".to_string()),
            result.final_state
        );
    }

    for &(unit, slot, baseline) in SNAKING_CORRIDOR_VEHICLE_SLOT_BASELINES {
        let result = results
            .iter()
            .find(|result| result.unit == unit)
            .expect("result should exist for each vehicle unit kind");
        let clear_ticks = result
            .clear_ticks
            .get(slot)
            .copied()
            .flatten()
            .unwrap_or_else(|| {
                panic!(
                    "snaking corridor unit={} slot={} timed out; final_state={:?}",
                    unit, slot, result.final_state
                )
            });
        assert!(
            clear_ticks.saturating_mul(10) <= baseline.saturating_mul(11),
            "snaking corridor unit={} slot={} regressed: {} ticks vs baseline {} (>10% slower); final_state={:?}",
            unit,
            slot,
            clear_ticks,
            baseline,
            result.final_state
        );
    }
}

#[test]
fn scout_car_wall_chokepoint_five_clear_without_parallel_nose_wedge() {
    let unit = EntityKind::ScoutCar;
    let unit_count = 5;
    let (map, exit_clear_y, starts, goal) = scout_car_wall_chokepoint_map(unit, unit_count);
    let mut entities = EntityStore::new();
    let units = spawn_wall_chokepoint_units(&mut entities, unit, starts);
    let mut pathing = PathingService::new(65_536, 512);
    let max_ticks = 3_000u32;
    let mut max_no_progress = 0u16;

    for tick in 1..=max_ticks {
        pathing.advance_tick(tick);
        let occ = Occupancy::build(&map, &entities);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, tick);
        if tick == 1 {
            coordinator.order_group_move(&mut entities, 1, &units, goal, false);
        }
        coordinator.process_awaiting_paths(&mut entities);

        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
        let spatial = SpatialIndex::build(&entities, map.size);
        resolve_collisions(&mut entities, &spatial, &map, &occ);

        for id in &units {
            if let Some(stuck) = entities
                .get(*id)
                .and_then(|e| e.movement.as_ref())
                .map(|m| m.stuck_ticks)
            {
                max_no_progress = max_no_progress.max(stuck);
            }
        }

        if wall_chokepoint_units_clear_exit(&entities, unit, &units, exit_clear_y) {
            assert!(
                tick <= 300,
                "five scout cars should clear the chokepoint promptly; tick={tick}, state={:?}",
                describe_units(&entities, &units)
            );
            assert!(
                max_no_progress < config::SCOUT_CAR_STUCK_RECOVERY_TRIGGER_TICKS,
                "scout cars should not need stuck recovery in the open approach; max_no_progress={max_no_progress}, state={:?}",
                describe_units(&entities, &units)
            );
            return;
        }
    }

    panic!(
        "five scout cars failed to clear wall chokepoint within {max_ticks} ticks; state={:?}",
        describe_units(&entities, &units)
    );
}

#[test]
fn wall_chokepoint_case_clear_time_regression() {
    let results: Vec<_> = WALL_CHOKEPOINT_SCORE_BASELINES
        .par_iter()
        .map(|&(unit, count, _)| measure_wall_chokepoint_clear_times(unit, count))
        .collect();

    println!("WALL_CHOKEPOINT_CLEAR_TIMES");
    println!("unit | count | clear_ticks | score_ticks | case_score | baseline");
    for result in &results {
        let mut clear_tick_cells = Vec::with_capacity(result.clear_ticks.len());
        let mut score_cells = Vec::with_capacity(result.clear_ticks.len());
        for (slot, clear_ticks) in result.clear_ticks.iter().enumerate() {
            let score = clear_ticks.unwrap_or(WALL_CHOKEPOINT_MAX_TICKS);
            clear_tick_cells.push(format!(
                "{slot}:{}",
                clear_ticks
                    .map(|ticks| ticks.to_string())
                    .unwrap_or_else(|| "timeout".to_string())
            ));
            score_cells.push(format!("{slot}:{score}"));
        }
        let case_score = wall_chokepoint_case_score(result);
        let baseline = wall_chokepoint_score_baseline(result.unit, result.unit_count);
        println!(
            "{:>14} | {:>5} | {:>11} | {:>11} | {:>10} | {:>8}",
            result.unit,
            result.unit_count,
            clear_tick_cells.join(","),
            score_cells.join(","),
            case_score,
            baseline
        );

        assert!(
            case_score.saturating_mul(10) <= baseline.saturating_mul(11),
            "wall chokepoint unit={} count={} regressed: {} ticks vs baseline {} (>10% slower); results={:?}",
            result.unit,
            result.unit_count,
            case_score,
            baseline,
            wall_chokepoint_timing_summary(&results)
        );
    }
}

#[test]
#[ignore = "manual timing sweep across all unit types; prints timings and allows timeouts"]
fn snaking_corridor_clear_times_all_units() {
    let scenarios = [
        (EntityKind::Worker, 1usize),
        (EntityKind::Worker, 4usize),
        (EntityKind::Rifleman, 1usize),
        (EntityKind::Rifleman, 4usize),
        (EntityKind::MachineGunner, 1usize),
        (EntityKind::MachineGunner, 4usize),
        (EntityKind::AntiTankGun, 1usize),
        (EntityKind::AntiTankGun, 4usize),
        (EntityKind::ScoutCar, 1usize),
        (EntityKind::ScoutCar, 4usize),
        (EntityKind::Tank, 1usize),
        (EntityKind::Tank, 4usize),
    ];
    let results: Vec<_> = scenarios
        .into_iter()
        .map(|(unit, count)| measure_snaking_corridor_clear_time(unit, count))
        .collect();

    println!("SNAKING_CORRIDOR_CLEAR_TIMES");
    println!("unit | count | clear_ticks | clear_seconds | final_state");
    for result in &results {
        match (result.clear_ticks, result.clear_seconds) {
            (Some(ticks), Some(seconds)) => println!(
                "{:>14} | {:>5} | {:>11} | {:>13.2} | {:?}",
                result.unit, result.unit_count, ticks, seconds, result.final_state
            ),
            _ => println!(
                "{:>14} | {:>5} | {:>11} | {:>13} | {:?}",
                result.unit, result.unit_count, "timeout", "timeout", result.final_state
            ),
        }
    }
}

#[test]
fn scout_car_phase0_factory_corner_graze_cardinal_approaches() {
    let ts = config::TILE_SIZE as f32;
    let clearance = scout_car_body_half_width() + 8.0;

    for name in [
        "west_to_north",
        "north_to_east",
        "east_to_south",
        "south_to_west",
    ] {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Factory, 20, 20);
        entities
            .spawn_building(2, EntityKind::Factory, bx, by, true)
            .expect("factory should spawn");
        let rect = building_rect_for_footprint(EntityKind::Factory, 20, 20).expect("factory rect");
        let (start, goal, facing) = match name {
            "west_to_north" => (
                (rect.min_x - ts * 4.0, rect.max_y + clearance),
                (rect.max_x + clearance, rect.min_y - ts * 4.0),
                0.0,
            ),
            "north_to_east" => (
                (rect.min_x - clearance, rect.min_y - ts * 4.0),
                (rect.max_x + ts * 4.0, rect.max_y + clearance),
                std::f32::consts::FRAC_PI_2,
            ),
            "east_to_south" => (
                (rect.max_x + ts * 4.0, rect.min_y - clearance),
                (rect.min_x - clearance, rect.max_y + ts * 4.0),
                std::f32::consts::PI,
            ),
            "south_to_west" => (
                (rect.max_x + clearance, rect.max_y + ts * 4.0),
                (rect.min_x - ts * 4.0, rect.min_y - clearance),
                -std::f32::consts::FRAC_PI_2,
            ),
            _ => unreachable!("case list is exhaustive"),
        };
        let scout = spawn_ordered_scout_car(&mut entities, start, goal, facing);

        let baseline =
            measure_scout_car_fixture(name, &map, &mut entities, scout, goal, 1_200, true);

        baseline.assert_reference_envelope(name);
        assert!(
            baseline.min_static_clearance_px <= 2.0,
            "{name}: corner graze fixture should exercise tight static clearance: {:?}",
            baseline
        );
        assert!(
            baseline.static_blocked_step_attempts <= 45,
            "{name}: corner graze repeatedly aimed into static blockers: {:?}",
            baseline
        );
    }
}

#[test]
fn scout_car_phase0_two_building_alley() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    for tile_y in [16, 20] {
        let (bx, by) = footprint_center(&map, EntityKind::Depot, 18, tile_y);
        entities
            .spawn_building(2, EntityKind::Depot, bx, by, true)
            .expect("depot should spawn");
    }
    let lower = building_rect_for_footprint(EntityKind::Depot, 18, 16).expect("lower depot rect");
    let upper = building_rect_for_footprint(EntityKind::Depot, 18, 20).expect("upper depot rect");
    let lane_y = (lower.max_y + upper.min_y) * 0.5;
    let start = (lower.min_x - config::TILE_SIZE as f32 * 5.0, lane_y);
    let goal = (lower.max_x + config::TILE_SIZE as f32 * 5.0, lane_y);
    let scout = spawn_ordered_scout_car(&mut entities, start, goal, 0.0);

    let baseline = measure_scout_car_fixture(
        "two_building_alley",
        &map,
        &mut entities,
        scout,
        goal,
        900,
        true,
    );

    baseline.assert_reference_envelope("two_building_alley");
    assert!(
        baseline.min_static_clearance_px <= 16.0,
        "fixture should exercise a narrow lane, got {:?}",
        baseline
    );
    assert!(
        baseline.static_blocked_step_attempts <= 10,
        "scout car should not repeatedly hit alley corners: {:?}",
        baseline
    );
}

#[test]
fn scout_car_phase0_diagonal_pinch_avoids_one_tile_gap() {
    let mut map = flat_map(1);
    block_rect_tiles(&mut map, 10, 10, 12, 12);
    block_rect_tiles(&mut map, 14, 13, 16, 15);
    let mut entities = EntityStore::new();
    let start = map.tile_center(9, 15);
    let goal = map.tile_center(18, 10);
    let scout = spawn_ordered_scout_car(&mut entities, start, goal, -std::f32::consts::FRAC_PI_4);

    let baseline = measure_scout_car_fixture(
        "diagonal_pinch",
        &map,
        &mut entities,
        scout,
        goal,
        1_200,
        true,
    );

    baseline.assert_reference_envelope("diagonal_pinch");
    assert!(
        baseline.min_static_clearance_px >= 0.0,
        "diagonal pinch route must never accept a clipping pose: {:?}",
        baseline
    );
    assert!(
        baseline.max_consecutive_no_progress_ticks < 90,
        "diagonal pinch should not wedge the scout car: {:?}",
        baseline
    );
}

#[test]
fn scout_car_phase0_wall_parallel_lane_does_not_ram_wall() {
    let mut map = flat_map(1);
    for tx in 8..=30 {
        let idx = map.index(tx, 16);
        map.terrain[idx] = crate::protocol::terrain::ROCK;
    }
    let mut entities = EntityStore::new();
    let start = map.tile_center(8, 17);
    let goal = map.tile_center(30, 17);
    let scout = spawn_ordered_scout_car(&mut entities, start, goal, 0.0);

    let baseline = measure_scout_car_fixture(
        "wall_parallel_lane",
        &map,
        &mut entities,
        scout,
        goal,
        900,
        true,
    );

    baseline.assert_reference_envelope("wall_parallel_lane");
    assert_eq!(
        baseline.reverse_recovery_activations, 0,
        "parallel wall lane should not need reverse recovery: {:?}",
        baseline
    );
    assert!(
        baseline.static_blocked_step_attempts <= 5,
        "wall-parallel route should not repeatedly aim into the wall: {:?}",
        baseline
    );
}

#[test]
fn scout_car_phase0_blocked_nose_recovery_is_measurable() {
    let (map, mut entities, scout, _start, goal, _initial_facing) =
        front_blocked_scout_car_fixture();

    let baseline = measure_scout_car_fixture(
        "blocked_nose_recovery",
        &map,
        &mut entities,
        scout,
        goal,
        180,
        false,
    );

    assert!(
        !baseline.accepted_static_illegal_pose,
        "recovery fixture must never accept a clipping pose: {:?}",
        baseline
    );
    baseline.assert_reference_envelope("blocked_nose_recovery");
    assert!(
        baseline.static_blocked_step_attempts == 0,
        "capsule geometry should clear the former blocked-nose fixture without static step churn: {:?}",
        baseline
    );
    assert!(
        baseline.reverse_recovery_activations == 0,
        "capsule geometry should not need reverse recovery in the former blocked-nose fixture: {:?}",
        baseline
    );
}

#[test]
fn scout_car_phase0_traffic_compression_near_factory() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Factory, 20, 18);
    entities
        .spawn_building(2, EntityKind::Factory, bx, by, true)
        .expect("factory should spawn");
    let rect = building_rect_for_footprint(EntityKind::Factory, 20, 18).expect("factory rect");
    let lane_y = rect.max_y + scout_car_body_half_width() + 12.0;
    let start = (rect.min_x - config::TILE_SIZE as f32 * 5.0, lane_y);
    let goal = (rect.max_x + config::TILE_SIZE as f32 * 5.0, lane_y);
    let scout = spawn_ordered_scout_car(&mut entities, start, goal, 0.0);

    for (dx, dy, kind) in [
        (-54.0, -12.0, EntityKind::Rifleman),
        (-28.0, 18.0, EntityKind::Rifleman),
        (18.0, -14.0, EntityKind::MachineGunner),
        (54.0, 16.0, EntityKind::Rifleman),
        (86.0, 0.0, EntityKind::Tank),
    ] {
        let id = entities
            .spawn_unit(2, kind, rect.min_x + dx, lane_y + dy)
            .expect("traffic unit should spawn");
        if let Some(e) = entities.get_mut(id) {
            e.set_facing(0.0);
        }
    }

    let baseline = measure_scout_car_fixture(
        "traffic_compression",
        &map,
        &mut entities,
        scout,
        goal,
        1_200,
        true,
    );

    assert!(
        !baseline.accepted_static_illegal_pose,
        "traffic compression must not accept static clipping even when wedged: {:?}",
        baseline
    );
    assert!(
        baseline.path_length_px > moved_distance(start, goal),
        "traffic compression should show churn around the factory lane: {:?}",
        baseline
    );
    assert!(
        baseline.final_error_px <= config::TILE_SIZE as f32,
        "capsule geometry should keep the scout car mobile through traffic compression: {:?}",
        baseline
    );
    assert!(
        baseline.reverse_recovery_activations == 0,
        "capsule geometry should avoid reverse recovery churn in this traffic fixture: {:?}",
        baseline
    );
    assert!(
        baseline.collision_displacement_px > config::TILE_SIZE as f32,
        "traffic compression should record collision pressure from nearby units: {:?}",
        baseline
    );
    assert!(
        baseline.repath_count <= 4,
        "traffic fixture should distinguish clean progress from repeated repaths: {:?}",
        baseline
    );
}

#[test]
fn tank_with_zero_oil_does_not_move() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("tank should spawn");
    set_path_direct(&mut entities, tank, vec![(sx + 128.0, sy)]);
    if let Some(e) = entities.get_mut(tank) {
        e.set_order(Order::move_to(sx + 128.0, sy));
    }
    let mut players = vec![player_with_oil(1, 0)];

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut players, &occ, &spatial, 0);

    assert_eq!(pos(&entities, tank), (sx, sy));
    assert_eq!(
        entities.get(tank).and_then(|e| e.lifetime_oil_used()),
        Some(0.0)
    );
    assert_eq!(
        entities
            .get(tank)
            .and_then(|e| e.movement.as_ref())
            .map(|m| m.oil_starved_pause_ticks),
        Some(config::TANK_OIL_STARVED_PAUSE_TICKS - 1)
    );
}

#[test]
fn scout_car_with_zero_oil_does_not_move() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let scout_car = entities
        .spawn_unit(1, EntityKind::ScoutCar, sx, sy)
        .expect("scout car should spawn");
    set_path_direct(&mut entities, scout_car, vec![(sx + 128.0, sy)]);
    if let Some(e) = entities.get_mut(scout_car) {
        e.set_order(Order::move_to(sx + 128.0, sy));
        e.set_facing(0.0);
    }
    let mut players = vec![player_with_oil(1, 0)];

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut players, &occ, &spatial, 0);

    assert_eq!(pos(&entities, scout_car), (sx, sy));
    assert_eq!(
        entities
            .get(scout_car)
            .and_then(|e| e.movement.as_ref())
            .map(|m| m.oil_starved_pause_ticks),
        Some(config::TANK_OIL_STARVED_PAUSE_TICKS - 1)
    );
}

#[test]
fn tank_oil_starvation_pauses_before_retrying() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("tank should spawn");
    set_path_direct(&mut entities, tank, vec![(sx + 128.0, sy)]);
    if let Some(e) = entities.get_mut(tank) {
        e.set_order(Order::move_to(sx + 128.0, sy));
        e.set_facing(0.0);
    }
    let mut players = vec![player_with_oil(1, 0)];

    for tick in 0..config::TANK_OIL_STARVED_PAUSE_TICKS as u32 {
        if tick == 1 {
            players[0].oil = 1;
        }
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut players, &occ, &spatial, tick);
        assert_eq!(
            pos(&entities, tank),
            (sx, sy),
            "tank should stay paused on tick {tick}"
        );
    }

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(
        &map,
        &mut entities,
        &mut players,
        &occ,
        &spatial,
        config::TANK_OIL_STARVED_PAUSE_TICKS as u32,
    );

    assert!(
        moved_distance((sx, sy), pos(&entities, tank)) > 0.01,
        "tank should retry movement after the pause when oil is available"
    );
}

#[test]
fn tank_oil_starvation_emits_positioned_oil_alert_once_per_pause() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("tank should spawn");
    set_path_direct(&mut entities, tank, vec![(sx + 128.0, sy)]);
    if let Some(e) = entities.get_mut(tank) {
        e.set_order(Order::move_to(sx + 128.0, sy));
    }
    let mut players = vec![player_with_oil(1, 0)];
    let mut events = HashMap::new();
    let smokes = crate::game::smoke::SmokeCloudStore::new();
    let ability_runtime = crate::game::ability_runtime::AbilityRuntime::new();

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system_with_events(
        &map,
        &mut entities,
        &mut players,
        &occ,
        &spatial,
        0,
        &mut events,
        &smokes,
        &ability_runtime,
    );

    assert!(
        events.get(&1).is_some_and(|events| {
            matches!(
                events.as_slice(),
                [Event::Notice {
                    msg,
                    x: Some(x),
                    y: Some(y),
                    severity: NoticeSeverity::Alert,
                }] if msg == "alert:out_of_oil"
                    && (*x - sx).abs() < 0.001
                    && (*y - sy).abs() < 0.001
            )
        }),
        "starved tank should emit a positioned oil alert: {events:?}"
    );

    events.clear();
    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system_with_events(
        &map,
        &mut entities,
        &mut players,
        &occ,
        &spatial,
        1,
        &mut events,
        &smokes,
        &ability_runtime,
    );

    assert!(
        events.get(&1).map_or(true, Vec::is_empty),
        "cooldown ticks should not repeat the oil alert: {events:?}"
    );
}

#[test]
fn moving_tank_accrues_lifetime_oil_and_charges_player_stockpile() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("tank should spawn");
    set_path_direct(&mut entities, tank, vec![(sx + 360.0, sy)]);
    if let Some(e) = entities.get_mut(tank) {
        e.set_order(Order::move_to(sx + 360.0, sy));
        e.set_facing(0.0);
    }
    let mut players = vec![player_with_oil(1, 10)];

    let mut total_moved = 0.0;
    for tick in 0..300u32 {
        let before = pos(&entities, tank);
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut players, &occ, &spatial, tick);
        let after = pos(&entities, tank);
        total_moved += moved_distance(before, after);
        if total_moved >= 330.0 {
            break;
        }
    }

    let oil_used = entities
        .get(tank)
        .and_then(|e| e.lifetime_oil_used())
        .expect("tank should report oil used");
    let expected = total_moved * config::TANK_OIL_COST_PER_PX;
    assert!(
        (oil_used - expected).abs() <= 0.001,
        "expected oil used {expected:.4}, got {oil_used:.4}"
    );
    assert!(
        oil_used >= 1.0,
        "test should move far enough to burn at least one oil, got {oil_used:.4}"
    );
    assert_eq!(players[0].oil, 10 - oil_used.floor() as u32);
}

#[test]
fn moving_scout_car_accrues_lifetime_oil_and_charges_player_stockpile() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let scout_car = entities
        .spawn_unit(1, EntityKind::ScoutCar, sx, sy)
        .expect("scout car should spawn");
    set_path_direct(&mut entities, scout_car, vec![(sx + 720.0, sy)]);
    if let Some(e) = entities.get_mut(scout_car) {
        e.set_order(Order::move_to(sx + 720.0, sy));
        e.set_facing(0.0);
    }
    let mut players = vec![player_with_oil(1, 10)];

    let mut total_moved = 0.0;
    for tick in 0..400u32 {
        let before = pos(&entities, scout_car);
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut players, &occ, &spatial, tick);
        let after = pos(&entities, scout_car);
        total_moved += moved_distance(before, after);
        if total_moved >= 620.0 {
            break;
        }
    }

    let oil_used = entities
        .get(scout_car)
        .and_then(|e| e.movement.as_ref())
        .map(|m| m.lifetime_oil_used)
        .expect("scout car should have movement state");
    let expected = total_moved * config::SCOUT_CAR_OIL_COST_PER_PX;
    assert!(
        (oil_used - expected).abs() <= 0.001,
        "expected oil used {expected:.4}, got {oil_used:.4}"
    );
    assert!(
        oil_used >= 1.0,
        "test should move far enough to burn at least one oil, got {oil_used:.4}"
    );
    assert_eq!(players[0].oil, 10 - oil_used.floor() as u32);
}

#[test]
fn tank_route_lookahead_uses_long_open_segment() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("tank should spawn");

    set_path_direct(
        &mut entities,
        tank,
        vec![
            (sx + config::TILE_SIZE as f32, sy),
            (sx + config::TILE_SIZE as f32 * 2.0, sy),
            (sx + config::TILE_SIZE as f32 * 3.0, sy),
            (sx + config::TILE_SIZE as f32 * 4.0, sy),
            (sx + config::TILE_SIZE as f32 * 8.0, sy),
        ],
    );

    let occ = Occupancy::build(&map, &entities);
    let e = entities.get(tank).expect("tank should exist");
    let desired = pivot_drive_desired_path_point(&map, &occ, e, sx, sy)
        .expect("tank should have route intent");

    assert!(
        (desired.0 - (sx + PIVOT_VEHICLE_LOOKAHEAD_PX)).abs() <= 0.001,
        "open route intent should use the long tank lookahead, got x {:.2} from start {:.2}",
        desired.0,
        sx
    );
    assert!(
        (desired.1 - sy).abs() <= 0.001,
        "open route intent should stay on the route segment, got y {:.2}",
        desired.1
    );
}

#[test]
fn tank_route_lookahead_stops_before_blocked_corner() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Depot, 10, 10);
    entities
        .spawn_building(1, EntityKind::Depot, bx, by, true)
        .expect("depot spawn");
    let rect = building_rect_for_footprint(EntityKind::Depot, 10, 10).expect("depot rect");
    let tank_radius = config::unit_stats(EntityKind::Tank)
        .expect("tank stats")
        .radius;

    let start = (
        rect.min_x - tank_radius - 8.0,
        rect.max_y + tank_radius + 8.0,
    );
    let corner = (rect.max_x + tank_radius + 8.0, start.1);
    let after_corner = (corner.0, rect.min_y - tank_radius - 8.0);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank spawn");
    set_path_direct(&mut entities, tank, vec![corner, after_corner]);

    let occ = Occupancy::build(&map, &entities);
    assert!(
        standability::unit_static_segment_standable(&map, &occ, EntityKind::Tank, start, corner),
        "fixture requires a legal current route segment"
    );
    assert!(
        !standability::unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::Tank,
            start,
            after_corner
        ),
        "fixture requires the direct look-through-corner segment to be blocked"
    );

    let e = entities.get(tank).expect("tank should exist");
    let desired =
        pivot_drive_desired_path_point(&map, &occ, e, start.0, start.1).expect("route intent");

    assert!(
        (desired.1 - start.1).abs() <= 0.001,
        "tank intent should stay on the legal segment before the corner, got {:?}",
        desired
    );
    assert!(
        desired.0 > start.0 && desired.0 <= corner.0,
        "tank intent should advance along the current segment only, got {:?}",
        desired
    );
}

#[test]
fn scout_car_route_lookahead_skips_lateral_waypoint_when_next_segment_reachable() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let lateral_offset = config::VEHICLE_WAYPOINT_ACCEPTANCE_RADIUS_PX + 10.0;
    let start = (sx, sy + lateral_offset);
    let intermediate = (sx + config::TILE_SIZE as f32, sy);
    let goal = (sx + config::TILE_SIZE as f32 * 8.0, sy);
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, start.0, start.1)
        .expect("scout car should spawn");
    if let Some(e) = entities.get_mut(scout) {
        e.set_facing(0.0);
        e.set_order(Order::move_to(goal.0, goal.1));
    }
    set_path_direct(&mut entities, scout, vec![intermediate, goal]);

    let occ = Occupancy::build(&map, &entities);
    let e = entities.get(scout).expect("scout car should exist");
    let desired = scout_car_desired_path_point(&map, &occ, e, start.0, start.1)
        .expect("scout car should have route intent");
    assert!(
        moved_distance(start, desired) >= SCOUT_CAR_ROUTE_LOOKAHEAD_PX - 0.001,
        "lookahead should aim past the lateral waypoint along the reachable route, got {:?}",
        desired
    );
    assert_ne!(
        desired, intermediate,
        "scout car should not chase the lateral intermediate waypoint center"
    );

    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(scout).expect("scout car should exist");
    assert_eq!(
        e.movement.as_ref().map(|m| m.path.len()),
        Some(1),
        "statically reachable next segment should consume the lateral intermediate waypoint"
    );
    assert_eq!(e.next_waypoint(), Some(goal));
    assert!(
        e.pos_x > start.0,
        "scout car should drive down the reachable route after consuming the waypoint"
    );
}

#[test]
fn infantry_skips_lateral_waypoint_when_next_segment_reachable() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let lateral_offset = config::VEHICLE_WAYPOINT_ACCEPTANCE_RADIUS_PX + 10.0;
    let start = (sx, sy + lateral_offset);
    let intermediate = (sx + config::TILE_SIZE as f32, sy);
    let goal = (sx + config::TILE_SIZE as f32 * 4.0, sy);
    let rifleman = entities
        .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
        .expect("rifleman should spawn");
    if let Some(e) = entities.get_mut(rifleman) {
        e.set_order(Order::move_to(goal.0, goal.1));
    }
    set_path_direct(&mut entities, rifleman, vec![intermediate, goal]);

    let occ = Occupancy::build(&map, &entities);
    assert!(
        standability::unit_static_segment_standable(&map, &occ, EntityKind::Rifleman, start, goal),
        "fixture requires the infantry body to sweep directly to the next route segment"
    );
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(rifleman).expect("rifleman should exist");
    assert_eq!(
        e.movement.as_ref().map(|m| m.path.len()),
        Some(1),
        "reachable intermediate waypoint should be consumed"
    );
    assert_eq!(e.next_waypoint(), Some(goal));
    assert!(
        e.pos_x > start.0,
        "rifleman should move along the reachable route after consuming the waypoint"
    );
}

#[test]
fn worker_keeps_lateral_waypoint_when_next_segment_reachable() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let lateral_offset = config::VEHICLE_WAYPOINT_ACCEPTANCE_RADIUS_PX + 10.0;
    let start = (sx, sy + lateral_offset);
    let intermediate = (sx + config::TILE_SIZE as f32, sy);
    let goal = (sx + config::TILE_SIZE as f32 * 4.0, sy);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, start.0, start.1)
        .expect("worker should spawn");
    if let Some(e) = entities.get_mut(worker) {
        e.set_order(Order::move_to(goal.0, goal.1));
    }
    set_path_direct(&mut entities, worker, vec![intermediate, goal]);

    let occ = Occupancy::build(&map, &entities);
    assert!(
        standability::unit_static_segment_standable(&map, &occ, EntityKind::Worker, start, goal),
        "fixture requires the worker body to have a legal direct route segment"
    );
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(worker).expect("worker should exist");
    assert_eq!(
        e.movement.as_ref().map(|m| m.path.len()),
        Some(2),
        "workers should not consume intermediate waypoints via route skipping"
    );
    assert_eq!(e.next_waypoint(), Some(intermediate));
    assert!(
        e.pos_x > start.0 && e.pos_y < start.1,
        "worker should move toward the intermediate waypoint instead of skipping to the route"
    );
}

#[test]
fn infantry_route_skip_stops_before_blocked_corner() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Depot, 10, 10);
    entities
        .spawn_building(1, EntityKind::Depot, bx, by, true)
        .expect("depot spawn");
    let rect = building_rect_for_footprint(EntityKind::Depot, 10, 10).expect("depot rect");
    let infantry_radius = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman stats")
        .radius;

    let start = (
        rect.min_x - infantry_radius - 8.0,
        rect.max_y + infantry_radius + 8.0,
    );
    let corner = (rect.max_x + infantry_radius + 8.0, start.1);
    let after_corner = (corner.0, rect.min_y - infantry_radius - 8.0);
    let rifleman = entities
        .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
        .expect("rifleman spawn");
    if let Some(e) = entities.get_mut(rifleman) {
        e.set_order(Order::move_to(after_corner.0, after_corner.1));
    }
    set_path_direct(&mut entities, rifleman, vec![corner, after_corner]);

    let occ = Occupancy::build(&map, &entities);
    assert!(
        standability::unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::Rifleman,
            start,
            corner
        ),
        "fixture requires a legal current route segment"
    );
    assert!(
        !standability::unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::Rifleman,
            start,
            after_corner
        ),
        "fixture requires the direct look-through-corner segment to be blocked"
    );
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(rifleman).expect("rifleman should exist");
    assert_eq!(
        e.movement.as_ref().map(|m| m.path.len()),
        Some(2),
        "blocked look-through-corner segment should keep the intermediate waypoint"
    );
    assert_eq!(e.next_waypoint(), Some(corner));
}

#[test]
fn scout_car_route_lookahead_stops_before_blocked_corner() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Depot, 10, 10);
    entities
        .spawn_building(1, EntityKind::Depot, bx, by, true)
        .expect("depot spawn");
    let rect = building_rect_for_footprint(EntityKind::Depot, 10, 10).expect("depot rect");
    let clearance = scout_car_body_half_width() + 8.0;

    let start = (
        rect.min_x - config::TILE_SIZE as f32 * 2.0,
        rect.max_y + clearance,
    );
    let corner = (rect.max_x + config::TILE_SIZE as f32 * 2.0, start.1);
    let after_corner = (corner.0, rect.min_y - clearance);
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, start.0, start.1)
        .expect("scout car spawn");
    if let Some(e) = entities.get_mut(scout) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, scout, vec![corner, after_corner]);

    let occ = Occupancy::build(&map, &entities);
    assert!(
        standability::unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::ScoutCar,
            start,
            corner
        ),
        "fixture requires a legal current route segment"
    );
    assert!(
        !standability::unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::ScoutCar,
            start,
            after_corner
        ),
        "fixture requires the direct look-through-corner segment to be blocked"
    );

    let e = entities.get(scout).expect("scout car should exist");
    let desired =
        scout_car_desired_path_point(&map, &occ, e, start.0, start.1).expect("route intent");

    assert!(
        (desired.1 - start.1).abs() <= 0.001,
        "scout car intent should stay on the legal segment before the corner, got {:?}",
        desired
    );
    assert!(
        desired.0 > start.0 && desired.0 <= corner.0,
        "scout car intent should advance along the current segment only, got {:?}",
        desired
    );
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

#[test]
fn moving_unit_steers_around_braced_unit_when_space_exists() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let mover = entities
        .spawn_unit(1, EntityKind::Rifleman, sx, sy)
        .expect("mover spawn");
    let blocker = entities
        .spawn_unit(2, EntityKind::MachineGunner, sx + 34.0, sy + 8.0)
        .expect("blocker spawn");
    if let Some(e) = entities.get_mut(blocker) {
        e.set_weapon_setup(WeaponSetup::Deployed);
    }
    set_path_direct(&mut entities, mover, vec![(sx + 200.0, sy)]);
    if let Some(e) = entities.get_mut(mover) {
        e.set_order(Order::move_to(sx + 200.0, sy));
    }

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let after = pos(&entities, mover);
    assert!(after.0 > sx, "mover should still progress along its path");
    assert!(
        after.1 < sy - 0.05,
        "mover should gain lateral separation from the braced unit, before y {:.2}, after {:.2}",
        sy,
        after.1
    );
}

#[test]
fn moving_infantry_skirting_tank_trap_line_gets_small_lateral_bias() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(8, 12);
    let (trap_x, _) = map.tile_center(12, 12);
    for dy in [-1.0_f32, 0.0, 1.0] {
        entities
            .spawn_building(
                2,
                EntityKind::TankTrap,
                trap_x,
                sy + dy * config::TILE_SIZE as f32,
                true,
            )
            .expect("tank trap spawn");
    }
    let mover = entities
        .spawn_unit(1, EntityKind::Rifleman, sx, sy)
        .expect("mover spawn");
    let goal = (sx + config::TILE_SIZE as f32 * 8.0, sy);
    set_path_direct(&mut entities, mover, vec![goal]);
    if let Some(e) = entities.get_mut(mover) {
        e.set_order(Order::move_to(goal.0, goal.1));
    }

    let mut max_lateral = 0.0_f32;
    for tick in 0..120 {
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
        let after = pos(&entities, mover);
        max_lateral = max_lateral.max((after.1 - sy).abs());
        if after.0 > trap_x + config::TILE_SIZE as f32 {
            break;
        }
    }

    let after = pos(&entities, mover);
    assert!(
        after.0 > trap_x + config::TILE_SIZE as f32,
        "infantry should still pass through the vehicle-only trap line, pos={after:?}"
    );
    assert!(
        max_lateral > 1.0,
        "infantry should visibly skirt the tank trap line, max lateral {max_lateral:.3}px"
    );
    assert!(
        max_lateral < config::TILE_SIZE as f32,
        "tank trap steering should stay cosmetic, max lateral {max_lateral:.3}px"
    );
}

#[test]
fn choke_still_clogs_when_no_space_exists() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Depot, 10, 10);
    entities
        .spawn_building(1, EntityKind::Depot, bx, by, true)
        .expect("depot spawn");
    let rect = building_rect_for_footprint(EntityKind::Depot, 10, 10).expect("depot rect");
    let start = (rect.min_x - tank_body_half_len() - 0.1, rect.min_y + 32.0);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank spawn");
    let blocker = entities
        .spawn_unit(2, EntityKind::MachineGunner, start.0 - 12.0, start.1 - 18.0)
        .expect("blocker spawn");
    if let Some(e) = entities.get_mut(blocker) {
        e.set_weapon_setup(WeaponSetup::Deployed);
    }
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, tank, vec![(rect.max_x + 64.0, start.1)]);
    if let Some(e) = entities.get_mut(tank) {
        e.set_order(Order::move_to(rect.max_x + 64.0, start.1));
    }

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let after = pos(&entities, tank);
    assert!(
        moved_distance(start, after) <= 0.01,
        "steering must not move a tank through a blocked choke, moved from {:?} to {:?}",
        start,
        after
    );
    assert!(tank_standable_at_entity_facing(&map, &occ, &entities, tank));
}

#[test]
fn tank_frontal_traffic_slows_without_sidestep_waypoint() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("tank spawn");
    let blocker = entities
        .spawn_unit(
            2,
            EntityKind::MachineGunner,
            sx + tank_body_half_len() + 10.0,
            sy,
        )
        .expect("blocker spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(0.0);
        e.set_order(Order::move_to(sx + 200.0, sy));
    }
    if let Some(e) = entities.get_mut(blocker) {
        e.set_weapon_setup(WeaponSetup::Deployed);
    }
    set_path_direct(&mut entities, tank, vec![(sx + 200.0, sy)]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let after = pos(&entities, tank);
    let moved = moved_distance((sx, sy), after);
    assert!(
        moved
            < config::unit_stats(EntityKind::Tank)
                .expect("tank stats")
                .speed,
        "frontal braced traffic should reduce tank throttle, moved {moved:.3}px"
    );
    assert!(
        (after.1 - sy).abs() <= 0.001,
        "tank traffic avoidance should not inject a perpendicular sidestep"
    );
    assert_eq!(
        entities
            .get(tank)
            .and_then(|e| e.movement.as_ref().map(|m| m.path.len())),
        Some(1),
        "tank should keep its original route instead of adding sidestep waypoints"
    );
}

#[test]
fn steering_ignores_ghost_harvester() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let mover = entities
        .spawn_unit(1, EntityKind::Rifleman, sx, sy)
        .expect("mover spawn");
    let node = entities
        .spawn_node(EntityKind::Steel, sx + 34.0, sy + 8.0)
        .expect("node spawn");
    let harvester = entities
        .spawn_unit(1, EntityKind::Worker, sx + 34.0, sy + 8.0)
        .expect("harvester spawn");
    if let Some(e) = entities.get_mut(harvester) {
        e.set_order(Order::gather(node));
        e.mark_gather_phase(GatherPhase::Harvesting);
    }
    set_path_direct(&mut entities, mover, vec![(sx + 200.0, sy)]);
    if let Some(e) = entities.get_mut(mover) {
        e.set_order(Order::move_to(sx + 200.0, sy));
    }

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let after = pos(&entities, mover);
    assert!(after.0 > sx, "mover should still progress along its path");
    assert!(
        (after.1 - sy).abs() <= 0.001,
        "ghost harvester should not create steering displacement, before y {:.2}, after {:.2}",
        sy,
        after.1
    );
}

#[test]
fn steering_candidate_rejected_when_body_would_clip_building() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Depot, 10, 10);
    entities
        .spawn_building(1, EntityKind::Depot, bx, by, true)
        .expect("depot spawn");
    let rect = building_rect_for_footprint(EntityKind::Depot, 10, 10).expect("depot rect");
    let start = (rect.min_x - 5.5, rect.min_y - 8.5);
    let mover = entities
        .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
        .expect("mover spawn");
    let blocker = entities
        .spawn_unit(2, EntityKind::MachineGunner, start.0 + 24.0, start.1 - 12.0)
        .expect("blocker spawn");
    if let Some(e) = entities.get_mut(blocker) {
        e.set_weapon_setup(WeaponSetup::Deployed);
    }
    set_path_direct(&mut entities, mover, vec![(rect.max_x + 64.0, start.1)]);
    if let Some(e) = entities.get_mut(mover) {
        e.set_order(Order::move_to(rect.max_x + 64.0, start.1));
    }

    let occ = Occupancy::build(&map, &entities);
    assert!(standability::unit_static_standable(
        &map,
        &occ,
        EntityKind::Rifleman,
        start.0,
        start.1
    ));
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let after = pos(&entities, mover);
    assert!(
        after.0 > start.0,
        "blocked steered candidate should fall back to the direct legal path step"
    );
    assert!(
        (after.1 - start.1).abs() <= 0.001,
        "body-clipping steered candidate must be rejected, before y {:.2}, after {:.2}",
        start.1,
        after.1
    );
    assert!(standability::unit_static_standable(
        &map,
        &occ,
        EntityKind::Rifleman,
        after.0,
        after.1
    ));
}

fn steering_neighbor_cap_position() -> (f32, f32) {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let mover = entities
        .spawn_unit(1, EntityKind::Rifleman, sx, sy)
        .expect("mover spawn");
    for i in 0..32u32 {
        let angle = i as f32 * 0.37;
        let d = 28.0 + (i % 5) as f32 * 3.0;
        let id = entities
            .spawn_unit(
                2,
                if i % 3 == 0 {
                    EntityKind::MachineGunner
                } else {
                    EntityKind::Rifleman
                },
                sx + angle.cos() * d,
                sy + angle.sin() * d,
            )
            .expect("neighbor spawn");
        if i % 3 == 0 {
            if let Some(e) = entities.get_mut(id) {
                e.set_weapon_setup(WeaponSetup::Deployed);
            }
        }
    }
    set_path_direct(&mut entities, mover, vec![(sx + 200.0, sy)]);
    if let Some(e) = entities.get_mut(mover) {
        e.set_order(Order::move_to(sx + 200.0, sy));
    }

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);
    pos(&entities, mover)
}

#[test]
fn steering_neighbor_cap_is_deterministic() {
    let first = steering_neighbor_cap_position();
    for _ in 0..8 {
        let next = steering_neighbor_cap_position();
        assert_eq!(
            first, next,
            "steering with more than the neighbor cap must produce deterministic movement"
        );
    }
}

#[test]
fn movement_rejects_tank_body_clipping_building_corner() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Depot, 10, 10);
    entities
        .spawn_building(1, EntityKind::Depot, bx, by, true)
        .expect("building spawn");
    let rect = building_rect_for_footprint(EntityKind::Depot, 10, 10).expect("depot rect");

    let corner_offset = (tank_body_half_len() + 0.5) / 2.0_f32.sqrt();
    let start = (rect.max_x + corner_offset, rect.min_y - corner_offset);
    let goal = (rect.max_x, rect.min_y);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank spawn");
    let desired = (goal.1 - start.1).atan2(goal.0 - start.0);
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(desired);
    }
    set_path_direct(&mut entities, tank, vec![goal]);

    let occ = Occupancy::build(&map, &entities);
    assert!(standability::unit_static_standable_with_facing(
        &map,
        &occ,
        EntityKind::Tank,
        start.0,
        start.1,
        desired
    ));
    let center_tile = map.tile_of(
        rect.max_x + corner_offset - 1.0,
        rect.min_y - corner_offset + 1.0,
    );
    assert!(
        map.is_passable(center_tile.0 as i32, center_tile.1 as i32)
            && occ.passable(center_tile.0 as i32, center_tile.1 as i32),
        "candidate center tile should remain passable so the body check is the blocker"
    );

    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(tank).expect("tank should exist");
    assert!(
        moved_distance(start, (e.pos_x, e.pos_y)) <= 0.01,
        "tank body must not step into the building corner"
    );
}

#[test]
fn wall_slide_uses_unit_body_clearance() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Depot, 10, 10);
    entities
        .spawn_building(1, EntityKind::Depot, bx, by, true)
        .expect("building spawn");
    let rect = building_rect_for_footprint(EntityKind::Depot, 10, 10).expect("depot rect");

    let future_extent_x = tank_body_half_len() * TANK_BODY_TURN_RATE_RAD_PER_TICK.cos()
        + tank_body_half_width() * TANK_BODY_TURN_RATE_RAD_PER_TICK.sin();
    let future_extent_y = tank_body_half_len() * TANK_BODY_TURN_RATE_RAD_PER_TICK.sin()
        + tank_body_half_width() * TANK_BODY_TURN_RATE_RAD_PER_TICK.cos();
    let start = (
        rect.min_x - future_extent_x - 0.2,
        rect.min_y - future_extent_y + 0.2,
    );
    let goal = (start.0 + 64.0, start.1 + 6.0);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, tank, vec![goal]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(tank).expect("tank should exist");
    assert!(
        (e.pos_x - start.0).abs() <= 0.01,
        "x-only slide would clip the building body and must be rejected"
    );
    assert!(
        e.pos_y > start.1,
        "body-legal y-only slide should still make progress"
    );
    assert!(tank_standable_at_entity_facing(&map, &occ, &entities, tank));
}

#[test]
fn collision_push_does_not_move_tank_body_into_building() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Depot, 10, 10);
    entities
        .spawn_building(1, EntityKind::Depot, bx, by, true)
        .expect("building spawn");
    let rect = building_rect_for_footprint(EntityKind::Depot, 10, 10).expect("depot rect");

    let tank_start = (rect.max_x + tank_body_half_len() + 0.1, rect.min_y + 32.0);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, tank_start.0, tank_start.1)
        .expect("tank spawn");
    let rifleman = entities
        .spawn_unit(2, EntityKind::Rifleman, tank_start.0 + 8.0, tank_start.1)
        .expect("rifleman spawn");
    mark_moving(&mut entities, rifleman, (tank_start.0 + 64.0, tank_start.1));

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    resolve_collisions(&mut entities, &spatial, &map, &occ);

    let tank_after = pos(&entities, tank);
    assert!(
        (tank_after.0 - tank_start.0).abs() <= 0.01 && (tank_after.1 - tank_start.1).abs() <= 0.01,
        "blocked collision push must not move tank into the building body"
    );
    assert!(tank_standable_at_entity_facing(&map, &occ, &entities, tank));
    assert!(
        moved_distance((tank_start.0 + 8.0, tank_start.1), pos(&entities, rifleman)) > 0.01,
        "the legal side should absorb the collision push"
    );
    assert!(
        body_overlap_depth(&entities, tank, rifleman) <= COLLISION_EPS_PX,
        "tank and rifleman body overlap should be resolved by the legal side"
    );
}

#[test]
fn collision_push_does_not_move_tank_body_into_wall() {
    let mut map = flat_map(1);
    let row = 20u32;
    for ty in [row - 1, row + 1] {
        for tx in 10..30u32 {
            let idx = map.index(tx, ty);
            map.terrain[idx] = crate::protocol::terrain::ROCK;
        }
    }

    let mut entities = EntityStore::new();
    let (tx, ty) = map.tile_center(20, row);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, tx, ty)
        .expect("tank spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(0.0);
    }
    let rifleman = entities
        .spawn_unit(2, EntityKind::Rifleman, tx, ty + 6.0)
        .expect("rifleman spawn");
    mark_moving(&mut entities, rifleman, (tx, ty + 32.0));
    let tank_before = pos(&entities, tank);

    let occ = Occupancy::build(&map, &entities);
    assert!(tank_standable_at_entity_facing(&map, &occ, &entities, tank));
    let spatial = SpatialIndex::build(&entities, map.size);
    resolve_collisions(&mut entities, &spatial, &map, &occ);

    assert!(
        tank_standable_at_entity_facing(&map, &occ, &entities, tank),
        "collision must not push tank hull into the corridor wall"
    );
    assert!(
        (pos(&entities, tank).1 - tank_before.1).abs() <= 0.01,
        "blocked side push should not slide the tank through the wall"
    );
}

#[test]
fn tank_body_locomotion_suppresses_illegal_rotation_when_blocked() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Depot, 10, 10);
    entities
        .spawn_building(1, EntityKind::Depot, bx, by, true)
        .expect("building spawn");
    let rect = building_rect_for_footprint(EntityKind::Depot, 10, 10).expect("depot rect");

    let start = (rect.max_x + tank_body_half_width() + 0.1, rect.min_y + 32.0);
    let goal = (rect.min_x, rect.min_y + 32.0);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank spawn");
    let initial_facing = std::f32::consts::FRAC_PI_2;
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(initial_facing);
    }
    set_path_direct(&mut entities, tank, vec![goal]);

    let occ = Occupancy::build(&map, &entities);
    assert!(tank_standable_at_entity_facing(&map, &occ, &entities, tank));
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(tank).expect("tank should exist");
    assert!(
        moved_distance(start, (e.pos_x, e.pos_y)) <= 0.01,
        "blocked tank must not take an illegal body step"
    );
    assert!(
        (e.facing() - initial_facing).abs() <= 0.001,
        "tank should not rotate its hull into a building footprint while blocked"
    );
    assert!(tank_standable_at_entity_facing(&map, &occ, &entities, tank));
}

#[test]
fn tank_body_locomotion_backs_out_when_rotation_corner_is_blocked() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Depot, 10, 10);
    entities
        .spawn_building(1, EntityKind::Depot, bx, by, true)
        .expect("building spawn");
    let rect = building_rect_for_footprint(EntityKind::Depot, 10, 10).expect("depot rect");

    let initial_facing = 0.0;
    let next_facing = TANK_BODY_TURN_RATE_RAD_PER_TICK;
    let speed = config::unit_stats(EntityKind::Tank)
        .expect("tank stats")
        .speed;
    let occ = Occupancy::build(&map, &entities);
    let mut start = None;
    for ix in 0..80 {
        for iy in 0..80 {
            let x = rect.min_x - tank_body_half_len() - 4.0 + ix as f32 * 0.25;
            let y = rect.min_y - tank_body_half_width() - 4.0 + iy as f32 * 0.25;
            let original_legal = standability::unit_static_standable_with_facing(
                &map,
                &occ,
                EntityKind::Tank,
                x,
                y,
                initial_facing,
            );
            let rotated_legal = standability::unit_static_standable_with_facing(
                &map,
                &occ,
                EntityKind::Tank,
                x,
                y,
                next_facing,
            );
            let reverse_rotated_legal = standability::unit_static_standable_with_facing(
                &map,
                &occ,
                EntityKind::Tank,
                x - speed,
                y,
                next_facing,
            );
            let forward_rotated_legal = standability::unit_static_standable_with_facing(
                &map,
                &occ,
                EntityKind::Tank,
                x + speed,
                y,
                next_facing,
            );
            if original_legal && !rotated_legal && reverse_rotated_legal && !forward_rotated_legal {
                start = Some((x, y));
                break;
            }
        }
        if start.is_some() {
            break;
        }
    }
    let start = start.expect("fixture should find a front-corner rotation blockage");
    let goal = (
        start.0 + config::TILE_SIZE as f32 * 4.0,
        start.1 + config::TILE_SIZE as f32 * 4.0,
    );
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(initial_facing);
    }
    set_path_direct(&mut entities, tank, vec![goal]);

    assert!(standability::unit_static_standable_with_facing(
        &map,
        &occ,
        EntityKind::Tank,
        start.0,
        start.1,
        initial_facing
    ));
    assert!(
        !standability::unit_static_standable_with_facing(
            &map,
            &occ,
            EntityKind::Tank,
            start.0,
            start.1,
            next_facing
        ),
        "fixture requires the first tank rotation to clip the building corner"
    );

    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(tank).expect("tank should exist");
    assert!(
        e.pos_x < start.0 - 1.0,
        "front-corner rotation blockage should make the tank back out along its hull, start x {:.2}, got {:.2}",
        start.0,
        e.pos_x
    );
    assert!(
        e.facing() > initial_facing && e.facing() <= next_facing + 0.0001,
        "tank should keep the legal unjammed rotation, facing {:.4}",
        e.facing()
    );
    assert!(tank_standable_at_entity_facing(&map, &occ, &entities, tank));
}

#[test]
fn scout_car_locomotion_suppresses_illegal_rotation_when_blocked() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Factory, 22, 9);
    entities
        .spawn_building(1, EntityKind::Factory, bx, by, true)
        .expect("factory spawn");

    // Regression for a live crash: the scout car center is below the factory and legal while
    // nearly horizontal, but rotation can still swing the authoritative body into the factory
    // footprint without moving.
    let start = (710.0, 396.3);
    let initial_facing = 0.05;
    let illegal_next_facing = initial_facing + TANK_BODY_TURN_RATE_RAD_PER_TICK;
    assert!(standability::unit_static_standable_with_facing(
        &map,
        &Occupancy::build(&map, &entities),
        EntityKind::ScoutCar,
        start.0,
        start.1,
        initial_facing
    ));
    assert!(!standability::unit_static_standable_with_facing(
        &map,
        &Occupancy::build(&map, &entities),
        EntityKind::ScoutCar,
        start.0,
        start.1,
        illegal_next_facing
    ));

    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, start.0, start.1)
        .expect("scout car spawn");
    let goal = (start.0 + 128.0, start.1 + 20.0);
    if let Some(e) = entities.get_mut(scout) {
        e.set_facing(initial_facing);
        e.set_order(Order::move_to(goal.0, goal.1));
    }
    set_path_direct(&mut entities, scout, vec![goal]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(scout).expect("scout car should exist");
    let moved = moved_distance(start, (e.pos_x, e.pos_y));
    assert!(
        moved > 0.01 || (e.facing() - initial_facing).abs() <= 0.001,
        "scout car may rotate only when translation keeps the capsule legal"
    );
    assert!(
        matches!(e.order(), Order::Move(_)) && !e.path_is_empty(),
        "front-blocked scout car should preserve the player's move order for recovery"
    );
    let occ = Occupancy::build(&map, &entities);
    assert!(standability::unit_static_standable_with_facing(
        &map,
        &occ,
        EntityKind::ScoutCar,
        e.pos_x,
        e.pos_y,
        e.facing()
    ));
}

fn front_blocked_scout_car_fixture() -> (Map, EntityStore, u32, (f32, f32), (f32, f32), f32) {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Factory, 22, 9);
    entities
        .spawn_building(1, EntityKind::Factory, bx, by, true)
        .expect("factory spawn");
    let rect = building_rect_for_footprint(EntityKind::Factory, 22, 9).expect("factory rect");

    let start = (
        rect.max_x + config::SCOUT_CAR_BODY_LENGTH_PX * 0.5 + 1.5,
        (rect.min_y + rect.max_y) * 0.5,
    );
    let initial_facing = std::f32::consts::PI;
    let goal = (
        rect.min_x - config::TILE_SIZE as f32 * 2.0,
        rect.max_y + config::TILE_SIZE as f32 * 3.0,
    );
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, start.0, start.1)
        .expect("scout car spawn");
    if let Some(e) = entities.get_mut(scout) {
        e.set_facing(initial_facing);
        e.set_order(Order::move_to(goal.0, goal.1));
        e.reset_stuck(start.0, start.1);
    }
    set_path_direct(&mut entities, scout, vec![goal]);

    (map, entities, scout, start, goal, initial_facing)
}

fn run_scout_car_movement_tick(map: &Map, entities: &mut EntityStore, tick: u32) {
    let occ = Occupancy::build(map, entities);
    let spatial = SpatialIndex::build(entities, map.size);
    movement_system(map, entities, &mut [], &occ, &spatial, tick);
}

#[test]
fn scout_car_former_front_blocked_fixture_keeps_original_goal() {
    let (map, mut entities, scout, start, goal, _initial_facing) =
        front_blocked_scout_car_fixture();

    for tick in 0..config::SCOUT_CAR_STUCK_RECOVERY_TRIGGER_TICKS as u32 {
        run_scout_car_movement_tick(&map, &mut entities, tick);
    }

    let e = entities.get(scout).expect("scout car should exist");
    assert_eq!(
        e.next_waypoint(),
        Some(goal),
        "capsule geometry should keep the original goal in the former blocked-nose fixture"
    );
    assert!(
        e.movement
            .as_ref()
            .and_then(|m| m.scout_car_reverse_waypoint)
            .is_none(),
        "capsule geometry should not inject reverse recovery for the former blocked-nose fixture"
    );

    for tick in config::SCOUT_CAR_STUCK_RECOVERY_TRIGGER_TICKS as u32
        ..config::SCOUT_CAR_STUCK_RECOVERY_TRIGGER_TICKS as u32 + 20
    {
        run_scout_car_movement_tick(&map, &mut entities, tick);
    }

    let e = entities.get(scout).expect("scout car should exist");
    assert!(
        moved_distance(start, (e.pos_x, e.pos_y)) > 4.0,
        "scout car should make direct progress without reverse recovery, start {:?}, got ({:.2}, {:.2})",
        start,
        e.pos_x,
        e.pos_y
    );
    let occ = Occupancy::build(&map, &entities);
    assert!(
        standability::unit_static_standable_with_facing(
            &map,
            &occ,
            EntityKind::ScoutCar,
            e.pos_x,
            e.pos_y,
            e.facing()
        ),
        "direct progress should keep the scout car body legal"
    );
}

#[test]
fn scout_car_traffic_throttle_can_trigger_reverse_recovery() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let goal = (sx + config::TILE_SIZE as f32 * 8.0, sy);
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, sx, sy)
        .expect("scout car spawn");
    let blocker = entities
        .spawn_unit(2, EntityKind::Tank, sx + 1.0, sy)
        .expect("traffic blocker spawn");
    if let Some(e) = entities.get_mut(scout) {
        e.set_facing(0.0);
        e.set_order(Order::move_to(goal.0, goal.1));
        e.reset_stuck(sx, sy);
    }
    if let Some(e) = entities.get_mut(blocker) {
        e.set_facing(0.0);
        e.pos_x = sx + 0.0001;
    }
    set_path_direct(&mut entities, scout, vec![goal]);
    if let Some(e) = entities.get_mut(scout) {
        if let Some(m) = e.movement.as_mut() {
            m.stuck_ticks = config::SCOUT_CAR_STUCK_RECOVERY_TRIGGER_TICKS - 1;
            m.last_progress_pos = (sx, sy);
        }
    }

    run_scout_car_movement_tick(&map, &mut entities, 0);

    let e = entities.get(scout).expect("scout car should exist");
    assert_eq!(
        e.movement.as_ref().map(|m| m.path.len()),
        Some(2),
        "slow frontal traffic should eventually inject a reverse recovery waypoint"
    );
    assert!(
        e.next_waypoint().is_some_and(|wp| wp.0 < sx),
        "traffic recovery waypoint should be behind the scout car"
    );
}

#[test]
fn scout_car_recovery_cooldown_bounds_duplicate_waypoints() {
    let (map, mut entities, scout, _, _, _) = front_blocked_scout_car_fixture();
    let mut max_path_len = 0usize;

    for tick in 0..config::SCOUT_CAR_RECOVERY_COOLDOWN_TICKS as u32 {
        run_scout_car_movement_tick(&map, &mut entities, tick);
        let path_len = entities
            .get(scout)
            .and_then(|e| e.movement.as_ref())
            .map(|m| m.path.len())
            .unwrap_or(0);
        max_path_len = max_path_len.max(path_len);
    }

    assert!(
        max_path_len <= 2,
        "recovery should not stack duplicate waypoints; max path len was {max_path_len}"
    );
}

#[test]
fn scout_car_resumes_original_path_after_reverse_recovery() {
    let (map, mut entities, scout, start, goal, _) = front_blocked_scout_car_fixture();

    for tick in 0..180 {
        run_scout_car_movement_tick(&map, &mut entities, tick);
        if entities
            .get(scout)
            .is_some_and(|e| e.path_is_empty() && matches!(e.order(), Order::Idle))
        {
            break;
        }
    }

    let e = entities.get(scout).expect("scout car should exist");
    assert!(
        moved_distance((e.pos_x, e.pos_y), goal) < moved_distance(start, goal),
        "after reversing out, scout car should resume toward the original goal; pos=({:.2},{:.2})",
        e.pos_x,
        e.pos_y
    );
    assert!(
        e.path_is_empty() || e.path_goal() == Some(goal),
        "recovery should preserve the original path goal"
    );
}

#[test]
fn scout_car_reverse_recovery_is_deterministic() {
    fn run_once() -> ((f32, f32), f32, Vec<(f32, f32)>) {
        let (map, mut entities, scout, _, _, _) = front_blocked_scout_car_fixture();
        for tick in 0..90 {
            run_scout_car_movement_tick(&map, &mut entities, tick);
        }
        let e = entities.get(scout).expect("scout car should exist");
        (
            (e.pos_x, e.pos_y),
            e.facing(),
            e.movement
                .as_ref()
                .map(|m| m.path.clone())
                .unwrap_or_default(),
        )
    }

    let a = run_once();
    let b = run_once();
    assert_eq!(
        a, b,
        "same recovery fixture should produce identical path state"
    );
}

#[test]
fn scout_car_consumes_lateral_intermediate_waypoint_inside_car_radius() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let intermediate = (sx, sy + config::VEHICLE_WAYPOINT_ACCEPTANCE_RADIUS_PX - 2.0);
    let goal = (sx + config::TILE_SIZE as f32 * 4.0, intermediate.1);
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, sx, sy)
        .expect("scout car should spawn");
    if let Some(e) = entities.get_mut(scout) {
        e.set_facing(0.0);
        e.set_order(Order::move_to(goal.0, goal.1));
    }
    set_path_direct(&mut entities, scout, vec![intermediate, goal]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(scout).expect("scout car should exist");
    assert_eq!(
        e.movement.as_ref().map(|m| m.path.len()),
        Some(1),
        "lateral intermediate waypoint inside scout-car acceptance radius should be consumed"
    );
    assert_eq!(e.next_waypoint(), Some(goal));
    assert!(
        e.pos_x > sx,
        "after consuming the lateral waypoint, scout car should continue down the route corridor"
    );
}

#[test]
fn scout_car_lateral_final_goal_settles_inside_final_tolerance() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let goal = (sx, sy + config::SCOUT_CAR_FINAL_GOAL_TOLERANCE_PX - 2.0);
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, sx, sy)
        .expect("scout car should spawn");
    if let Some(e) = entities.get_mut(scout) {
        e.set_facing(0.0);
        e.set_order(Order::move_to(goal.0, goal.1));
    }
    set_path_direct(&mut entities, scout, vec![goal]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(scout).expect("scout car should exist");
    assert!(
        e.path_is_empty() && matches!(e.order(), Order::Idle),
        "lateral final goal inside scout-car tolerance should settle without oscillation"
    );
    assert!(
        moved_distance((e.pos_x, e.pos_y), goal) <= config::SCOUT_CAR_FINAL_GOAL_TOLERANCE_PX,
        "settled scout car should remain inside final tolerance"
    );
}

#[test]
fn scout_car_attack_move_lateral_final_goal_keeps_attack_move_order() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let goal = (sx, sy + config::SCOUT_CAR_FINAL_GOAL_TOLERANCE_PX - 2.0);
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, sx, sy)
        .expect("scout car should spawn");
    if let Some(e) = entities.get_mut(scout) {
        e.set_facing(0.0);
        e.set_order(Order::attack_move_to(goal.0, goal.1));
    }
    set_path_direct(&mut entities, scout, vec![goal]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(scout).expect("scout car should exist");
    assert!(e.path_is_empty(), "attack-move scout car should settle");
    assert_eq!(e.move_phase(), Some(MovePhase::Arrived));
    assert!(
        matches!(e.order(), Order::AttackMove(_)),
        "attack-move arrival should keep combat movement semantics"
    );
}

#[test]
fn scout_car_turns_by_curvature_while_moving() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let (_, gy) = map.tile_center(20, 26);
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, sx, sy)
        .expect("scout car should spawn");
    if let Some(e) = entities.get_mut(scout) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, scout, vec![(sx, gy)]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(scout).expect("scout car should exist");
    let moved = moved_distance((sx, sy), (e.pos_x, e.pos_y));
    let max_turn = config::unit_stats(EntityKind::ScoutCar)
        .expect("scout stats")
        .speed
        / SCOUT_CAR_MIN_TURN_RADIUS_PX;
    assert!(moved > 0.01, "scout car should turn while translating");
    assert!(
        e.facing() > 0.0 && e.facing() <= max_turn + 0.0001,
        "scout car yaw should be capped by movement curvature, got {:.4}",
        e.facing()
    );
    assert!(
        e.pos_x > sx,
        "scout car should advance along its facing instead of sliding straight north"
    );
}

#[test]
fn scout_car_does_not_pivot_in_place_for_far_goal_behind() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let goal = (
        sx - VEHICLE_REVERSE_GOAL_DISTANCE_PX - config::TILE_SIZE as f32,
        sy,
    );
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, sx, sy)
        .expect("scout car should spawn");
    if let Some(e) = entities.get_mut(scout) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, scout, vec![goal]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(scout).expect("scout car should exist");
    let moved = moved_distance((sx, sy), (e.pos_x, e.pos_y));
    assert!(
        moved > 0.01,
        "far behind goal should make the scout car drive through a turn, not pivot"
    );
    assert!(
        e.pos_x > sx,
        "far behind goal should start as a forward turning arc instead of backing up; start x {:.2}, got {:.2}",
        sx,
        e.pos_x
    );
    assert!(
        e.movement
            .as_ref()
            .is_some_and(|m| m.scout_car_reverse_waypoint.is_none()),
        "far behind goal must not latch reverse from the route lookahead point"
    );
    assert!(
        e.facing().abs() > 0.0,
        "scout car should still steer while it moves"
    );
}

#[test]
fn scout_car_symmetric_far_behind_turn_is_deterministic() {
    fn run_once() -> ((f32, f32), f32) {
        let map = flat_map(1);
        let mut entities = EntityStore::new();
        let (sx, sy) = map.tile_center(20, 20);
        let goal = (
            sx - VEHICLE_REVERSE_GOAL_DISTANCE_PX - config::TILE_SIZE as f32,
            sy,
        );
        let scout = entities
            .spawn_unit(1, EntityKind::ScoutCar, sx, sy)
            .expect("scout car should spawn");
        if let Some(e) = entities.get_mut(scout) {
            e.set_facing(0.0);
        }
        set_path_direct(&mut entities, scout, vec![goal]);

        run_scout_car_movement_tick(&map, &mut entities, 0);

        let e = entities.get(scout).expect("scout car should exist");
        ((e.pos_x, e.pos_y), e.facing())
    }

    let first = run_once();
    let second = run_once();
    assert_eq!(
        first, second,
        "symmetric scout-car primitive tie should resolve deterministically"
    );
}

#[test]
fn scout_car_far_behind_click_after_forward_motion_does_not_backtrack_first() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, sx, sy)
        .expect("scout car should spawn");
    if let Some(e) = entities.get_mut(scout) {
        e.set_facing(0.0);
        e.set_order(Order::move_to(sx + config::TILE_SIZE as f32 * 8.0, sy));
    }
    set_path_direct(
        &mut entities,
        scout,
        vec![(sx + config::TILE_SIZE as f32 * 8.0, sy)],
    );

    for tick in 0..20 {
        run_scout_car_movement_tick(&map, &mut entities, tick);
    }
    let before_click = pos(&entities, scout);
    let behind_goal = (
        before_click.0 - config::TILE_SIZE as f32 * 6.0,
        before_click.1,
    );
    if let Some(e) = entities.get_mut(scout) {
        e.set_order(Order::move_to(behind_goal.0, behind_goal.1));
        e.reset_stuck(before_click.0, before_click.1);
    }
    set_path_direct(&mut entities, scout, vec![behind_goal]);

    run_scout_car_movement_tick(&map, &mut entities, 21);

    let e = entities.get(scout).expect("scout car should exist");
    assert!(
        e.pos_x > before_click.0,
        "clicking far behind a moving scout car should begin with a forward turn, before x {:.2}, got {:.2}",
        before_click.0,
        e.pos_x
    );
    assert!(
        e.movement
            .as_ref()
            .is_some_and(|m| m.scout_car_reverse_waypoint.is_none()),
        "far-behind retarget should not enter reverse gear"
    );
}

#[test]
fn scout_car_close_reverse_keeps_waypoint_latched_until_arrival() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let goal = (sx - config::TILE_SIZE as f32 * 2.0, sy + 20.0);
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, sx, sy)
        .expect("scout car should spawn");
    if let Some(e) = entities.get_mut(scout) {
        e.set_facing(0.0);
        e.set_order(Order::move_to(goal.0, goal.1));
    }
    set_path_direct(&mut entities, scout, vec![goal]);

    let mut saw_reverse = false;
    for tick in 0..120 {
        run_scout_car_movement_tick(&map, &mut entities, tick);
        let e = entities.get(scout).expect("scout car should exist");
        if e.path_is_empty() {
            break;
        }
        let reverse_waypoint = e
            .movement
            .as_ref()
            .and_then(|m| m.scout_car_reverse_waypoint);
        assert_eq!(
            reverse_waypoint,
            e.next_waypoint(),
            "close reverse should stay latched to the immediate waypoint until it is consumed"
        );
        saw_reverse = true;
    }

    let e = entities.get(scout).expect("scout car should exist");
    assert!(saw_reverse, "fixture should exercise reverse movement");
    assert!(
        e.path_is_empty() && matches!(e.order(), Order::Idle),
        "latched reverse should still arrive cleanly; pos=({:.2},{:.2}) goal=({:.2},{:.2}) facing={:.3}",
        e.pos_x,
        e.pos_y,
        goal.0,
        goal.1,
        e.facing()
    );
}

#[test]
fn scout_car_reversing_to_nearby_offset_goal_arrives() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let goal = (sx - config::TILE_SIZE as f32 * 2.0, sy + 20.0);
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, sx, sy)
        .expect("scout car should spawn");
    if let Some(e) = entities.get_mut(scout) {
        e.set_facing(0.0);
        e.set_order(Order::move_to(goal.0, goal.1));
    }
    set_path_direct(&mut entities, scout, vec![goal]);

    for tick in 0..120 {
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
        if entities
            .get(scout)
            .is_some_and(|e| e.path_is_empty() && matches!(e.order(), Order::Idle))
        {
            break;
        }
    }

    let e = entities.get(scout).expect("scout car should exist");
    assert!(
        e.path_is_empty() && matches!(e.order(), Order::Idle),
        "reverse steering should settle at the final waypoint instead of jiggling near it; pos=({:.2},{:.2}) goal=({:.2},{:.2}) facing={:.3}",
        e.pos_x,
        e.pos_y,
        goal.0,
        goal.1,
        e.facing()
    );
    assert!(
        moved_distance((e.pos_x, e.pos_y), goal) <= config::VEHICLE_WAYPOINT_ACCEPTANCE_RADIUS_PX,
        "straight reverse should settle within scout-car acceptance without reverse turning"
    );
}

#[test]
fn tank_body_facing_turns_gradually_along_path() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let (_, gy) = map.tile_center(20, 26);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("tank should spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, tank, vec![(sx, gy)]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let facing = entities.get(tank).expect("tank should exist").facing();
    assert!(
        facing > 0.0 && facing <= TANK_BODY_TURN_RATE_RAD_PER_TICK + 0.0001,
        "tank body should turn by at most the turn-rate constant, got {facing:.4}"
    );
}

#[test]
fn anti_tank_gun_body_uses_pivot_drive_turning_along_path() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let (_, gy) = map.tile_center(20, 26);
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, sx, sy)
        .expect("anti-tank gun should spawn");
    if let Some(e) = entities.get_mut(anti_tank_gun) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, anti_tank_gun, vec![(sx, gy)]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities
        .get(anti_tank_gun)
        .expect("anti-tank gun should exist");
    assert!(
        e.facing() > 0.0 && e.facing() <= ANTI_TANK_GUN_BODY_TURN_RATE_RAD_PER_TICK + 0.0001,
        "anti-tank gun body should turn by at most the support-weapon turn-rate constant, got {:.4}",
        e.facing()
    );
    assert!(
        moved_distance((sx, sy), (e.pos_x, e.pos_y)) < 0.01,
        "anti-tank gun should pivot before driving when the target is far off its facing"
    );
}

#[test]
fn mixed_tank_infantry_group_movement_stays_body_legal() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(12, 20);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("tank spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(0.0);
    }
    let rifle_a = entities
        .spawn_unit(1, EntityKind::Rifleman, sx + 38.0, sy - 13.0)
        .expect("rifle a spawn");
    let rifle_b = entities
        .spawn_unit(1, EntityKind::Rifleman, sx + 38.0, sy + 13.0)
        .expect("rifle b spawn");
    let ids = [tank, rifle_a, rifle_b];
    let goal = (sx + config::TILE_SIZE as f32 * 8.0, sy);
    let mut pathing = PathingService::new(8_192, 256);

    for tick in 1u32..=180 {
        pathing.advance_tick(tick);
        let occ = Occupancy::build(&map, &entities);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, tick);
        if tick == 1 {
            coordinator.order_group_move(&mut entities, 1, &ids, goal, false);
        }
        coordinator.process_awaiting_paths(&mut entities);
        let spatial = SpatialIndex::build(&entities, map.size);
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
        let spatial = SpatialIndex::build(&entities, map.size);
        resolve_collisions(&mut entities, &spatial, &map, &occ);

        let occ_after = Occupancy::build(&map, &entities);
        assert!(
            tank_standable_at_entity_facing(&map, &occ_after, &entities, tank),
            "mixed group traffic must keep tank body static-legal at tick {tick}"
        );
    }

    assert!(
        pos(&entities, tank).0 > sx + config::TILE_SIZE as f32 * 2.0,
        "tank should still make progress in a mixed group"
    );
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            assert!(
                body_overlap_depth(&entities, ids[i], ids[j]) <= 4.0,
                "mixed group units should not remain deeply overlapped"
            );
        }
    }
}

#[test]
fn tank_pauses_when_body_badly_misaligned() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let (gx, _) = map.tile_center(14, 20);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("tank should spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, tank, vec![(gx, sy)]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(tank).expect("tank should exist");
    let moved = moved_distance((sx, sy), (e.pos_x, e.pos_y));
    assert!(
        moved <= 0.01,
        "badly misaligned tank should pivot in place, moved {moved:.4}px"
    );
    assert!(
        e.facing().abs() > 0.0 && e.facing().abs() <= TANK_BODY_TURN_RATE_RAD_PER_TICK + 0.0001,
        "tank should still rotate while paused, facing {:.4}",
        e.facing()
    );
}

#[test]
fn tank_reverses_to_nearby_goal_behind() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let goal = (sx - config::TILE_SIZE as f32, sy);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("tank should spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, tank, vec![goal]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(tank).expect("tank should exist");
    assert!(
        e.pos_x < sx,
        "near behind goal should make the tank reverse, start x {sx:.2}, got {:.2}",
        e.pos_x
    );
    assert!(
        angle_delta(0.0, e.facing()).abs() <= 0.001,
        "directly reversing should not spin the hull, facing {:.4}",
        e.facing()
    );
}

#[test]
fn tank_still_pivots_for_far_goal_behind() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let goal = (
        sx - VEHICLE_REVERSE_GOAL_DISTANCE_PX - config::TILE_SIZE as f32,
        sy,
    );
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("tank should spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, tank, vec![goal]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(tank).expect("tank should exist");
    let moved = moved_distance((sx, sy), (e.pos_x, e.pos_y));
    assert!(
        moved <= 0.01,
        "far behind goal should pivot before driving, moved {moved:.4}px"
    );
    assert!(
        e.facing().abs() > 0.0 && e.facing().abs() <= TANK_BODY_TURN_RATE_RAD_PER_TICK + 0.0001,
        "far behind goal should rotate hull toward the forward route, facing {:.4}",
        e.facing()
    );
}

#[test]
fn tank_does_not_reverse_for_behind_intermediate_waypoint() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let behind = (sx - config::TILE_SIZE as f32, sy);
    let goal = (sx + config::TILE_SIZE as f32 * 8.0, sy);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("tank should spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, tank, vec![behind, goal]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(tank).expect("tank should exist");
    let moved = moved_distance((sx, sy), (e.pos_x, e.pos_y));
    assert!(
        moved <= 0.01,
        "behind intermediate waypoint should make the tank pivot before moving, moved {moved:.4}px"
    );
    assert!(
        e.facing().abs() > 0.0 && e.facing().abs() <= TANK_BODY_TURN_RATE_RAD_PER_TICK + 0.0001,
        "tank should rotate toward the behind waypoint instead of staying in reverse-facing gear, facing {:.4}",
        e.facing()
    );
}

#[test]
fn tank_reverse_correction_uses_short_angle_across_wrap() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let goal = (sx + config::TILE_SIZE as f32, sy + 0.5);
    let initial_facing = std::f32::consts::PI - 0.01;
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("tank should spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(initial_facing);
    }
    set_path_direct(&mut entities, tank, vec![goal]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let e = entities.get(tank).expect("tank should exist");
    let hull_delta = angle_delta(initial_facing, e.facing()).abs();
    assert!(
        hull_delta <= TANK_BODY_TURN_RATE_RAD_PER_TICK + 0.0001,
        "reverse correction should use the short wrapped turn, delta {hull_delta:.4}, facing {:.4}",
        e.facing()
    );
    assert!(
        e.pos_x > sx,
        "near behind wrapped goal should reverse toward positive x, got {:.2} from start {sx:.2}",
        e.pos_x
    );
}

#[test]
fn tank_facing_remains_finite_after_movement() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, sx, sy)
        .expect("tank should spawn");
    if let Some(e) = entities.get_mut(tank) {
        e.set_facing(f32::NAN);
    }
    set_path_direct(&mut entities, tank, vec![(sx + 200.0, sy)]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let facing = entities.get(tank).expect("tank should exist").facing();
    assert!(
        facing.is_finite(),
        "tank movement should recover a finite hull facing, got {facing:?}"
    );
}

#[test]
fn rifleman_facing_remains_instant_for_path_segment() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let (_, gy) = map.tile_center(20, 26);
    let rifleman = entities
        .spawn_unit(1, EntityKind::Rifleman, sx, sy)
        .expect("rifleman should spawn");
    if let Some(e) = entities.get_mut(rifleman) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, rifleman, vec![(sx, gy)]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let facing = entities
        .get(rifleman)
        .expect("rifleman should exist")
        .facing();
    assert!(
        (facing - std::f32::consts::FRAC_PI_2).abs() <= 0.0001,
        "rifleman should snap to path-segment facing, got {facing:.4}"
    );
}

#[test]
fn anti_tank_gun_facing_turns_gradually_along_path() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let (_, gy) = map.tile_center(20, 26);
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, sx, sy)
        .expect("anti-tank gun should spawn");
    if let Some(e) = entities.get_mut(anti_tank_gun) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, anti_tank_gun, vec![(sx, gy)]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let facing = entities
        .get(anti_tank_gun)
        .expect("anti-tank gun should exist")
        .facing();
    assert!(
        facing > 0.0 && facing <= ANTI_TANK_GUN_BODY_TURN_RATE_RAD_PER_TICK + 0.0001,
        "anti-tank gun should turn by at most the turn-rate constant, got {facing:.4}"
    );
}

#[test]
fn mortar_team_facing_snaps_along_path() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let (_, gy) = map.tile_center(20, 26);
    let mortar = entities
        .spawn_unit(1, EntityKind::MortarTeam, sx, sy)
        .expect("mortar should spawn");
    if let Some(e) = entities.get_mut(mortar) {
        e.set_facing(0.0);
    }
    set_path_direct(&mut entities, mortar, vec![(sx, gy)]);

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

    let facing = entities.get(mortar).expect("mortar should exist").facing();
    assert!(
        (facing - std::f32::consts::FRAC_PI_2).abs() <= 0.0001,
        "mortar should snap to its movement direction, got {facing:.4}"
    );
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
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

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
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
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
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
        if entities.get(unit).is_none_or(|e| e.path_is_empty()) {
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
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

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
    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);

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
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
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
        &mut [],
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
fn unit_pressed_against_building_wall_reaches_goal() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();

    // Depot (2×2): center (352, 288) → footprint tiles (10,8),(11,8),(10,9),(11,9).
    // West wall at x=320, north wall at y=256.
    entities
        .spawn_building(1, EntityKind::Depot, 352.0, 288.0, true)
        .expect("building spawn");

    // Unit pressed against the building's west wall at body clearance: the rifleman radius
    // is 9 px, so x=310.5 leaves a narrow legal gap before the west wall at x=320.
    let unit = entities
        .spawn_unit(1, EntityKind::Rifleman, 310.5, 272.0)
        .expect("unit spawn");

    // Path along the building's north side to a goal well past it.  The arrival-radius
    // logic pops (9,8) immediately (dist ≤ 16 px). The route then asks the unit to skim
    // the northwest corner before heading east. Goal (13,7) is far enough away that
    // tolerant arrival cannot hide a wall-slide regression.
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
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
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
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);
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

/// Regression for a tick-panic seen in live play: two infantry meeting head-on inside a
/// 1-tile-wide rock corridor with a slight lateral offset would lock at ~9.5 px (overlap
/// ~8.5 px, well past the 8 px invariant tolerance). The line-of-centers push was diagonal,
/// so both target positions clipped into the rock wall on either side of the corridor and
/// the resolver fell into the `(false, false)` branch and did nothing across all four passes.
/// The axis-aligned fallback must break the deadlock.
#[test]
fn head_on_in_one_tile_corridor_does_not_deadlock() {
    let mut map = flat_map(1);
    // Carve a horizontal 1-tile corridor at row 20 by filling rows 19 and 21 with rock.
    let row = 20u32;
    for ty in [row - 1, row + 1] {
        for tx in 10..30u32 {
            let idx = map.index(tx, ty);
            map.terrain[idx] = crate::protocol::terrain::ROCK;
        }
    }

    let mut entities = EntityStore::new();
    // Place the two units in adjacent corridor tiles, each offset laterally toward the
    // opposite wall so the connecting line has a real Y component. With radius 9 and a
    // 32 px tile, ±5 px of Y drift still leaves the bodies clear of the walls but makes
    // the diagonal push from line-of-centers clip into the walls.
    let (ax, ay) = map.tile_center(19, row);
    let (bx, by) = map.tile_center(20, row);
    let a = entities
        .spawn_unit(1, EntityKind::Worker, ax, ay - 5.0)
        .unwrap();
    let b = entities
        .spawn_unit(2, EntityKind::Rifleman, bx, by + 5.0)
        .unwrap();

    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.size);
    resolve_collisions(&mut entities, &spatial, &map, &occ);

    let ra = entities.get(a).unwrap().radius();
    let rb = entities.get(b).unwrap().radius();
    let d = dist(&entities, a, b);
    // The invariant tolerates 8 px of residue; require at least that much breathing room
    // so this case can't trip it.
    assert!(
        d + 8.0 >= ra + rb,
        "head-on corridor units must separate to within the invariant tolerance \
             (dist {:.2}, min {:.1})",
        d,
        ra + rb
    );
}
