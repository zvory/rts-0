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
        let ability = match mode {
            ArtilleryFireMode::Point => AbilityKind::PointFire,
            ArtilleryFireMode::Blanket => AbilityKind::BlanketFire,
        };
        let Some(staging) =
            crate::game::services::ability_orders::staging_point(map, entities, id, ability, x, y)
        else {
            return false;
        };
        coordinator.order_ability(entities, id, ability, (x, y), staging);
        let intent = match mode {
            ArtilleryFireMode::Point => OrderIntent::point_fire(x, y),
            ArtilleryFireMode::Blanket => OrderIntent::blanket_fire(x, y, radius_tiles),
        };
        return entities
            .get_mut(id)
            .is_some_and(|entity| entity.append_queued_order(intent));
    }
    start_artillery_fire_promoted_order(entities, id, target, mode, radius_tiles)
}
