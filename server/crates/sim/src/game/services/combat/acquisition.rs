use crate::config;
use crate::game::entity::{
    movement_body_class, Entity, EntityKind, EntityStore, MovementBodyClass, Order,
};
use crate::game::entrenchment_combat;
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::line_of_sight::LineOfSight;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::world_query;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::rules::combat as combat_rules;
use crate::rules::target as target_rules;
use crate::rules::terrain::{self, TerrainKind};

use super::priority::{AttackPriorityContext, TargetCandidate};
use super::projection::{friendly_hard_blocker_between, shot_hits_intended_target};
use super::weapons::{
    choose_target_preferring_anti_tank_field, effective_attack_profile,
    moving_fire_move_order_holds_path,
};

#[derive(Copy, Clone, PartialEq)]
pub(super) enum CombatMode {
    Ordered,
    Aggressive,
    Opportunistic,
    Passive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DirectFireVisibility {
    Owner,
    Team,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DirectFireLegality {
    visibility: DirectFireVisibility,
    requires_intended_target: bool,
}

impl DirectFireLegality {
    pub(super) fn auto_acquire() -> Self {
        Self {
            visibility: DirectFireVisibility::Owner,
            requires_intended_target: false,
        }
    }

    pub(super) fn intended_target(visibility: DirectFireVisibility) -> Self {
        Self {
            visibility,
            requires_intended_target: true,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn direct_fire_target_legal(
    map: &Map,
    entities: &EntityStore,
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
    let Some(target_entity) = entities.get(target) else {
        return false;
    };
    let targetable = if legality.requires_intended_target {
        world_query::is_explicit_attack_targetable(target_entity, teams, attacker_owner, attacker)
    } else {
        world_query::is_enemy_targetable(target_entity, teams, attacker_owner, attacker)
    };
    if !targetable {
        return false;
    }
    let end = (target_entity.pos_x, target_entity.pos_y);
    if smokes.point_inside(start.0, start.1) || smokes.point_inside(end.0, end.1) {
        return false;
    }
    let visible = match legality.visibility {
        DirectFireVisibility::Owner => fog.is_visible_world(attacker_owner, end.0, end.1),
        DirectFireVisibility::Team => {
            crate::rules::projection::team_visible_world(attacker_owner, end.0, end.1, fog, teams)
        }
    };
    if !visible || !los.clear_between_world_points(start, end) {
        return false;
    }
    if legality.requires_intended_target {
        shot_hits_intended_target(
            map,
            entities,
            teams,
            attacker,
            attacker_owner,
            target,
            start,
        )
    } else {
        !friendly_hard_blocker_between(map, entities, attacker, attacker_owner, start, end)
    }
}

pub(super) fn combat_mode_with_moving_fire(e: &Entity, can_fire_while_moving: bool) -> CombatMode {
    match e.order() {
        Order::Attack(_) => CombatMode::Ordered,
        Order::HoldPosition => CombatMode::Opportunistic,
        Order::Idle if entrenchment_combat::is_actively_entrenched(e) => CombatMode::Opportunistic,
        Order::AttackMove(_) => CombatMode::Aggressive,
        Order::Move(_) if moving_fire_move_order_holds_path(e, can_fire_while_moving) => {
            CombatMode::Opportunistic
        }
        Order::Idle if e.is_building() => CombatMode::Aggressive,
        Order::Idle if e.is_unit() && !is_passive_idle_unit(e.kind) => CombatMode::Aggressive,
        _ => CombatMode::Passive,
    }
}

#[cfg(test)]
pub(super) fn combat_mode(e: &Entity) -> CombatMode {
    combat_mode_with_moving_fire(e, super::weapons::can_fire_while_moving(e, false))
}

fn is_passive_idle_unit(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::Worker | EntityKind::Golem)
}

/// Resolve which entity an attacker should engage this tick.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_target(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    spatial: &SpatialIndex,
    los: &LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    tank_trap_obstructs_vehicle_route: &dyn Fn(&Entity, &Entity) -> bool,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
    mode: CombatMode,
    attacker_can_fire_while_moving: bool,
    target_filter: &dyn Fn(u32) -> bool,
) -> Option<u32> {
    if smokes.point_inside(px, py) {
        return None;
    }
    // Ordered attackers keep command intent outside the ranker. If the target is
    // still explicitly attackable and visible, the combat system may chase for a
    // fireable shot instead of letting auto-acquisition steal focus.
    if mode == CombatMode::Ordered {
        if let Some(e) = entities.get(self_id) {
            if let Some(target) = e.order().attack_target() {
                if world_query::unit_explicit_attack_target_valid(
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
        }
        // Explicit target gone → fall through to acquisition so we don't stand idle.
    }

    if matches!(mode, CombatMode::Passive) {
        return None;
    }

    let attacker = entities.get(self_id)?;
    let context = AttackPriorityContext {
        attacker_is_unit: attacker.is_unit(),
        attacker_weapon_class: if attacker.kind == EntityKind::Panzerfaust {
            crate::rules::defs::WeaponClass::AntiTank
        } else {
            combat_rules::weapon_class(attacker.kind)
        },
        policy_id: combat_rules::default_target_priority_policy(attacker.kind),
        can_retain_moving_target: attacker_can_fire_while_moving,
    };
    let attacker_is_vehicle_body =
        movement_body_class(attacker.kind) == MovementBodyClass::VehicleBody;
    let weapon_range_px = weapon_range_px(attacker);
    let candidates = legal_target_candidates(
        map,
        entities,
        teams,
        spatial,
        los,
        fog,
        smokes,
        self_id,
        owner,
        px,
        py,
        acquire_px,
        weapon_range_px,
        &context,
        attacker.kind,
        attacker_is_vehicle_body,
        tank_trap_obstructs_vehicle_route,
        attacker.target_id(),
    );
    if mode_requires_currently_fireable_targets(mode)
        || aggressive_auto_acquisition_prefers_currently_fireable_targets(mode)
    {
        let fireable_target = choose_target_preferring_anti_tank_field(
            &context,
            attacker,
            px,
            py,
            &candidates,
            |candidate| candidate.in_weapon_range && target_filter(candidate.id),
        );
        if mode_requires_currently_fireable_targets(mode) {
            return fireable_target;
        }
        if fireable_target.is_some() {
            return fireable_target;
        }
    }
    choose_target_preferring_anti_tank_field(&context, attacker, px, py, &candidates, |candidate| {
        target_filter(candidate.id)
    })
}

fn mode_requires_currently_fireable_targets(mode: CombatMode) -> bool {
    mode == CombatMode::Opportunistic
}

fn aggressive_auto_acquisition_prefers_currently_fireable_targets(mode: CombatMode) -> bool {
    mode == CombatMode::Aggressive
}

fn target_relevant_for_auto_acquisition(attacker: EntityKind, target: &Entity) -> bool {
    !(movement_body_class(attacker) == MovementBodyClass::InfantryLike
        && target.kind == EntityKind::TankTrap)
}

fn weapon_range_px(attacker: &Entity) -> f32 {
    if attacker.kind == EntityKind::Panzerfaust {
        return entrenchment_combat::attack_range_tiles(
            attacker,
            config::PANZERFAUST_RANGE_TILES as f32,
        ) * config::TILE_SIZE as f32
            + attacker.radius()
            + super::RANGE_SLACK;
    }
    let profile = effective_attack_profile(attacker);
    profile.range_tiles * config::TILE_SIZE as f32 + attacker.radius() + super::RANGE_SLACK
}

#[allow(clippy::too_many_arguments)]
fn legal_target_candidates(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    spatial: &SpatialIndex,
    los: &LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
    weapon_range_px: f32,
    context: &AttackPriorityContext,
    attacker_kind: EntityKind,
    attacker_is_vehicle_body: bool,
    tank_trap_obstructs_vehicle_route: &dyn Fn(&Entity, &Entity) -> bool,
    retained_target_id: Option<u32>,
) -> Vec<TargetCandidate> {
    let mut candidates = Vec::new();
    let attacker = entities.get(self_id);
    for id in spatial.ids_in_circle_bbox(px, py, acquire_px) {
        let Some(target) = entities.get(id) else {
            continue;
        };
        // Retained target status is only a ranker fact. It must still pass the
        // same hostile, visible, smoke, LOS, and blocker checks as any other
        // auto-acquired candidate.
        let retained_target = retained_target_id == Some(id);
        let retained_moving_fire_target = context.can_retain_moving_target && retained_target;
        if !world_query::is_enemy_targetable(target, teams, owner, self_id) {
            continue;
        }
        if !retained_moving_fire_target
            && !target_relevant_for_auto_acquisition(attacker_kind, target)
        {
            continue;
        }
        let concealment = terrain::concealment_modifier(target.kind, TerrainKind::Open).max(0.0);
        let effective_acquire_px = acquire_px * concealment;
        if !effective_acquire_px.is_finite() {
            continue;
        }
        let dx = target.pos_x - px;
        let dy = target.pos_y - py;
        let distance_sq = dx * dx + dy * dy;
        if distance_sq > effective_acquire_px * effective_acquire_px {
            continue;
        }
        let effective_weapon_range_px = weapon_range_px * concealment;
        let in_weapon_range = effective_weapon_range_px.is_finite()
            && distance_sq <= effective_weapon_range_px * effective_weapon_range_px;
        if !target_has_legal_shot(
            map, entities, teams, los, fog, smokes, self_id, owner, px, py, target,
        ) {
            continue;
        }
        let tank_trap_obstructs_vehicle_route =
            if target.kind == EntityKind::TankTrap && attacker_is_vehicle_body {
                attacker
                    .map(|attacker| tank_trap_obstructs_vehicle_route(attacker, target))
                    .unwrap_or(false)
            } else {
                false
            };
        candidates.push(TargetCandidate {
            id,
            owner: target.owner,
            pos_x: target.pos_x,
            pos_y: target.pos_y,
            distance_sq,
            facts: target_rules::target_facts(target.kind),
            in_weapon_range,
            tank_trap_obstructs_vehicle_route,
            retained_target,
        });
    }
    candidates
}

#[allow(clippy::too_many_arguments)]
fn target_has_legal_shot(
    map: &Map,
    entities: &EntityStore,
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
    !smokes.point_inside(px, py)
        && !smokes.point_inside(target.pos_x, target.pos_y)
        && target_visible_to_owner(fog, smokes, owner, target)
        && (attacker_uses_indirect_fire(entities, self_id)
            || direct_fire_target_legal(
                map,
                entities,
                teams,
                los,
                fog,
                smokes,
                self_id,
                owner,
                (px, py),
                target.id,
                DirectFireLegality::auto_acquire(),
            ))
}

fn attacker_uses_indirect_fire(entities: &EntityStore, self_id: u32) -> bool {
    matches!(
        entities.get(self_id).map(|e| e.kind),
        Some(EntityKind::MortarTeam)
    )
}

fn target_visible_to_owner(
    fog: &Fog,
    smokes: &SmokeCloudStore,
    owner: u32,
    target: &Entity,
) -> bool {
    fog.is_visible_world(owner, target.pos_x, target.pos_y)
        && !smokes.point_inside(target.pos_x, target.pos_y)
}
