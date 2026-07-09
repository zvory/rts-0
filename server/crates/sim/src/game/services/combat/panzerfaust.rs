use crate::config;
use crate::game::entity::{AttackPhase, Entity, EntityKind, EntityStore, Order, PanzerfaustState};
use crate::game::entrenchment_combat;
use crate::game::services::world_query;

use super::acquisition::{
    combat_mode_with_moving_fire, direct_fire_target_legal,
    resolve_target as resolve_target_with_obstruction, CombatMode, DirectFireLegality,
    DirectFireVisibility,
};
use super::chase::{chase_goal_for_target, chase_path_needs_refresh};
use super::weapons::mirror_weapon_to_body;
use super::{
    dist2, Fog, LineOfSight, Map, MoveCoordinator, Occupancy, SmokeCloudStore, SpatialIndex,
    StaticPathingRelation, TeamRelations, RANGE_SLACK,
};

mod events;
mod runtime;

pub(super) use runtime::tick_states;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_combat_if_panzerfaust(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    methamphetamines_researched: &dyn Fn(u32) -> bool,
    occ: &Occupancy,
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    id: u32,
) -> bool {
    if !matches!(
        entities.get(id).map(|entity| entity.kind),
        Some(EntityKind::Panzerfaust)
    ) {
        return false;
    }
    handle_loaded_combat(
        map,
        entities,
        teams,
        methamphetamines_researched,
        occ,
        spatial,
        coordinator,
        fog,
        smokes,
        id,
    );
    true
}

#[allow(clippy::too_many_arguments)]
fn handle_loaded_combat(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    methamphetamines_researched: &dyn Fn(u32) -> bool,
    occ: &Occupancy,
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    id: u32,
) {
    if entities.get(id).and_then(panzerfaust_state) != Some(PanzerfaustState::Loaded) {
        return;
    }
    let Some((owner, px, py, range_px, acquire_px, mode)) =
        panzerfaust_combat_context(entities, id)
    else {
        return;
    };
    if mode == CombatMode::Passive {
        return;
    }

    let los = LineOfSight::with_smoke(map, smokes);
    let target = resolve_panzerfaust_target(
        map, entities, teams, occ, spatial, &los, fog, smokes, id, owner, px, py, acquire_px, mode,
    );
    let Some(target) = target else {
        handle_no_target(entities, coordinator, id, mode);
        return;
    };
    let Some((tx, ty, target_owner)) = entities
        .get(target)
        .map(|target| (target.pos_x, target.pos_y, target.owner))
    else {
        return;
    };
    if !(teams.is_enemy_owner(owner, target_owner)
        || mode == CombatMode::Ordered && target_owner == owner)
    {
        return;
    }

    let distance = dist2(px, py, tx, ty).sqrt();
    let target_angle = (ty - py).atan2(tx - px);
    let fire_context = PanzerfaustFireContext::new(map, entities, teams, &los, fog, smokes);
    let clear_shot = panzerfaust_target_fireable(&fire_context, id, owner, target);
    if distance <= range_px && clear_shot {
        if let Some(attacker) = entities.get_mut(id) {
            if target_angle.is_finite() {
                attacker.set_facing(target_angle);
                mirror_weapon_to_body(attacker, target_angle);
            }
            attacker.set_target_id(Some(target));
            attacker.mark_attack_phase(AttackPhase::Firing);
            attacker.clear_path();
            set_panzerfaust_state(
                attacker,
                PanzerfaustState::Windup {
                    target,
                    ticks_remaining: windup_ticks(methamphetamines_researched(owner)),
                },
            );
        }
        return;
    }

    if mode != CombatMode::Opportunistic {
        let chase_goal =
            chase_goal_for_target(map, entities, id, (px, py), (tx, ty), range_px, distance);
        let chase_goal = coordinator.attack_chase_goal(entities, id, target, chase_goal, range_px);
        let want_repath = entities
            .get(id)
            .map(|e| chase_path_needs_refresh(e, chase_goal))
            .unwrap_or(false);
        if let Some(attacker) = entities.get_mut(id) {
            if target_angle.is_finite() {
                attacker.set_facing(target_angle);
                mirror_weapon_to_body(attacker, target_angle);
            }
            attacker.set_target_id(Some(target));
            attacker.mark_attack_phase(AttackPhase::Chasing);
        }
        if want_repath {
            coordinator.request_chase_path(entities, id, chase_goal);
        }
    }
}

fn panzerfaust_combat_context(
    entities: &EntityStore,
    id: u32,
) -> Option<(u32, f32, f32, f32, f32, CombatMode)> {
    let attacker = entities.get(id)?;
    if attacker.hp == 0 || attacker.kind != EntityKind::Panzerfaust || !attacker.can_attack() {
        return None;
    }
    let range_px = panzerfaust_range_tiles(attacker) * config::TILE_SIZE as f32
        + attacker.radius()
        + RANGE_SLACK;
    let aggro_px = if matches!(attacker.order(), Order::HoldPosition) {
        range_px
    } else {
        (attacker.sight_tiles() as f32 * config::TILE_SIZE as f32).max(range_px)
    };
    let mode = combat_mode_with_moving_fire(attacker, false);
    let acquire_px = if mode == CombatMode::Opportunistic {
        range_px
    } else {
        aggro_px
    };
    Some((
        attacker.owner,
        attacker.pos_x,
        attacker.pos_y,
        range_px,
        acquire_px,
        mode,
    ))
}

