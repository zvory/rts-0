use crate::config;
use crate::game::ability::AbilityKind;
use crate::game::ability_projectile::{AbilityProjectileReturnTarget, AbilityProjectileSpec};
use crate::game::entity::{EntityKind, EntityStore, MovePhase};
use crate::game::map::Map;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::standability;

pub(crate) fn clamped_world_ability_vector(
    from_x: f32,
    from_y: f32,
    target_x: f32,
    target_y: f32,
    range_tiles: Option<u32>,
) -> Option<(f32, f32)> {
    let dx = target_x - from_x;
    let dy = target_y - from_y;
    let distance = (dx * dx + dy * dy).sqrt();
    if !distance.is_finite() {
        return None;
    }
    let max_distance = range_tiles.unwrap_or(0) as f32 * config::TILE_SIZE as f32;
    if distance <= f32::EPSILON || max_distance <= f32::EPSILON {
        return Some((from_x, from_y));
    }
    let scale = (max_distance / distance).min(1.0);
    Some((from_x + dx * scale, from_y + dy * scale))
}

pub(crate) fn ekat_dash_destination(
    map: &Map,
    entities: &EntityStore,
    caster: u32,
    x: f32,
    y: f32,
    range_tiles: Option<u32>,
) -> Option<(f32, f32)> {
    let caster_entity = entities.get(caster)?;
    let destination =
        clamped_world_ability_vector(caster_entity.pos_x, caster_entity.pos_y, x, y, range_tiles)?;
    let occ = Occupancy::build(map, entities);
    standability::unit_static_standable(map, &occ, EntityKind::Ekat, destination.0, destination.1)
        .then_some(destination)
}

pub(crate) fn ekat_return_destination_valid(
    map: &Map,
    entities: &EntityStore,
    caster: u32,
    x: f32,
    y: f32,
) -> bool {
    let Some(caster_entity) = entities.get(caster) else {
        return false;
    };
    let occ = Occupancy::build(map, entities);
    standability::unit_static_standable(map, &occ, caster_entity.kind, x, y)
}

pub(crate) fn move_ekat_to(entities: &mut EntityStore, caster: u32, x: f32, y: f32) -> bool {
    let Some(e) = entities.get_mut(caster) else {
        return false;
    };
    e.set_position(x, y);
    e.clear_active_order();
    e.set_path(Vec::new());
    e.set_path_goal(None);
    e.mark_move_phase(MovePhase::Arrived);
    true
}

pub(crate) fn ekat_line_projectile_spec(
    entities: &EntityStore,
    player: u32,
    caster: u32,
    target_x: f32,
    target_y: f32,
    range_tiles: Option<u32>,
    tick: u32,
) -> Option<AbilityProjectileSpec> {
    let (from_x, from_y) = entities.get(caster).map(|e| (e.pos_x, e.pos_y))?;
    ekat_line_projectile_spec_from_origin(
        player,
        caster,
        None,
        (from_x, from_y),
        (target_x, target_y),
        range_tiles,
        tick,
    )
}

pub(crate) fn ekat_line_projectile_spec_from_origin(
    player: u32,
    caster: u32,
    source_object_id: Option<u32>,
    origin: (f32, f32),
    target: (f32, f32),
    range_tiles: Option<u32>,
    tick: u32,
) -> Option<AbilityProjectileSpec> {
    let (from_x, from_y) = origin;
    let (to_x, to_y) =
        clamped_world_ability_vector(from_x, from_y, target.0, target.1, range_tiles)?;
    let range_px =
        range_tiles.unwrap_or(config::EKAT_LINE_SHOT_RANGE_TILES) as f32 * config::TILE_SIZE as f32;
    let max_round_trip_ticks =
        ((range_px * 2.0) / config::EKAT_LINE_SHOT_SPEED_PX_PER_TICK).ceil() as u32;
    Some(AbilityProjectileSpec {
        owner: player,
        caster_id: caster,
        source_object_id,
        ability: AbilityKind::EkatLineShot,
        origin: (from_x, from_y),
        endpoint: (to_x, to_y),
        return_target: AbilityProjectileReturnTarget::Entity { id: caster },
        speed_px_per_tick: config::EKAT_LINE_SHOT_SPEED_PX_PER_TICK,
        width_px: config::EKAT_LINE_SHOT_WIDTH_TILES * config::TILE_SIZE as f32 * 0.5,
        damage: config::EKAT_LINE_SHOT_DAMAGE,
        created_tick: tick,
        expires_tick: tick
            .saturating_add(max_round_trip_ticks)
            .saturating_add(config::TICK_HZ),
    })
}
