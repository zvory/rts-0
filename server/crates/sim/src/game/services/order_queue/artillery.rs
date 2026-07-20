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
    stored_artillery_point_fire_target(
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
    id: u32,
    x: f32,
    y: f32,
    mode: ArtilleryFireMode,
    radius_tiles: f32,
) -> bool {
    let Some(owner) = entities.get(id).map(|e| e.owner) else {
        return false;
    };
    let Some(target) = stored_artillery_point_fire_target(
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
    start_artillery_fire_promoted_order(entities, id, target, mode, radius_tiles)
}
