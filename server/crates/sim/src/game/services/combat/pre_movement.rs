use crate::config;
use crate::game::entity::{AttackPhase, EntityKind, EntityStore, MovePhase, Order, WeaponSetup};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;

use super::acquisition::CombatMode;
use super::acquisition_pass;
use super::shot_blocker_index::ShotBlockerIndex;
use super::weapons::effective_attack_profile;
use super::{LineOfSight, SpatialIndex, RANGE_SLACK};

/// Resolve stationary attack-move engagements before movement spends a tank's charged range
/// or a fresh ground order interrupts an MG's existing setup. Travelling units scan later.
pub(in crate::game) fn hold_attack_move_engagements(
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
            e.hp > 0
                && matches!(e.order(), Order::AttackMove(_))
                && (!e.path_is_empty() || e.move_phase() == Some(MovePhase::AwaitingPath))
                && match e.kind {
                    EntityKind::Tank => {
                        e.target_id().is_none()
                            && effective_attack_profile(e).range_tiles > base_range
                    }
                    EntityKind::MachineGunner => matches!(
                        e.weapon_setup(),
                        WeaponSetup::SettingUp { .. } | WeaponSetup::Deployed
                    ),
                    _ => false,
                }
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
        let Some((owner, x, y, range_px, is_mg)) = entities.get(id).map(|e| {
            let range_px = effective_attack_profile(e).range_tiles * config::TILE_SIZE as f32
                + e.radius()
                + RANGE_SLACK;
            (
                e.owner,
                e.pos_x(),
                e.pos_y(),
                range_px,
                e.kind == EntityKind::MachineGunner,
            )
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
            !is_mg,
        )
        .filter(|target| {
            if !is_mg {
                return true;
            }
            let Some(attacker) = entities.get(id) else {
                return false;
            };
            let Some(enemy) = entities.get(*target) else {
                return false;
            };
            let profile = effective_attack_profile(attacker);
            let Some(weapon) = profile.weapon else {
                return false;
            };
            let distance_sq =
                super::projection::primary_target_distance_sq(map, weapon.id, (x, y), enemy);
            let min_px = profile.min_range_tiles * config::TILE_SIZE as f32;
            distance_sq <= range_px * range_px
                && distance_sq >= min_px * min_px
                && super::target_legality::direct_fire_target_legal(
                    map,
                    entities,
                    &blockers,
                    teams,
                    &los,
                    fog,
                    smokes,
                    id,
                    owner,
                    (x, y),
                    *target,
                    super::target_legality::DirectFireLegality::AutoAcquire,
                )
        });
        if is_mg && target.is_none() {
            if let Some(mg) = entities.get_mut(id) {
                // A fresh order should walk when there is nothing to engage. Ongoing orders
                // retain combat's existing no-target grace instead of packing on target flicker.
                if mg
                    .combat
                    .as_ref()
                    .is_some_and(|c| c.attack_move_no_target_ticks == 0)
                {
                    mg.begin_weapon_teardown_for_movement();
                }
            }
        }
        if let (Some(target), Some(unit)) = (target, entities.get_mut(id)) {
            unit.set_target_id(Some(target));
            unit.clear_path();
            unit.mark_attack_phase(AttackPhase::Firing);
        }
    }
}
