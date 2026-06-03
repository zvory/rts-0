use crate::config;
use crate::game::entity::{Entity, EntityKind};
use crate::game::map::Map;
use crate::game::services::occupancy::building_footprint;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CircleBody {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RectBody {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

pub(crate) fn unit_body(kind: EntityKind, x: f32, y: f32) -> Option<CircleBody> {
    let stats = config::unit_stats(kind)?;
    if !x.is_finite() || !y.is_finite() || !stats.radius.is_finite() || stats.radius <= 0.0 {
        return None;
    }
    Some(CircleBody {
        x,
        y,
        radius: stats.radius,
    })
}

pub(crate) fn building_rect_for_footprint(
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> Option<RectBody> {
    let stats = config::building_stats(kind)?;
    if stats.foot_w == 0 || stats.foot_h == 0 {
        return None;
    }

    let max_tile_x = tile_x.checked_add(stats.foot_w)?;
    let max_tile_y = tile_y.checked_add(stats.foot_h)?;
    let ts = config::TILE_SIZE as f32;

    Some(RectBody {
        min_x: tile_x as f32 * ts,
        min_y: tile_y as f32 * ts,
        max_x: max_tile_x as f32 * ts,
        max_y: max_tile_y as f32 * ts,
    })
}

pub(crate) fn building_rect_for_entity(map: &Map, e: &Entity) -> Option<RectBody> {
    config::building_stats(e.kind)?;
    if !e.pos_x.is_finite() || !e.pos_y.is_finite() {
        return None;
    }

    let footprint = building_footprint(map, e);
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0;
    let mut max_y = 0;
    for (tx, ty) in footprint {
        min_x = min_x.min(tx);
        min_y = min_y.min(ty);
        max_x = max_x.max(tx.checked_add(1)?);
        max_y = max_y.max(ty.checked_add(1)?);
    }
    if min_x == u32::MAX || min_y == u32::MAX {
        return None;
    }
    let ts = config::TILE_SIZE as f32;

    Some(RectBody {
        min_x: min_x as f32 * ts,
        min_y: min_y as f32 * ts,
        max_x: max_x as f32 * ts,
        max_y: max_y as f32 * ts,
    })
}

pub(crate) fn circle_intersects_rect(circle: CircleBody, rect: RectBody) -> bool {
    if !valid_circle(circle) || !valid_rect(rect) {
        return false;
    }

    let nearest_x = circle.x.clamp(rect.min_x, rect.max_x);
    let nearest_y = circle.y.clamp(rect.min_y, rect.max_y);
    let dx = circle.x - nearest_x;
    let dy = circle.y - nearest_y;
    dx * dx + dy * dy <= circle.radius * circle.radius
}

pub(crate) fn tile_rect(tx: i32, ty: i32) -> RectBody {
    let ts = config::TILE_SIZE as f32;
    RectBody {
        min_x: tx as f32 * ts,
        min_y: ty as f32 * ts,
        max_x: (tx + 1) as f32 * ts,
        max_y: (ty + 1) as f32 * ts,
    }
}

pub(crate) fn rects_intersect(a: RectBody, b: RectBody) -> bool {
    valid_rect(a)
        && valid_rect(b)
        && a.min_x < b.max_x
        && a.max_x > b.min_x
        && a.min_y < b.max_y
        && a.max_y > b.min_y
}

fn valid_circle(circle: CircleBody) -> bool {
    circle.x.is_finite()
        && circle.y.is_finite()
        && circle.radius.is_finite()
        && circle.radius >= 0.0
}

fn valid_rect(rect: RectBody) -> bool {
    rect.min_x.is_finite()
        && rect.min_y.is_finite()
        && rect.max_x.is_finite()
        && rect.max_y.is_finite()
        && rect.min_x <= rect.max_x
        && rect.min_y <= rect.max_y
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::services::occupancy::footprint_center;

    #[test]
    fn tank_body_intersects_building_even_when_center_tile_is_clear() {
        let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
        let tank =
            unit_body(EntityKind::Tank, rect.max_x + 19.0, rect.min_y + 32.0).expect("tank body");

        assert_eq!((tank.x / config::TILE_SIZE as f32).floor() as u32, 6);
        assert!(circle_intersects_rect(tank, rect));
    }

    #[test]
    fn building_rect_for_entity_matches_centered_footprint_tiles() {
        let map = flat_map(12);
        let (x, y) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let building =
            Entity::new_building(1, EntityKind::Depot, x, y, true).expect("depot should spawn");

        assert_eq!(
            building_rect_for_entity(&map, &building),
            building_rect_for_footprint(EntityKind::Depot, 4, 4)
        );
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
