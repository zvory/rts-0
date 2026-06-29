use super::{
    building_rect_for_entity, circle_intersects_rect, CircleBody, Entity, EntityKind, EntityStore,
    Map, RectBody,
};

pub(super) fn live_oil_node_intersecting_rect<'a>(
    entities: impl Iterator<Item = &'a Entity>,
    rect: RectBody,
) -> Option<u32> {
    entities.into_iter().find_map(|entity| {
        (entity.kind == EntityKind::Oil
            && entity.remaining().unwrap_or(0) > 0
            && circle_intersects_rect(entity_circle_body(entity), rect))
        .then_some(entity.id)
    })
}

pub(super) fn oil_node_has_pump_jack(map: &Map, entities: &EntityStore, oil_id: u32) -> bool {
    let Some(oil) = entities.get(oil_id) else {
        return false;
    };
    let oil_body = entity_circle_body(oil);
    entities.iter().any(|entity| {
        entity.kind == EntityKind::PumpJack
            && entity.hp > 0
            && building_rect_for_entity(map, entity)
                .is_some_and(|rect| circle_intersects_rect(oil_body, rect))
    })
}

fn entity_circle_body(e: &Entity) -> CircleBody {
    CircleBody {
        x: e.pos_x,
        y: e.pos_y,
        radius: e.radius(),
    }
}
