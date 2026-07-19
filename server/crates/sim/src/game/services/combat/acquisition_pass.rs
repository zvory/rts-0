use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;

use super::acquisition::{resolve_target, CombatMode};
use super::shot_blocker_index::ShotBlockerIndex;
use super::target_legality::auto_target_legality;
use super::weapons::anti_tank_gun_target_inside_field_of_fire;

#[allow(clippy::too_many_arguments)]
pub(super) fn acquire(
    map: &Map,
    entities: &EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
    occ: &crate::game::services::occupancy::Occupancy,
    spatial: &crate::game::services::spatial::SpatialIndex,
    los: &crate::game::services::line_of_sight::LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
    mode: CombatMode,
    can_move_fire: bool,
    is_mortar_team: bool,
    min_range_px: f32,
    range_px: f32,
    require_safe_mortar_target: bool,
    tick: u32,
) -> Option<u32> {
    let tank_trap_obstructs_vehicle_route = |attacker: &Entity, target: &Entity| {
        occ.tank_trap_obstructs_vehicle_route(attacker, target, teams)
    };
    resolve_target(
        map,
        entities,
        blockers,
        teams,
        spatial,
        los,
        fog,
        smokes,
        &tank_trap_obstructs_vehicle_route,
        id,
        owner,
        px,
        py,
        acquire_px,
        mode,
        can_move_fire,
        &|target_id| {
            (!is_mortar_team
                || super::mortar_autocast_target_eligible(
                    entities,
                    id,
                    target_id,
                    min_range_px,
                    range_px,
                ))
                && (!require_safe_mortar_target
                    || super::mortar_autocast_target_safe(
                        entities, teams, fog, spatial, owner, id, target_id, tick,
                    ))
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select(
    map: &Map,
    entities: &EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
    occ: &crate::game::services::occupancy::Occupancy,
    spatial: &crate::game::services::spatial::SpatialIndex,
    los: &crate::game::services::line_of_sight::LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
    mode: CombatMode,
    can_move_fire: bool,
    weapon: crate::rules::combat::WeaponKind,
    is_mortar_team: bool,
    min_range_px: f32,
    range_px: f32,
    require_safe_mortar_target: bool,
    tick: u32,
) -> Option<u32> {
    let ready = entities
        .get(id)
        .is_some_and(|entity| entity.weapon_cooldown(weapon) == 0);
    let target_filter = |target_id| {
        (!is_mortar_team
            || super::mortar_autocast_target_eligible(
                entities,
                id,
                target_id,
                min_range_px,
                range_px,
            ))
            && (!require_safe_mortar_target
                || super::mortar_autocast_target_safe(
                    entities, teams, fog, spatial, owner, id, target_id, tick,
                ))
    };
    let retained = retained_target(
        map,
        entities,
        blockers,
        teams,
        los,
        fog,
        smokes,
        id,
        owner,
        px,
        py,
        acquire_px,
        mode,
        can_move_fire,
        range_px,
        ready,
        &target_filter,
    );
    if !ready || retained.is_some() {
        return retained;
    }
    acquire(
        map,
        entities,
        blockers,
        teams,
        occ,
        spatial,
        los,
        fog,
        smokes,
        id,
        owner,
        px,
        py,
        acquire_px,
        mode,
        can_move_fire,
        is_mortar_team,
        min_range_px,
        range_px,
        require_safe_mortar_target,
        tick,
    )
}

#[allow(clippy::too_many_arguments)]
fn retained_target(
    map: &Map,
    entities: &EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
    los: &crate::game::services::line_of_sight::LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
    mode: CombatMode,
    attacker_can_fire_while_moving: bool,
    weapon_range_px: f32,
    require_weapon_range: bool,
    target_filter: &dyn Fn(u32) -> bool,
) -> Option<u32> {
    if smokes.point_inside(px, py) {
        return None;
    }
    let attacker = entities.get(self_id)?;
    if mode == CombatMode::Ordered {
        if let Some(target) = attacker.order().attack_target() {
            if crate::game::services::world_query::unit_explicit_attack_target_valid(
                entities,
                teams,
                fog,
                Some(smokes),
                owner,
                self_id,
                target,
            ) {
                return Some(target);
            }
        }
        // Once the commanded target is invalid, resolve_target() falls back to normal
        // auto-acquisition. Preserve that fallback through its cooldown cycle too.
    }
    if mode == CombatMode::Passive {
        return None;
    }

    let target_id = attacker.target_id()?;
    let target = entities.get(target_id)?;
    if attacker.kind == EntityKind::AntiTankGun {
        let target_angle = (target.pos_y - py).atan2(target.pos_x - px);
        if !anti_tank_gun_target_inside_field_of_fire(attacker, target_angle) {
            // Explicit attacks returned above preserve their commanded target. Automatic retention
            // must not drag an emplaced gun away from another target already in its current cone.
            return None;
        }
    }
    let legality = auto_target_legality(
        map,
        entities,
        blockers,
        teams,
        los,
        fog,
        smokes,
        self_id,
        owner,
        px,
        py,
        acquire_px,
        weapon_range_px,
        attacker.kind,
        attacker_can_fire_while_moving,
        target,
    )?;
    if ((require_weapon_range || mode == CombatMode::Opportunistic) && !legality.in_weapon_range)
        || !target_filter(target_id)
    {
        return None;
    }
    Some(target_id)
}
