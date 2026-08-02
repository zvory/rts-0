use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::rules::terrain::{self, TerrainKind};

use super::projection::{friendly_hard_blocker_between, shot_hits_intended_target};
use super::shot_blocker_index::ShotBlockerIndex;
use super::{Fog, LineOfSight, Map, SmokeCloudStore, TeamRelations};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DirectFireLegality {
    AutoAcquire,
    IntendedTarget,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn direct_fire_target_legal(
    map: &Map,
    entities: &EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
    los: &LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    attacker: u32,
    attacker_owner: u32,
    start: (f32, f32),
    target: u32,
    legality: DirectFireLegality,
) -> bool {
    let Some(attacker_entity) = entities.get(attacker) else {
        return false;
    };
    let Some(target_entity) = entities.get(target) else {
        return false;
    };
    if !crate::rules::target::default_weapon_can_target(attacker_entity.kind, target_entity.kind) {
        return false;
    }
    let targetable = if legality == DirectFireLegality::IntendedTarget {
        crate::game::services::world_query::is_explicit_attack_targetable(
            target_entity,
            teams,
            attacker_owner,
            attacker,
        )
    } else {
        !target_entity.is_neutral_obstacle()
            && crate::game::services::world_query::is_enemy_targetable(
                target_entity,
                teams,
                attacker_owner,
                attacker,
            )
    };
    if !targetable {
        return false;
    }
    let end = (target_entity.pos_x, target_entity.pos_y);
    let smoke_melee_visibility = smokes.units_have_melee_visibility(attacker_entity, target_entity);
    if (smokes.point_inside(start.0, start.1) || smokes.point_inside(end.0, end.1))
        && !smoke_melee_visibility
    {
        return false;
    }
    let visible = smoke_melee_visibility
        || crate::rules::projection::team_visible_world(attacker_owner, end.0, end.1, fog, teams);
    let clear_los = if smoke_melee_visibility {
        LineOfSight::new(map).clear_between_world_points(start, end)
    } else {
        los.clear_between_world_points(start, end)
    };
    if !visible || !clear_los {
        return false;
    }
    if legality == DirectFireLegality::IntendedTarget {
        shot_hits_intended_target(
            map,
            entities,
            blockers,
            teams,
            attacker,
            attacker_owner,
            target,
            start,
        )
    } else {
        !friendly_hard_blocker_between(
            map,
            entities,
            blockers,
            attacker,
            attacker_owner,
            start,
            end,
        )
    }
}

pub(super) struct AutoTargetLegality {
    pub(super) distance_sq: f32,
    pub(super) in_weapon_range: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn auto_target_legality(
    map: &Map,
    entities: &EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
    los: &LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
    weapon_range_px: f32,
    target: &Entity,
) -> Option<AutoTargetLegality> {
    if target.is_neutral_obstacle()
        || !crate::game::services::world_query::is_enemy_targetable(target, teams, owner, self_id)
    {
        return None;
    }
    let concealment = terrain::concealment_modifier(target.kind, TerrainKind::Open).max(0.0);
    let effective_acquire_px = acquire_px * concealment;
    let effective_weapon_range_px = weapon_range_px * concealment;
    let dx = target.pos_x - px;
    let dy = target.pos_y - py;
    let distance_sq = dx * dx + dy * dy;
    if !distance_sq.is_finite()
        || !effective_acquire_px.is_finite()
        || distance_sq > effective_acquire_px * effective_acquire_px
        || !target_has_legal_shot(
            map, entities, blockers, teams, los, fog, smokes, self_id, owner, px, py, target,
        )
    {
        return None;
    }
    Some(AutoTargetLegality {
        distance_sq,
        in_weapon_range: effective_weapon_range_px.is_finite()
            && distance_sq <= effective_weapon_range_px * effective_weapon_range_px,
    })
}

#[allow(clippy::too_many_arguments)]
fn target_has_legal_shot(
    map: &Map,
    entities: &EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
    los: &LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    target: &Entity,
) -> bool {
    let smoke_melee_visibility = entities
        .get(self_id)
        .is_some_and(|attacker| smokes.units_have_melee_visibility(attacker, target));
    let visible = smoke_melee_visibility
        || (!smokes.point_inside(px, py)
            && !smokes.point_inside(target.pos_x, target.pos_y)
            && crate::rules::projection::team_visible_world(
                owner,
                target.pos_x,
                target.pos_y,
                fog,
                teams,
            ));
    visible
        && (entities
            .get(self_id)
            .is_some_and(|entity| entity.kind == EntityKind::MortarTeam)
            || direct_fire_target_legal(
                map,
                entities,
                blockers,
                teams,
                los,
                fog,
                smokes,
                self_id,
                owner,
                (px, py),
                target.id,
                DirectFireLegality::AutoAcquire,
            ))
}
