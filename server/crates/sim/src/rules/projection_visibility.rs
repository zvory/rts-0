//! Shared fog and smoke predicates used by entity and event projection.

use crate::game::entity::{Entity, EntityKind};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;

use super::projection::PrivateDetailProjection;

impl PrivateDetailProjection<'_> {
    pub(super) fn has_concealment_detection(
        self,
        viewer: u32,
        entity_id: u32,
        fog: &Fog,
        teams: &TeamRelations,
    ) -> bool {
        match self {
            Self::ExactViewer => teams
                .same_team_player_ids(viewer)
                .into_iter()
                .any(|player| fog.has_concealment_detection(player, entity_id)),
            Self::SelectedOwners(player_ids) => player_ids
                .iter()
                .any(|player| fog.has_concealment_detection(*player, entity_id)),
            Self::AllProjected => true,
        }
    }

    pub(super) fn entity_hidden_by_concealment(
        self,
        viewer: u32,
        entity: &Entity,
        map: &Map,
        fog: &Fog,
        teams: &TeamRelations,
    ) -> bool {
        match self {
            Self::ExactViewer => {
                entity_hidden_by_concealment_from_team(viewer, entity, map, fog, teams)
            }
            Self::SelectedOwners(player_ids) => entity_hidden_by_concealment_from_players(
                player_ids.iter().copied(),
                entity,
                map,
                fog,
                teams,
            ),
            Self::AllProjected => false,
        }
    }
}

/// Environmental concealment hides enemy units even when their ground tile is otherwise visible.
/// An active firing reveal makes only that firing entity actionable again.
pub(crate) fn entity_hidden_by_concealment_from_team(
    viewer: u32,
    entity: &Entity,
    map: &Map,
    fog: &Fog,
    teams: &TeamRelations,
) -> bool {
    entity_hidden_by_concealment_from_players([viewer], entity, map, fog, teams)
}

pub(crate) fn entity_hidden_by_concealment_from_players(
    viewers: impl IntoIterator<Item = u32>,
    entity: &Entity,
    map: &Map,
    fog: &Fog,
    teams: &TeamRelations,
) -> bool {
    if !entity.is_unit() || !map.world_point_is_concealed(entity.pos_x, entity.pos_y) {
        return false;
    }
    !viewers.into_iter().any(|viewer| {
        teams.same_team_or_same_owner(viewer, entity.owner)
            || teams
                .same_team_player_ids(viewer)
                .into_iter()
                .any(|player| {
                    fog.has_concealment_detection(player, entity.id)
                        || fog
                            .active_firing_reveal_episode(player, entity.id)
                            .is_some()
                })
    })
}

pub fn event_visible_to(
    viewer: u32,
    event_origin_x: f32,
    event_origin_y: f32,
    attacker_owner: u32,
    fog: &Fog,
) -> bool {
    viewer == attacker_owner || fog.is_visible_world(viewer, event_origin_x, event_origin_y)
}

pub fn team_visible_world(viewer: u32, x: f32, y: f32, fog: &Fog, teams: &TeamRelations) -> bool {
    teams
        .same_team_player_ids(viewer)
        .into_iter()
        .any(|player_id| fog.is_visible_world(player_id, x, y))
}

pub fn event_visible_to_team(
    viewer: u32,
    event_origin_x: f32,
    event_origin_y: f32,
    owner: u32,
    fog: &Fog,
    teams: &TeamRelations,
) -> bool {
    teams.same_team_or_same_owner(viewer, owner)
        || team_visible_world(viewer, event_origin_x, event_origin_y, fog, teams)
}

pub fn event_visible_to_team_with_smoke(
    viewer: u32,
    event_origin_x: f32,
    event_origin_y: f32,
    owner: u32,
    fog: &Fog,
    teams: &TeamRelations,
    smokes: &SmokeCloudStore,
) -> bool {
    if !teams.same_team_or_same_owner(viewer, owner)
        && smokes.point_inside(event_origin_x, event_origin_y)
    {
        return false;
    }
    event_visible_to_team(viewer, event_origin_x, event_origin_y, owner, fog, teams)
}

#[allow(clippy::too_many_arguments)]
pub fn attack_event_visible_to_team(
    viewer: u32,
    attacker_x: f32,
    attacker_y: f32,
    target_x: f32,
    target_y: f32,
    attacker_owner: u32,
    fog: &Fog,
    teams: &TeamRelations,
) -> bool {
    event_visible_to_team(viewer, attacker_x, attacker_y, attacker_owner, fog, teams)
        || team_visible_world(viewer, target_x, target_y, fog, teams)
}

/// Whether a direct shot exposes its attacker through transient and actionable firing reveals.
pub(crate) fn shot_reveals_attacker(victim_kind: EntityKind) -> bool {
    victim_kind != EntityKind::TankTrap
}

pub fn event_visible_to_with_smoke(
    viewer: u32,
    event_origin_x: f32,
    event_origin_y: f32,
    attacker_owner: u32,
    fog: &Fog,
    smokes: &SmokeCloudStore,
) -> bool {
    if viewer != attacker_owner && smokes.point_inside(event_origin_x, event_origin_y) {
        return false;
    }
    event_visible_to(viewer, event_origin_x, event_origin_y, attacker_owner, fog)
}

#[allow(dead_code)]
pub fn attack_event_visible_to(
    viewer: u32,
    attacker_x: f32,
    attacker_y: f32,
    target_x: f32,
    target_y: f32,
    attacker_owner: u32,
    fog: &Fog,
) -> bool {
    event_visible_to(viewer, attacker_x, attacker_y, attacker_owner, fog)
        || fog.is_visible_world(viewer, target_x, target_y)
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub fn attack_event_visible_to_with_smoke(
    viewer: u32,
    attacker_x: f32,
    attacker_y: f32,
    target_x: f32,
    target_y: f32,
    attacker_owner: u32,
    fog: &Fog,
    smokes: &SmokeCloudStore,
) -> bool {
    if viewer != attacker_owner && smokes.point_inside(attacker_x, attacker_y) {
        return false;
    }
    event_visible_to_with_smoke(viewer, attacker_x, attacker_y, attacker_owner, fog, smokes)
        || (!smokes.point_inside(target_x, target_y)
            && fog.is_visible_world(viewer, target_x, target_y))
}
