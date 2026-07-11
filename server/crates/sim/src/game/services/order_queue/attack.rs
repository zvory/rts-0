use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore, PanzerfaustState};
use crate::game::entrenchment_combat;
use crate::game::map::Map;
use crate::game::services::dist2;

const ATTACK_RANGE_SLACK_PX: f32 = 4.0;

pub(super) fn panzerfaust_attack_cycle_active(attacker: &Entity) -> bool {
    attacker.kind == EntityKind::Panzerfaust
        && matches!(
            attacker
                .combat
                .as_ref()
                .and_then(|combat| combat.panzerfaust),
            Some(
                PanzerfaustState::Windup { .. }
                    | PanzerfaustState::InFlight { .. }
                    | PanzerfaustState::Recovery { .. }
            )
        )
}

pub(super) fn attack_can_fire_now(
    map: &Map,
    entities: &EntityStore,
    attacker: &Entity,
    target: u32,
) -> bool {
    let Some(target) = entities.get(target) else {
        return false;
    };
    let Some(stats) = config::unit_stats(attacker.kind) else {
        return false;
    };
    let dmg = if attacker.kind == EntityKind::Panzerfaust {
        config::PANZERFAUST_DAMAGE
    } else {
        stats.dmg
    };
    if dmg == 0 {
        return false;
    }
    let range_tiles = if attacker.kind == EntityKind::Panzerfaust {
        entrenchment_combat::attack_range_tiles(attacker, config::PANZERFAUST_RANGE_TILES as f32)
    } else {
        stats.range_tiles as f32
    };
    let range_px =
        range_tiles * config::TILE_SIZE as f32 + attacker.radius() + ATTACK_RANGE_SLACK_PX;
    if dist2(attacker.pos_x, attacker.pos_y, target.pos_x, target.pos_y) > range_px * range_px {
        return false;
    }
    crate::game::services::line_of_sight::LineOfSight::new(map).clear_between_world_points(
        (attacker.pos_x, attacker.pos_y),
        (target.pos_x, target.pos_y),
    )
}
