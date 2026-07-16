use std::collections::BTreeSet;

use crate::config;
use crate::game::entity::{
    movement_body_class, uses_oriented_vehicle_body, Entity, EntityKind, EntityStore,
    MovementBodyClass,
};
use crate::game::map::Map;
use crate::game::services::geometry::{
    building_rect_for_entity, building_rect_for_footprint, circle_intersects_rect, rects_intersect,
    tile_rect, unit_bodies_intersect, unit_body, unit_body_for_entity, unit_body_intersects_rect,
    unit_body_with_facing, CircleBody, RectBody, UnitBody,
};
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;

mod placement_policy;
mod pump_jack;

use placement_policy::{build_placement_policy, BuildPlacementPolicy};

#[allow(dead_code)]
const BUILD_SITE_SPATIAL_PADDING_TILES: i32 = 8;

pub(crate) fn unit_static_standable(
    map: &Map,
    occ: &Occupancy,
    kind: EntityKind,
    x: f32,
    y: f32,
) -> bool {
    unit_static_standable_with_facing(map, occ, kind, x, y, 0.0)
}

pub(crate) fn unit_static_standable_with_facing(
    map: &Map,
    occ: &Occupancy,
    kind: EntityKind,
    x: f32,
    y: f32,
    facing: f32,
) -> bool {
    let Some(body) = unit_body_with_facing(kind, x, y, facing) else {
        return false;
    };
    if !unit_body_inside_world(map, body) {
        return false;
    }

    for (tx, ty) in body_tile_range(body) {
        if !map.in_bounds(tx, ty) {
            return false;
        }

        let tile = tile_rect(tx, ty);
        if !map.is_passable(tx, ty) && unit_body_intersects_rect(body, tile) {
            return false;
        }
        if !occ.passable_for_kind(tx, ty, kind) && unit_body_intersects_rect(body, tile) {
            return false;
        }
    }

    true
}

pub(crate) fn unit_static_segment_standable(
    map: &Map,
    occ: &Occupancy,
    kind: EntityKind,
    from: (f32, f32),
    to: (f32, f32),
) -> bool {
    let facing = segment_body_facing(kind, from, to);
    if !unit_static_standable_with_facing(map, occ, kind, from.0, from.1, facing)
        || !unit_static_standable_with_facing(map, occ, kind, to.0, to.1, facing)
    {
        return false;
    }

    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let distance = (dx * dx + dy * dy).sqrt();
    let step_px = config::TILE_SIZE as f32 / 4.0;
    let steps = (distance / step_px).ceil().max(1.0) as u32;

    for i in 1..steps {
        let t = i as f32 / steps as f32;
        let x = from.0 + dx * t;
        let y = from.1 + dy * t;
        if !unit_static_standable_with_facing(map, occ, kind, x, y, facing) {
            return false;
        }
    }

    true
}

pub(crate) fn unit_spawn_standable(
    map: &Map,
    occ: &Occupancy,
    entities: &EntityStore,
    kind: EntityKind,
    x: f32,
    y: f32,
) -> bool {
    let Some(body) = unit_body(kind, x, y) else {
        return false;
    };
    if !unit_static_standable(map, occ, kind, x, y) {
        return false;
    }

    entities.iter().all(|e| {
        e.hp == 0
            || !e.is_unit()
            || unit_body_for_entity(e).is_none_or(|existing| !unit_bodies_intersect(body, existing))
    })
}

#[allow(dead_code)]
pub(crate) fn building_site_clear(
    map: &Map,
    entities: &EntityStore,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    building_site_status_with_ignored_unit(map, entities, building, tile_x, tile_y, None).is_clear()
}

