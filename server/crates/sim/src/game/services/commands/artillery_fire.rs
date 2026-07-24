use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn order_artillery_point_fire(
    map: &Map,
    entities: &mut EntityStore,
    coordinator: &mut MoveCoordinator<'_>,
    players: &mut [PlayerState],
    teams: &TeamRelations,
    fog: &Fog,
    artillery_shells: &mut ArtilleryShellStore,
    firing_reveals: &mut Vec<FiringRevealSource>,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
    tick: u32,
    mode: ArtilleryFireMode,
    radius_tiles: f32,
) -> bool {
    let Some(target) = artillery_point_fire_target(
        map,
        entities,
        player,
        unit,
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
        let Some(staging) = ability_orders::staging_point(map, entities, unit, ability, x, y)
        else {
            return false;
        };
        if let Some(entity) = entities.get_mut(unit) {
            entity.clear_orders();
        }
        coordinator.order_ability(entities, unit, ability, (x, y), staging);
        let intent = match mode {
            ArtilleryFireMode::Point => OrderIntent::point_fire(x, y),
            ArtilleryFireMode::Blanket => OrderIntent::blanket_fire(x, y, radius_tiles),
        };
        return entities
            .get_mut(unit)
            .is_some_and(|entity| entity.append_queued_order(intent));
    }
    if !start_artillery_fire_command_order(entities, unit, target, mode, radius_tiles) {
        return false;
    }
    if !target.inside_field_of_fire {
        return true;
    }
    try_fire_artillery(
        entities,
        players,
        teams,
        fog,
        artillery_shells,
        firing_reveals,
        events,
        player,
        unit,
        target.x,
        target.y,
        tick,
        mode,
        radius_tiles,
    )
}