#[allow(clippy::too_many_arguments)]
fn resolve_panzerfaust_target(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    occ: &Occupancy,
    spatial: &SpatialIndex,
    los: &LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
    mode: CombatMode,
) -> Option<u32> {
    if mode == CombatMode::Ordered {
        return entities
            .get(id)
            .and_then(|attacker| attacker.order().attack_target())
            .filter(|target| {
                panzerfaust_target_valid(entities, teams, fog, smokes, owner, id, *target)
            });
    }
    let tank_trap_relation = StaticPathingRelation::for_player(owner, teams);
    let tank_trap_obstructs_vehicle_route = |attacker: &Entity, target: &Entity| {
        occ.tank_trap_obstructs_vehicle_route(attacker, target, &tank_trap_relation)
    };
    resolve_target_with_obstruction(
        map,
        entities,
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
        false,
        &|target_id| panzerfaust_target_valid(entities, teams, fog, smokes, owner, id, target_id),
    )
}

fn handle_no_target(
    entities: &mut EntityStore,
    coordinator: &mut MoveCoordinator<'_>,
    id: u32,
    mode: CombatMode,
) {
    if let Some(attacker) = entities.get_mut(id) {
        if matches!(
            attacker.order(),
            Order::Attack(_) | Order::AttackMove(_) | Order::Idle | Order::HoldPosition
        ) {
            attacker.set_target_id(None);
        }
    }
    if mode != CombatMode::Aggressive {
        return;
    }
    let Some(goal) = entities.get(id).and_then(|e| e.move_intent()) else {
        return;
    };
    let needs_resume = entities
        .get(id)
        .map(|e| {
            let stale_goal = e.path_goal().is_none_or(|path_goal| {
                (path_goal.0 - goal.0).abs() > f32::EPSILON
                    || (path_goal.1 - goal.1).abs() > f32::EPSILON
            });
            let interrupted_before_arrival = e.path_is_empty()
                && e.move_phase() != Some(crate::game::entity::MovePhase::Arrived);
            stale_goal || interrupted_before_arrival
        })
        .unwrap_or(true);
    if needs_resume {
        coordinator.request_chase_path(entities, id, goal);
    }
}

fn panzerfaust_target_valid(
    entities: &EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    owner: u32,
    attacker: u32,
    target: u32,
) -> bool {
    world_query::unit_explicit_attack_target_valid(
        entities,
        teams,
        fog,
        Some(smokes),
        owner,
        attacker,
        target,
    )
}

fn panzerfaust_state(entity: &Entity) -> Option<PanzerfaustState> {
    entity.combat.as_ref().and_then(|combat| combat.panzerfaust)
}

fn set_panzerfaust_state(entity: &mut Entity, state: PanzerfaustState) {
    if let Some(combat) = entity.combat.as_mut() {
        combat.panzerfaust = Some(state);
    }
}

fn panzerfaust_range_tiles(attacker: &Entity) -> f32 {
    entrenchment_combat::attack_range_tiles(attacker, config::PANZERFAUST_RANGE_TILES as f32)
}

fn panzerfaust_target_in_range(
    map: &Map,
    entities: &EntityStore,
    attacker_id: u32,
    target_id: u32,
) -> bool {
    let Some(attacker) = entities.get(attacker_id) else {
        return false;
    };
    let Some(target) = entities.get(target_id) else {
        return false;
    };
    let range_px = panzerfaust_range_tiles(attacker) * config::TILE_SIZE as f32
        + attacker.radius()
        + RANGE_SLACK;
    if dist2(attacker.pos_x, attacker.pos_y, target.pos_x, target.pos_y) > range_px * range_px {
        return false;
    }
    target.pos_x >= 0.0
        && target.pos_y >= 0.0
        && target.pos_x < map.world_size_px()
        && target.pos_y < map.world_size_px()
}

pub(super) struct PanzerfaustFireContext<'a, 'los> {
    map: &'a Map,
    entities: &'a EntityStore,
    teams: &'a TeamRelations,
    los: &'a LineOfSight<'los>,
    fog: &'a Fog,
    smokes: &'a SmokeCloudStore,
}

impl<'a, 'los> PanzerfaustFireContext<'a, 'los> {
    fn new(
        map: &'a Map,
        entities: &'a EntityStore,
        teams: &'a TeamRelations,
        los: &'a LineOfSight<'los>,
        fog: &'a Fog,
        smokes: &'a SmokeCloudStore,
    ) -> Self {
        Self {
            map,
            entities,
            teams,
            los,
            fog,
            smokes,
        }
    }
}

fn panzerfaust_target_fireable(
    context: &PanzerfaustFireContext<'_, '_>,
    attacker_id: u32,
    owner: u32,
    target_id: u32,
) -> bool {
    let Some(attacker) = context.entities.get(attacker_id) else {
        return false;
    };
    direct_fire_target_legal(
        context.map,
        context.entities,
        context.teams,
        context.los,
        context.fog,
        context.smokes,
        attacker_id,
        owner,
        (attacker.pos_x, attacker.pos_y),
        target_id,
        DirectFireLegality::intended_target(DirectFireVisibility::Team),
    )
}

fn windup_ticks(has_methamphetamines: bool) -> u16 {
    if has_methamphetamines {
        config::METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS
    } else {
        config::PANZERFAUST_WINDUP_TICKS
    }
}

fn recovery_ticks(has_methamphetamines: bool) -> u16 {
    if has_methamphetamines {
        config::METHAMPHETAMINES_PANZERFAUST_RECOVERY_TICKS
    } else {
        config::PANZERFAUST_RECOVERY_TICKS
    }
}
