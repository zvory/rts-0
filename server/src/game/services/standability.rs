#![allow(dead_code)] // Phase-zero predicates are intentionally staged before gameplay rewires.

use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::pathfinding::Passability;
use crate::game::services::geometry::{
    building_rect_for_entity, building_rect_for_footprint, circle_intersects_rect, rects_intersect,
    tile_rect, unit_body, CircleBody,
};
use crate::game::services::occupancy::Occupancy;

pub(crate) fn unit_static_standable(
    map: &Map,
    occ: &Occupancy,
    kind: EntityKind,
    x: f32,
    y: f32,
) -> bool {
    let Some(body) = unit_body(kind, x, y) else {
        return false;
    };
    if !circle_inside_world(map, body) {
        return false;
    }

    for (tx, ty) in circle_tile_range(body) {
        if !map.in_bounds(tx, ty) {
            return false;
        }

        let tile = tile_rect(tx, ty);
        if !map.is_passable(tx, ty) && circle_intersects_rect(body, tile) {
            return false;
        }
        if !occ.passable(tx, ty) && circle_intersects_rect(body, tile) {
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
            || unit_entity_body(e).is_none_or(|existing| !circles_intersect(body, existing))
    })
}

pub(crate) fn building_site_clear(
    map: &Map,
    entities: &EntityStore,
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

    entities.iter().all(|e| {
        if e.hp == 0 {
            return true;
        }
        if e.is_building() {
            return building_rect_for_entity(map, e)
                .is_none_or(|other| !rects_intersect(rect, other));
        }
        if e.is_node() {
            return !circle_intersects_rect(entity_circle_body(e), rect);
        }
        if e.is_unit() {
            return unit_entity_body(e).is_none_or(|body| !circle_intersects_rect(body, rect));
        }
        true
    })
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

fn circle_inside_world(map: &Map, body: CircleBody) -> bool {
    let max = map.world_size_px();
    body.x - body.radius >= 0.0
        && body.y - body.radius >= 0.0
        && body.x + body.radius <= max
        && body.y + body.radius <= max
}

fn circle_tile_range(body: CircleBody) -> impl Iterator<Item = (i32, i32)> {
    let ts = config::TILE_SIZE as f32;
    let min_tx = ((body.x - body.radius) / ts).floor() as i32;
    let min_ty = ((body.y - body.radius) / ts).floor() as i32;
    let max_tx = ((body.x + body.radius) / ts).ceil() as i32 - 1;
    let max_ty = ((body.y + body.radius) / ts).ceil() as i32 - 1;

    (min_ty..=max_ty).flat_map(move |ty| (min_tx..=max_tx).map(move |tx| (tx, ty)))
}

fn unit_entity_body(e: &Entity) -> Option<CircleBody> {
    unit_body(e.kind, e.pos_x, e.pos_y)
}

fn entity_circle_body(e: &Entity) -> CircleBody {
    CircleBody {
        x: e.pos_x,
        y: e.pos_y,
        radius: e.radius(),
    }
}

fn circles_intersect(a: CircleBody, b: CircleBody) -> bool {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let r = a.radius + b.radius;
    dx * dx + dy * dy <= r * r
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

        assert!(!unit_static_standable(
            &map,
            &occ,
            EntityKind::Tank,
            rect.max_x + 19.0,
            rect.min_y + 32.0,
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
        entities
            .spawn_unit(1, EntityKind::Tank, rect.max_x + 19.0, rect.min_y + 32.0)
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

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![crate::protocol::terrain::GRASS; (size * size) as usize],
            starts: vec![],
            expansion_sites: vec![],
        }
    }
}
