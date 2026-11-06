use crate::config;
use crate::game::entity::{AttackPhase, EntityKind, EntityStore, Order};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;

use super::acquisition::CombatMode;
use super::acquisition_pass;
use super::shot_blocker_index::ShotBlockerIndex;
use super::weapons::effective_attack_profile;
use super::{LineOfSight, SpatialIndex, RANGE_SLACK};

/// Preserve a tank's built-up stationary range when an attack-move path is about to spend it.
/// Ordinary travelling tanks already scan during the later combat pass.
pub(in crate::game) fn hold_charged_attack_move_tanks(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
) {
    let base_range = crate::rules::combat::attack_profile(EntityKind::Tank).range_tiles;
    let candidates: Vec<u32> = entities
        .iter()
        .filter(|e| {
            e.kind == EntityKind::Tank
                && e.hp > 0
                && matches!(e.order(), Order::AttackMove(_))
                && !e.path_is_empty()
                && e.target_id().is_none()
                && effective_attack_profile(e).range_tiles > base_range
        })
        .map(|e| e.id)
        .collect();
    if candidates.is_empty() {
        return;
    }

    let spatial = SpatialIndex::build(entities, map.width, map.height);
    let blockers = ShotBlockerIndex::build(map, entities);
    let los = LineOfSight::with_smoke(map, smokes);
    for id in candidates {
        let Some((owner, x, y, range_px)) = entities.get(id).map(|e| {
            let range_px = effective_attack_profile(e).range_tiles * config::TILE_SIZE as f32
                + e.radius()
                + RANGE_SLACK;
            (e.owner, e.pos_x(), e.pos_y(), range_px)
        }) else {
            continue;
        };
        let target = acquisition_pass::acquire(
            map,
            entities,
            &blockers,
            teams,
            &spatial,
            &los,
            fog,
            smokes,
            id,
            owner,
            x,
            y,
            range_px,
            CombatMode::Aggressive,
            true,
        );
        if let (Some(target), Some(tank)) = (target, entities.get_mut(id)) {
            tank.set_target_id(Some(target));
            tank.clear_path();
            tank.mark_attack_phase(AttackPhase::Firing);
        }
    }
}
