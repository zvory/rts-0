use crate::game::entity::EntityStore;
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;

use super::acquisition::{direct_fire_target_legal, DirectFireLegality};
use super::shot_blocker_index::ShotBlockerIndex;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SecondaryWeaponActivationConstraints {
    pub facing_rad: f32,
    pub half_arc_rad: f32,
    pub range_px: f32,
    pub direct_fire_legality: DirectFireLegality,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn secondary_weapon_target_passes_activation(
    map: &Map,
    entities: &EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
    los: &crate::game::services::line_of_sight::LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    attacker: u32,
    attacker_owner: u32,
    start: (f32, f32),
    target: u32,
    constraints: SecondaryWeaponActivationConstraints,
) -> bool {
    if !constraints.facing_rad.is_finite()
        || !constraints.half_arc_rad.is_finite()
        || constraints.half_arc_rad < 0.0
        || !constraints.range_px.is_finite()
        || constraints.range_px < 0.0
    {
        return false;
    }
    let Some(target_entity) = entities.get(target) else {
        return false;
    };
    let dx = target_entity.pos_x - start.0;
    let dy = target_entity.pos_y - start.1;
    let distance_sq = dx * dx + dy * dy;
    if !distance_sq.is_finite() || distance_sq > constraints.range_px * constraints.range_px {
        return false;
    }
    let target_angle = dy.atan2(dx);
    if !target_angle.is_finite()
        || angle_delta(constraints.facing_rad, target_angle).abs() > constraints.half_arc_rad
    {
        return false;
    }
    direct_fire_target_legal(
        map,
        entities,
        blockers,
        teams,
        los,
        fog,
        smokes,
        attacker,
        attacker_owner,
        start,
        target,
        constraints.direct_fire_legality,
    )
}

#[allow(dead_code)]
fn angle_delta(from: f32, to: f32) -> f32 {
    (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}
