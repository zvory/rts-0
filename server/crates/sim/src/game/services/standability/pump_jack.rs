use super::{building_rect_for_entity, Entity, EntityKind, EntityStore, Map, RectBody};

const POINT_IN_RECT_EPS_PX: f32 = 0.001;

pub(super) fn live_oil_node_centers_in_rect<'a>(
    entities: impl Iterator<Item = &'a Entity>,
    rect: RectBody,
) -> Vec<u32> {
    entities
        .into_iter()
        .filter_map(|entity| {
            (entity.kind == EntityKind::Oil
                && entity.remaining().unwrap_or(0) > 0
                && point_inside_rect((entity.pos_x, entity.pos_y), rect))
            .then_some(entity.id)
        })
        .collect()
}

pub(super) fn oil_node_center_in_rect(entity: &Entity, rect: RectBody) -> bool {
    entity.kind == EntityKind::Oil && point_inside_rect((entity.pos_x, entity.pos_y), rect)
}

pub(super) fn oil_node_has_pump_jack(map: &Map, entities: &EntityStore, oil_id: u32) -> bool {
    let Some(oil) = entities.get(oil_id) else {
        return false;
    };
    entities.iter().any(|entity| {
        entity.kind == EntityKind::PumpJack
            && entity.hp > 0
            && building_rect_for_entity(map, entity)
                .is_some_and(|rect| point_inside_rect((oil.pos_x, oil.pos_y), rect))
    })
}

fn point_inside_rect(point: (f32, f32), rect: RectBody) -> bool {
    point.0 >= rect.min_x - POINT_IN_RECT_EPS_PX
        && point.0 <= rect.max_x + POINT_IN_RECT_EPS_PX
        && point.1 >= rect.min_y - POINT_IN_RECT_EPS_PX
        && point.1 <= rect.max_y + POINT_IN_RECT_EPS_PX
}
