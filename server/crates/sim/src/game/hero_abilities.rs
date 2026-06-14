use crate::config;
use crate::game::entity::{EntityKind, EntityStore, MovePhase};
use crate::game::map::Map;
use crate::game::services::dist2;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::standability;
use crate::game::teams::TeamRelations;

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

pub(crate) fn ekat_teleport_destination(
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
    standability::unit_static_standable(
        map,
        &occ,
        EntityKind::Ekat,
        destination.0,
        destination.1,
    )
    .then_some(destination)
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_ekat_line_shot(
    entities: &mut EntityStore,
    teams: &TeamRelations,
    player: u32,
    caster: u32,
    target_x: f32,
    target_y: f32,
    range_tiles: Option<u32>,
    tick: u32,
) -> Option<(f32, f32)> {
    let (from_x, from_y) = entities.get(caster).map(|e| (e.pos_x, e.pos_y))?;
    let (to_x, to_y) =
        clamped_world_ability_vector(from_x, from_y, target_x, target_y, range_tiles)?;
    apply_line_shot_damage(
        entities,
        teams,
        player,
        caster,
        (from_x, from_y),
        (to_x, to_y),
        tick,
    );
    Some((to_x, to_y))
}

fn apply_line_shot_damage(
    entities: &mut EntityStore,
    teams: &TeamRelations,
    player: u32,
    caster: u32,
    from: (f32, f32),
    to: (f32, f32),
    tick: u32,
) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let len2 = dx * dx + dy * dy;
    if !len2.is_finite() || len2 <= f32::EPSILON {
        return;
    }
    let half_width_px = config::EKAT_LINE_SHOT_WIDTH_TILES * config::TILE_SIZE as f32 * 0.5;
    let mut hits = Vec::new();
    for id in entities.ids() {
        if id == caster {
            continue;
        }
        let Some(target) = entities.get(id) else {
            continue;
        };
        if target.hp == 0 || target.is_node() || !teams.is_enemy_owner(player, target.owner) {
            continue;
        }
        let tx = target.pos_x - from.0;
        let ty = target.pos_y - from.1;
        let t = ((tx * dx + ty * dy) / len2).clamp(0.0, 1.0);
        let nearest_x = from.0 + dx * t;
        let nearest_y = from.1 + dy * t;
        let distance = dist2(target.pos_x, target.pos_y, nearest_x, nearest_y).sqrt();
        if distance <= target.radius() + half_width_px {
            hits.push(id);
        }
    }
    for id in hits {
        if let Some(target) = entities.get_mut(id) {
            target.apply_damage(
                config::EKAT_LINE_SHOT_DAMAGE,
                Some((player, from, tick)),
            );
        }
    }
}