#[allow(dead_code)]
pub(crate) fn building_site_clear_spatial(
    map: &Map,
    entities: &EntityStore,
    spatial: &SpatialIndex,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    let Some(rect) = building_rect_for_footprint(building, tile_x, tile_y) else {
        return false;
    };
    if !footprint_in_bounds_and_passable(map, building, tile_x, tile_y) {
        return false;
    }
    let Some(stats) = config::building_stats(building) else {
        return false;
    };
    let min_tx = tile_x as i32 - BUILD_SITE_SPATIAL_PADDING_TILES;
    let min_ty = tile_y as i32 - BUILD_SITE_SPATIAL_PADDING_TILES;
    let max_tx = tile_x
        .saturating_add(stats.foot_w)
        .saturating_add(BUILD_SITE_SPATIAL_PADDING_TILES as u32) as i32;
    let max_ty = tile_y
        .saturating_add(stats.foot_h)
        .saturating_add(BUILD_SITE_SPATIAL_PADDING_TILES as u32) as i32;

    let policy = build_placement_policy(building);
    if !contextual_placement_status(map, entities, policy, rect).is_clear() {
        return false;
    }

    classify_entity_blockers(
        spatial
            .ids_in_rect(min_tx, min_ty, max_tx, max_ty)
            .filter_map(|id| entities.get(id)),
        map,
        rect,
        None,
        policy,
    )
    .is_clear()
}

pub(crate) fn resource_node_building_overlap_allowed(
    node: &Entity,
    building_kind: EntityKind,
    rect: RectBody,
) -> bool {
    build_placement_policy(building_kind) == BuildPlacementPolicy::PumpJackOilOnly
        && pump_jack::oil_node_center_in_rect(node, rect)
}

