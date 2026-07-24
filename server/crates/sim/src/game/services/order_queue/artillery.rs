use super::*;

pub(super) fn intent_valid(
    map: &Map,
    entities: &EntityStore,
    owner: u32,
    id: u32,
    x: f32,
    y: f32,
) -> bool {
    if x < 0.0 || y < 0.0 || x >= map.world_size_px() || y >= map.world_size_px() {
        return false;
    }
    artillery_point_fire_target(
        map,
        entities,
        owner,
        id,
        x,
        y,
        ArtilleryPointFireAcceptance::Command,
    )
    .is_some()
}

pub(super) fn execute(
    map: &Map,
    entities: &mut EntityStore,
    coordinator: &mut MoveCoordinator<'_>,
    id: u32,
    point: (f32, f32),
    mode: ArtilleryFireMode,
    radius_tiles: f32,
) -> bool {
    let (x, y) = point;
    let Some(owner) = entities.get(id).map(|e| e.owner) else {
        return false;
    };
    let Some(target) = artillery_point_fire_target(
        map,
        entities,
        owner,
        id,
        x,
        y,
        ArtilleryPointFireAcceptance::Command,
    ) else {
        return false;
    };
    if !target.in_range {
        let blanket_radius = matches!(mode, ArtilleryFireMode::Blanket).then_some(radius_tiles);
        return crate::game::services::ability_orders::queue_artillery_fire_reposition(
            map,
            entities,
            coordinator,
            id,
            point,
            crate::game::services::order_execution::artillery_ability(mode),
            blanket_radius,
        );
    }
    start_artillery_fire_promoted_order(entities, id, target, mode, radius_tiles)
}

pub(super) fn discard_failed_fire_intent(
    entities: &mut EntityStore,
    id: u32,
    mode: ArtilleryFireMode,
    x: f32,
    y: f32,
) {
    let matches_failed_reposition = entities
        .get(id)
        .and_then(|entity| entity.queued_orders().first())
        .is_some_and(|intent| match (mode, intent) {
            (ArtilleryFireMode::Point, OrderIntent::PointFire(point))
            | (ArtilleryFireMode::Blanket, OrderIntent::BlanketFire { point, .. }) => {
                point.x.to_bits() == x.to_bits() && point.y.to_bits() == y.to_bits()
            }
            _ => false,
        });
    if matches_failed_reposition {
        if let Some(entity) = entities.get_mut(id) {
            entity.pop_queued_order();
        }
    }
}
