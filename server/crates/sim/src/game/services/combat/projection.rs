use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::services::geometry::{
    building_rect_for_entity, segment_intersects_rect, segment_intersects_unit_body,
    unit_body_for_entity,
};
use crate::game::teams::TeamRelations;

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_shot_victim(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    attacker: u32,
    intended_victim: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
) -> Option<u32> {
    let victim = entities.get(intended_victim)?;
    let end = (victim.pos_x, victim.pos_y);
    if !ax.is_finite() || !ay.is_finite() || !end.0.is_finite() || !end.1.is_finite() {
        return Some(intended_victim);
    }

    let mut best = (intended_victim, 1.0f32);
    for candidate in entities.iter() {
        if candidate.id == attacker
            || candidate.is_node()
            || !teams.is_enemy_owner(attacker_owner, candidate.owner)
            || candidate.hp == 0
        {
            continue;
        }
        let Some(hit_t) = shot_blocker_intersection(map, candidate, (ax, ay), end) else {
            continue;
        };
        if hit_t <= best.1 + f32::EPSILON
            && (hit_t < best.1 - f32::EPSILON || candidate.id < best.0)
        {
            best = (candidate.id, hit_t);
        }
    }
    Some(best.0)
}

pub(super) fn friendly_hard_blocker_between(
    map: &Map,
    entities: &EntityStore,
    attacker: u32,
    attacker_owner: u32,
    start: (f32, f32),
    end: (f32, f32),
) -> bool {
    if !start.0.is_finite() || !start.1.is_finite() || !end.0.is_finite() || !end.1.is_finite() {
        return true;
    }
    entities.iter().any(|candidate| {
        candidate.id != attacker
            && candidate.owner == attacker_owner
            && candidate.hp > 0
            && shot_blocker_intersection(map, candidate, start, end).is_some()
    })
}

pub(super) fn shot_hits_intended_target(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    attacker: u32,
    attacker_owner: u32,
    intended_victim: u32,
    start: (f32, f32),
) -> bool {
    let Some(target) = entities.get(intended_victim) else {
        return false;
    };
    let end = (target.pos_x, target.pos_y);
    if friendly_hard_blocker_between(map, entities, attacker, attacker_owner, start, end) {
        return false;
    }
    resolve_shot_victim(
        map,
        entities,
        teams,
        attacker,
        intended_victim,
        attacker_owner,
        start.0,
        start.1,
    )
    .is_some_and(|victim| victim == intended_victim)
}

pub(super) fn shot_blocker_intersection(
    map: &Map,
    entity: &Entity,
    start: (f32, f32),
    end: (f32, f32),
) -> Option<f32> {
    if entity.kind == EntityKind::Tank {
        return unit_body_for_entity(entity)
            .and_then(|body| segment_intersects_unit_body(start, end, body));
    }
    if entity.is_building() && entity.kind != EntityKind::TankTrap {
        return building_rect_for_entity(map, entity)
            .and_then(|rect| segment_intersects_rect(start, end, rect));
    }
    None
}
