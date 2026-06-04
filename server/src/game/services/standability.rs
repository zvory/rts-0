use crate::config;
use crate::game::entity::{uses_oriented_vehicle_body, Entity, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::pathfinding::Passability;
use crate::game::services::geometry::{
    building_rect_for_entity, building_rect_for_footprint, circle_intersects_rect, rects_intersect,
    tile_rect, unit_bodies_intersect, unit_body, unit_body_for_entity, unit_body_intersects_rect,
    unit_body_with_facing, CircleBody, RectBody, UnitBody,
};
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;

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
        if !occ.passable(tx, ty) && unit_body_intersects_rect(body, tile) {
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
    building_site_clear_with_ignored_unit(map, entities, building, tile_x, tile_y, None)
}

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

    spatial
        .ids_in_rect(min_tx, min_ty, max_tx, max_ty)
        .all(|id| {
            entities
                .get(id)
                .is_none_or(|e| entity_clear_of_building_rect(map, e, rect, None))
        })
}

pub(crate) fn building_site_clear_for_build_intent(
    map: &Map,
    entities: &EntityStore,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
    builder_id: u32,
) -> bool {
    building_site_clear_with_ignored_unit(map, entities, building, tile_x, tile_y, Some(builder_id))
}

fn building_site_clear_with_ignored_unit(
    map: &Map,
    entities: &EntityStore,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
    ignored_unit: Option<u32>,
) -> bool {
    let Some(rect) = building_rect_for_footprint(building, tile_x, tile_y) else {
        return false;
    };
    if !footprint_in_bounds_and_passable(map, building, tile_x, tile_y) {
        return false;
    }

    entities
        .iter()
        .all(|e| entity_clear_of_building_rect(map, e, rect, ignored_unit))
}

fn entity_clear_of_building_rect(
    map: &Map,
    e: &Entity,
    rect: RectBody,
    ignored_unit: Option<u32>,
) -> bool {
    if e.hp == 0 {
        return true;
    }
    if e.is_unit() && ignored_unit == Some(e.id) {
        return true;
    }
    if e.is_building() {
        return building_rect_for_entity(map, e).is_none_or(|other| !rects_intersect(rect, other));
    }
    if e.is_node() {
        return !circle_intersects_rect(entity_circle_body(e), rect);
    }
    if e.is_unit() {
        return unit_body_for_entity(e).is_none_or(|body| !unit_body_intersects_rect(body, rect));
    }
    true
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
    fn spatial_building_site_clear_matches_full_scan_for_blockers() {
        let map = flat_map(12);
        let mut cases = Vec::new();

        let mut blocked_by_building = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Depot, 4, 4);
        blocked_by_building
            .spawn_building(1, EntityKind::Depot, bx, by, true)
            .expect("depot should spawn");
        cases.push(blocked_by_building);

        let mut blocked_by_unit = EntityStore::new();
        let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
        let radius = config::unit_stats(EntityKind::Tank)
            .expect("tank stats")
            .radius;
        blocked_by_unit
            .spawn_unit(
                1,
                EntityKind::Tank,
                rect.max_x + radius - 1.0,
                rect.min_y + 32.0,
            )
            .expect("tank should spawn");
        cases.push(blocked_by_unit);

        let mut blocked_by_resource = EntityStore::new();
        let (nx, ny) = map.tile_center(4, 4);
        blocked_by_resource
            .spawn_node(EntityKind::Steel, nx, ny)
            .expect("steel node should spawn");
        cases.push(blocked_by_resource);

        cases.push(EntityStore::new());

        for entities in cases {
            let spatial = SpatialIndex::build(&entities, map.size);
            assert_eq!(
                building_site_clear_spatial(&map, &entities, &spatial, EntityKind::Depot, 4, 4),
                building_site_clear(&map, &entities, EntityKind::Depot, 4, 4)
            );
        }
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
            expansion_sites: vec![],
        }
    }

    fn set_tile(map: &mut Map, x: u32, y: u32, terrain: u8) {
        let index = map.index(x, y);
        map.terrain[index] = terrain;
    }
}