pub(crate) fn unit_intersects_building_footprint(
    entity: &Entity,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    let Some(rect) = building_rect_for_footprint(building, tile_x, tile_y) else {
        return false;
    };
    unit_body_for_entity(entity).is_some_and(|body| unit_body_intersects_rect(body, rect))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn unit_position_clear_of_building_footprint(
    map: &Map,
    occupancy: &Occupancy<'_>,
    entities: &EntityStore,
    ignored_units: &BTreeSet<u32>,
    kind: EntityKind,
    facing: f32,
    x: f32,
    y: f32,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    let Some(site_rect) = building_rect_for_footprint(building, tile_x, tile_y) else {
        return false;
    };
    let Some(candidate_body) = unit_body_with_facing(kind, x, y, facing) else {
        return false;
    };
    !unit_body_intersects_rect(candidate_body, site_rect)
        && unit_static_standable_with_facing(map, occupancy, kind, x, y, facing)
        && entities.iter().all(|entity| {
            entity.hp == 0
                || !entity.is_unit()
                || ignored_units.contains(&entity.id)
                || unit_body_for_entity(entity)
                    .is_none_or(|occupied| !unit_bodies_intersect(candidate_body, occupied))
        })
}

#[cfg(test)]
pub(crate) fn building_site_clear_for_build_intent(
    map: &Map,
    entities: &EntityStore,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
    builder_id: u32,
) -> bool {
    building_site_status_for_build_intent(map, entities, building, tile_x, tile_y, builder_id)
        .is_clear()
}

pub(crate) fn building_site_status_for_build_intent(
    map: &Map,
    entities: &EntityStore,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
    builder_id: u32,
) -> BuildSiteStatus {
    building_site_status_with_ignored_unit(
        map,
        entities,
        building,
        tile_x,
        tile_y,
        Some(builder_id),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BuildSiteStatus {
    Clear,
    InvalidFootprint,
    BlockedByBuilding,
    BlockedByResourceNode,
    BlockedByUnit,
}

impl BuildSiteStatus {
    fn is_clear(self) -> bool {
        self == Self::Clear
    }
}

fn building_site_status_with_ignored_unit(
    map: &Map,
    entities: &EntityStore,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
    ignored_unit: Option<u32>,
) -> BuildSiteStatus {
    let Some(rect) = building_rect_for_footprint(building, tile_x, tile_y) else {
        return BuildSiteStatus::InvalidFootprint;
    };
    if !footprint_in_bounds_and_passable(map, building, tile_x, tile_y) {
        return BuildSiteStatus::InvalidFootprint;
    }
    let policy = build_placement_policy(building);
    let contextual_status = contextual_placement_status(map, entities, policy, rect);
    if !contextual_status.is_clear() {
        return contextual_status;
    }

    classify_entity_blockers(entities.iter(), map, rect, ignored_unit, policy)
}

fn contextual_placement_status(
    map: &Map,
    entities: &EntityStore,
    policy: BuildPlacementPolicy,
    rect: RectBody,
) -> BuildSiteStatus {
    if policy != BuildPlacementPolicy::PumpJackOilOnly {
        return BuildSiteStatus::Clear;
    }
    let oil_ids = pump_jack::live_oil_node_centers_in_rect(entities.iter(), rect);
    if oil_ids.is_empty() {
        return BuildSiteStatus::InvalidFootprint;
    }
    if oil_ids
        .iter()
        .all(|oil_id| pump_jack::oil_node_has_pump_jack(map, entities, *oil_id))
    {
        return BuildSiteStatus::BlockedByBuilding;
    }
    BuildSiteStatus::Clear
}

fn classify_entity_blockers<'a>(
    entities: impl Iterator<Item = &'a Entity>,
    map: &Map,
    rect: RectBody,
    ignored_unit: Option<u32>,
    policy: BuildPlacementPolicy,
) -> BuildSiteStatus {
    let mut status = BuildSiteStatus::Clear;
    for entity in entities {
        status = combine_build_site_status(
            status,
            entity_build_site_status(map, entity, rect, ignored_unit, policy),
        );
        if status == BuildSiteStatus::BlockedByBuilding {
            break;
        }
    }
    status
}

fn combine_build_site_status(
    current: BuildSiteStatus,
    candidate: BuildSiteStatus,
) -> BuildSiteStatus {
    use BuildSiteStatus::*;

    match (current, candidate) {
        (InvalidFootprint, _) | (_, InvalidFootprint) => InvalidFootprint,
        (BlockedByBuilding, _) | (_, BlockedByBuilding) => BlockedByBuilding,
        (BlockedByResourceNode, _) | (_, BlockedByResourceNode) => BlockedByResourceNode,
        (BlockedByUnit, _) | (_, BlockedByUnit) => BlockedByUnit,
        (Clear, Clear) => Clear,
    }
}

fn entity_build_site_status(
    map: &Map,
    e: &Entity,
    rect: RectBody,
    ignored_unit: Option<u32>,
    policy: BuildPlacementPolicy,
) -> BuildSiteStatus {
    if e.hp == 0 {
        return BuildSiteStatus::Clear;
    }
    if e.is_unit() && ignored_unit == Some(e.id) {
        return BuildSiteStatus::Clear;
    }
    if e.is_building() {
        return match building_rect_for_entity(map, e) {
            Some(other) if rects_intersect(rect, other) => BuildSiteStatus::BlockedByBuilding,
            _ => BuildSiteStatus::Clear,
        };
    }
    if e.is_node() {
        if !circle_intersects_rect(entity_circle_body(e), rect) {
            return BuildSiteStatus::Clear;
        }
        if policy == BuildPlacementPolicy::PumpJackOilOnly && e.kind == EntityKind::Oil {
            return BuildSiteStatus::Clear;
        }
        return BuildSiteStatus::BlockedByResourceNode;
    }
    if e.is_unit() {
        if policy == BuildPlacementPolicy::VehicleBodyOnly
            && movement_body_class(e.kind) == MovementBodyClass::InfantryLike
        {
            return BuildSiteStatus::Clear;
        }
        return match unit_body_for_entity(e) {
            Some(body) if unit_body_intersects_rect(body, rect) => BuildSiteStatus::BlockedByUnit,
            _ => BuildSiteStatus::Clear,
        };
    }
    BuildSiteStatus::Clear
}

fn footprint_in_bounds_and_passable(
    map: &Map,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    let Some(stats) = config::building_stats(building) else {
        return false;
    };
    if stats.foot_w == 0 || stats.foot_h == 0 {
        return false;
    }
    let Some(max_x) = tile_x.checked_add(stats.foot_w) else {
        return false;
    };
    let Some(max_y) = tile_y.checked_add(stats.foot_h) else {
        return false;
    };
    if max_x > map.size || max_y > map.size {
        return false;
    }

    for ty in tile_y..max_y {
        for tx in tile_x..max_x {
            if !map.is_passable(tx as i32, ty as i32) {
                return false;
            }
        }
    }

    true
}

fn unit_body_inside_world(map: &Map, body: UnitBody) -> bool {
    let max = map.world_size_px();
    let aabb = body.aabb();
    aabb.min_x >= 0.0 && aabb.min_y >= 0.0 && aabb.max_x <= max && aabb.max_y <= max
}

fn body_tile_range(body: UnitBody) -> impl Iterator<Item = (i32, i32)> {
    let ts = config::TILE_SIZE as f32;
    let eps = 0.001;
    let aabb = body.aabb();
    let min_tx = ((aabb.min_x - eps) / ts).floor() as i32;
    let min_ty = ((aabb.min_y - eps) / ts).floor() as i32;
    let max_tx = ((aabb.max_x + eps) / ts).ceil() as i32 - 1;
    let max_ty = ((aabb.max_y + eps) / ts).ceil() as i32 - 1;

    (min_ty..=max_ty).flat_map(move |ty| (min_tx..=max_tx).map(move |tx| (tx, ty)))
}

fn entity_circle_body(e: &Entity) -> CircleBody {
    CircleBody {
        x: e.pos_x,
        y: e.pos_y,
        radius: e.radius(),
    }
}

fn segment_body_facing(kind: EntityKind, from: (f32, f32), to: (f32, f32)) -> f32 {
    if !uses_oriented_vehicle_body(kind) {
        return 0.0;
    }
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let dist2 = dx * dx + dy * dy;
    if !dist2.is_finite() || dist2 <= 1.0e-4 {
        0.0
    } else {
        dy.atan2(dx)
    }
}

#[cfg(test)]
#[path = "standability/build_site_tests.rs"]
mod build_site_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::services::occupancy::footprint_center;

    #[test]
    fn unit_static_standable_rejects_body_clipping_building() {
        let map = flat_map(12);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Depot, 4, 4);
        entities
            .spawn_building(1, EntityKind::Depot, bx, by, true)
            .expect("depot should spawn");
        let occ = Occupancy::build(&map, &entities);
        let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
        let radius = config::unit_stats(EntityKind::Tank)
            .expect("tank stats")
            .radius;

        assert!(!unit_static_standable(
            &map,
            &occ,
            EntityKind::Tank,
            rect.max_x + radius - 1.0,
            rect.min_y + 32.0,
        ));
    }

    #[test]
    fn unit_static_standable_rejects_body_touching_building_edge() {
        let map = flat_map(12);
        let mut entities = EntityStore::new();
        let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
        let radius = config::unit_stats(EntityKind::Rifleman)
            .expect("rifleman stats")
            .radius;
        let building = footprint_center(&map, EntityKind::Depot, 4, 4);
        entities
            .spawn_building(1, EntityKind::Depot, building.0, building.1, true)
            .expect("depot should spawn");
        let occ = Occupancy::build(&map, &entities);

        assert!(!unit_static_standable(
            &map,
            &occ,
            EntityKind::Rifleman,
            rect.min_x - radius,
            rect.min_y + 32.0,
        ));
        assert!(!unit_static_standable(
            &map,
            &occ,
            EntityKind::Rifleman,
            rect.min_x + 32.0,
            rect.max_y + radius,
        ));
    }

    #[test]
    fn infantry_can_stand_on_tank_trap_but_vehicle_bodies_cannot() {
        let map = flat_map(12);
        let mut entities = EntityStore::new();
        let (x, y) = footprint_center(&map, EntityKind::TankTrap, 5, 5);
        entities
            .spawn_building(1, EntityKind::TankTrap, x, y, true)
            .expect("tank trap should spawn");
        let occ = Occupancy::build(&map, &entities);

        assert!(unit_static_standable(&map, &occ, EntityKind::Worker, x, y,));
        assert!(unit_static_standable(
            &map,
            &occ,
            EntityKind::Rifleman,
            x,
            y,
        ));
        for kind in [
            EntityKind::AntiTankGun,
            EntityKind::MortarTeam,
            EntityKind::Artillery,
            EntityKind::ScoutCar,
            EntityKind::Tank,
            EntityKind::CommandCar,
        ] {
            assert!(
                !unit_static_standable(&map, &occ, kind, x, y),
                "{kind:?} should reject Tank Trap footprint"
            );
        }
    }

    #[test]
    fn tank_trap_scaffold_blocks_vehicle_body_standability() {
        let map = flat_map(12);
        let mut entities = EntityStore::new();
        let (x, y) = footprint_center(&map, EntityKind::TankTrap, 5, 5);
        entities
            .spawn_building(1, EntityKind::TankTrap, x, y, false)
            .expect("tank trap scaffold should spawn");
        let occ = Occupancy::build(&map, &entities);

        assert!(unit_static_standable(&map, &occ, EntityKind::Worker, x, y,));
        assert!(!unit_static_standable(&map, &occ, EntityKind::Tank, x, y,));
    }

    #[test]
    fn all_ground_unit_bodies_can_stand_on_pump_jack() {
        let map = flat_map(12);
        let mut entities = EntityStore::new();
        let (x, y) = footprint_center(&map, EntityKind::PumpJack, 5, 5);
        entities
            .spawn_building(1, EntityKind::PumpJack, x, y, true)
            .expect("pump jack should spawn");
        let occ = Occupancy::build(&map, &entities);

        for kind in [
            EntityKind::Worker,
            EntityKind::Golem,
            EntityKind::Rifleman,
            EntityKind::MachineGunner,
            EntityKind::AntiTankGun,
            EntityKind::MortarTeam,
            EntityKind::Artillery,
            EntityKind::ScoutCar,
            EntityKind::Tank,
            EntityKind::CommandCar,
            EntityKind::Ekat,
        ] {
            assert!(
                unit_static_standable(&map, &occ, kind, x, y),
                "{kind:?} should ignore Pump Jack footprint for standability"
            );
        }
    }

    #[test]
    fn unit_spawn_standable_rejects_existing_unit_overlap() {
        let map = flat_map(12);
        let mut entities = EntityStore::new();
        entities
            .spawn_unit(1, EntityKind::Worker, 160.0, 160.0)
            .expect("worker should spawn");
        let occ = Occupancy::build(&map, &entities);

        assert!(!unit_spawn_standable(
            &map,
            &occ,
            &entities,
            EntityKind::Worker,
            160.0,
            160.0,
        ));
    }

    #[test]
    fn building_site_clear_rejects_unit_body_intersection() {
        let map = flat_map(12);
        let mut entities = EntityStore::new();
        let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
        let radius = config::unit_stats(EntityKind::Tank)
            .expect("tank stats")
            .radius;
        entities
            .spawn_unit(
                1,
                EntityKind::Tank,
                rect.max_x + radius - 1.0,
                rect.min_y + 32.0,
            )
            .expect("tank should spawn");

        assert!(!building_site_clear(
            &map,
            &entities,
            EntityKind::Depot,
            4,
            4,
        ));
    }

    #[test]
    fn tank_trap_build_policy_allows_infantry_overlap_but_rejects_vehicle_body() {
        let map = flat_map(12);
        let mut infantry_entities = EntityStore::new();
        let (x, y) = footprint_center(&map, EntityKind::TankTrap, 4, 4);
        let worker = infantry_entities
            .spawn_unit(1, EntityKind::Worker, x, y)
            .expect("worker should spawn");

        assert!(building_site_clear_for_build_intent(
            &map,
            &infantry_entities,
            EntityKind::TankTrap,
            4,
            4,
            worker,
        ));
        assert!(
            !building_site_clear(&map, &infantry_entities, EntityKind::Depot, 4, 4),
            "ordinary buildings still reject infantry body overlap"
        );

        let mut vehicle_entities = EntityStore::new();
        let builder = vehicle_entities
            .spawn_unit(1, EntityKind::Worker, x - config::TILE_SIZE as f32 * 2.0, y)
            .expect("worker should spawn");
        vehicle_entities
            .spawn_unit(1, EntityKind::Tank, x, y)
            .expect("tank should spawn");

        assert!(!building_site_clear_for_build_intent(
            &map,
            &vehicle_entities,
            EntityKind::TankTrap,
            4,
            4,
            builder,
        ));
    }

    #[test]
    fn building_site_rejects_tank_body_touching_footprint_edge() {
        let map = flat_map(12);
        let mut entities = EntityStore::new();
        let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
        let radius = config::unit_stats(EntityKind::Tank)
            .expect("tank stats")
            .radius;
        entities
            .spawn_unit(1, EntityKind::Tank, rect.max_x + radius, rect.min_y + 32.0)
            .expect("tank should spawn");

        assert_eq!(
            map.tile_of(rect.max_x + radius, rect.min_y + 32.0).0,
            6,
            "tank center should be outside the depot footprint tiles"
        );
        assert!(!building_site_clear(
            &map,
            &entities,
            EntityKind::Depot,
            4,
            4,
        ));
    }

    #[test]
    fn scout_car_static_standability_allows_shaved_front_corner_clearance() {
        let map = flat_map(12);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Depot, 4, 4);
        entities
            .spawn_building(1, EntityKind::Depot, bx, by, true)
            .expect("depot should spawn");
        let occ = Occupancy::build(&map, &entities);
        let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
        let radius = config::SCOUT_CAR_BODY_WIDTH_PX * 0.5 + config::SCOUT_CAR_BODY_CLEARANCE_PX;
        let half_segment =
            config::SCOUT_CAR_BODY_LENGTH_PX * 0.5 - config::SCOUT_CAR_BODY_WIDTH_PX * 0.5;
        let cap_corner_gap = radius * 0.72;

        assert!(unit_static_standable_with_facing(
            &map,
            &occ,
            EntityKind::ScoutCar,
            rect.min_x - cap_corner_gap - half_segment,
            rect.min_y - cap_corner_gap,
            0.0,
        ));
    }

    #[test]
    fn scout_car_static_standability_allows_shaved_rear_corner_clearance() {
        let map = flat_map(12);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Depot, 4, 4);
        entities
            .spawn_building(1, EntityKind::Depot, bx, by, true)
            .expect("depot should spawn");
        let occ = Occupancy::build(&map, &entities);
        let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
        let radius = config::SCOUT_CAR_BODY_WIDTH_PX * 0.5 + config::SCOUT_CAR_BODY_CLEARANCE_PX;
        let half_segment =
            config::SCOUT_CAR_BODY_LENGTH_PX * 0.5 - config::SCOUT_CAR_BODY_WIDTH_PX * 0.5;
        let cap_corner_gap = radius * 0.72;

        assert!(unit_static_standable_with_facing(
            &map,
            &occ,
            EntityKind::ScoutCar,
            rect.max_x + cap_corner_gap + half_segment,
            rect.min_y - cap_corner_gap,
            0.0,
        ));
    }

    #[test]
    fn scout_car_static_standability_still_rejects_building_overlap_and_diagonal_pinch() {
        let mut pinch_map = flat_map(12);
        set_tile(&mut pinch_map, 4, 4, crate::protocol::terrain::WATER);
        set_tile(&mut pinch_map, 5, 5, crate::protocol::terrain::ROCK);
        let empty = EntityStore::new();
        let pinch_occ = Occupancy::build(&pinch_map, &empty);

        assert!(!unit_static_standable_with_facing(
            &pinch_map,
            &pinch_occ,
            EntityKind::ScoutCar,
            5.0 * config::TILE_SIZE as f32,
            5.0 * config::TILE_SIZE as f32,
            std::f32::consts::FRAC_PI_4,
        ));

        let map = flat_map(12);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Depot, 4, 4);
        entities
            .spawn_building(1, EntityKind::Depot, bx, by, true)
            .expect("depot should spawn");
        let occ = Occupancy::build(&map, &entities);
        let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");

        assert!(!unit_static_standable_with_facing(
            &map,
            &occ,
            EntityKind::ScoutCar,
            rect.min_x + 16.0,
            rect.min_y + 16.0,
            0.0,
        ));
    }

    #[test]
    fn build_intent_ignores_only_the_chosen_builder_body() {
        let map = flat_map(12);
        let mut entities = EntityStore::new();
        let (x, y) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let builder = entities
            .spawn_unit(1, EntityKind::Worker, x, y)
            .expect("worker should spawn");
        let other = entities
            .spawn_unit(1, EntityKind::Worker, x, y)
            .expect("other worker should spawn");

        assert!(!building_site_clear_for_build_intent(
            &map,
            &entities,
            EntityKind::Depot,
            4,
            4,
            builder,
        ));
        if let Some(other) = entities.get_mut(other) {
            other.hp = 0;
        }
        assert!(building_site_clear_for_build_intent(
            &map,
            &entities,
            EntityKind::Depot,
            4,
            4,
            builder,
        ));
    }

    #[test]
    fn building_site_clear_rejects_resource_node_footprint() {
        let map = flat_map(12);
        let mut entities = EntityStore::new();
        let (nx, ny) = map.tile_center(4, 4);
        entities
            .spawn_node(EntityKind::Steel, nx, ny)
            .expect("steel node should spawn");

        assert!(!building_site_clear(
            &map,
            &entities,
            EntityKind::Depot,
            4,
            4,
        ));
    }

    #[test]
    fn standability_rejects_non_finite_coordinates() {
        let map = flat_map(12);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);

        assert!(!unit_static_standable(
            &map,
            &occ,
            EntityKind::Worker,
            f32::NAN,
            160.0,
        ));
        assert!(!unit_spawn_standable(
            &map,
            &occ,
            &entities,
            EntityKind::Worker,
            160.0,
            f32::INFINITY,
        ));
    }

    #[test]
    fn ejection_standability_preserves_oriented_vehicle_facing() {
        let mut map = flat_map(12);
        set_tile(&mut map, 5, 5, crate::protocol::terrain::ROCK);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let candidate = map.tile_center(5, 4);

        assert!(unit_static_standable_with_facing(
            &map,
            &occ,
            EntityKind::Tank,
            candidate.0,
            candidate.1,
            0.0,
        ));
        assert!(!unit_static_standable_with_facing(
            &map,
            &occ,
            EntityKind::Tank,
            candidate.0,
            candidate.1,
            std::f32::consts::FRAC_PI_2,
        ));
        assert!(!unit_position_clear_of_building_footprint(
            &map,
            &occ,
            &entities,
            &BTreeSet::new(),
            EntityKind::Tank,
            std::f32::consts::FRAC_PI_2,
            candidate.0,
            candidate.1,
            EntityKind::PumpJack,
            9,
            9,
        ));
    }

    #[test]
    fn building_site_clear_accepts_empty_passable_site() {
        let map = flat_map(12);
        let entities = EntityStore::new();

        assert!(building_site_clear(
            &map,
            &entities,
            EntityKind::Depot,
            4,
            4,
        ));
    }

    #[test]
    fn unit_static_segment_standable_accepts_open_segment() {
        let map = flat_map(12);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);

        assert!(unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::Rifleman,
            map.tile_center(2, 2),
            map.tile_center(8, 8),
        ));
    }

    #[test]
    fn unit_static_segment_standable_rejects_terrain_blockers() {
        let mut map = flat_map(12);
        set_tile(&mut map, 5, 4, crate::protocol::terrain::WATER);
        set_tile(&mut map, 5, 5, crate::protocol::terrain::ROCK);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);

        assert!(!unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::Rifleman,
            map.tile_center(2, 5),
            map.tile_center(8, 5),
        ));
        assert!(!unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::Rifleman,
            map.tile_center(5, 2),
            map.tile_center(5, 8),
        ));
    }

    #[test]
    fn unit_static_segment_standable_rejects_building_footprint() {
        let map = flat_map(12);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Depot, 4, 4);
        entities
            .spawn_building(1, EntityKind::Depot, bx, by, true)
            .expect("depot should spawn");
        let occ = Occupancy::build(&map, &entities);

        assert!(!unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::Rifleman,
            map.tile_center(2, 5),
            map.tile_center(8, 5),
        ));
    }

    #[test]
    fn unit_static_segment_standable_rejects_out_of_bounds_endpoint() {
        let map = flat_map(12);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);

        assert!(!unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::Worker,
            map.tile_center(2, 2),
            (map.world_size_px() + 1.0, map.tile_center(2, 2).1),
        ));
    }

    #[test]
    fn unit_static_segment_standable_rejects_tank_radius_clipping_blocker() {
        let mut map = flat_map(12);
        set_tile(&mut map, 5, 5, crate::protocol::terrain::ROCK);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let y = 5.0 * config::TILE_SIZE as f32 - 10.0;

        assert!(!unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::Tank,
            (3.5 * config::TILE_SIZE as f32, y),
            (7.5 * config::TILE_SIZE as f32, y),
        ));
    }

    #[test]
    fn unit_static_segment_standable_allows_rifleman_where_tank_clips() {
        let mut map = flat_map(12);
        set_tile(&mut map, 5, 5, crate::protocol::terrain::ROCK);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let from = (
            3.5 * config::TILE_SIZE as f32,
            5.0 * config::TILE_SIZE as f32 - 10.0,
        );
        let to = (
            7.5 * config::TILE_SIZE as f32,
            5.0 * config::TILE_SIZE as f32 - 10.0,
        );

        assert!(unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::Rifleman,
            from,
            to,
        ));
        assert!(!unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::Tank,
            from,
            to,
        ));
    }

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![crate::protocol::terrain::GRASS; (size * size) as usize],
            starts: vec![],
            base_sites: vec![],
        }
    }

    fn set_tile(map: &mut Map, x: u32, y: u32, terrain: u8) {
        let index = map.index(x, y);
        map.terrain[index] = terrain;
    }
}
