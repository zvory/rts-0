use crate::game::entity::{Entity, EntityKind, EntityStore, MovePhase, Order, WeaponSetup};
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

use super::priority::{AttackPriorityContext, TargetCandidate};
use super::projection::max_building_combat_extent_px;
use super::shot_blocker_index::ShotBlockerIndex;
use super::target_legality::{auto_target_candidate, auto_target_has_legal_shot};
use super::weapons::{
    auto_retention_target_inside_field_of_fire, effective_attack_profile,
    moving_fire_move_order_holds_path,
};

#[derive(Copy, Clone, PartialEq)]
pub(super) enum CombatMode {
    Ordered,
    Aggressive,
    Opportunistic,
    Passive,
}

pub(super) fn combat_mode_with_moving_fire(e: &Entity, can_fire_while_moving: bool) -> CombatMode {
    match e.order() {
        Order::Attack(_) => CombatMode::Ordered,
        Order::HoldPosition => CombatMode::Opportunistic,
        Order::AttackMove(_)
            if entrenchment_combat::is_actively_entrenched(e)
                && e.move_phase() == Some(MovePhase::Arrived) =>
        {
            CombatMode::Opportunistic
        }
        Order::Idle if idle_unit_holds_position(e) => CombatMode::Opportunistic,
        Order::AttackMove(_) => CombatMode::Aggressive,
        Order::Move(_) if moving_fire_move_order_holds_path(e, can_fire_while_moving) => {
            CombatMode::Opportunistic
        }
        Order::Idle if e.is_building() => CombatMode::Aggressive,
        Order::Idle if e.is_unit() && !is_passive_idle_unit(e.kind) => CombatMode::Aggressive,
        _ => CombatMode::Passive,
    }
}

fn idle_unit_holds_position(e: &Entity) -> bool {
    entrenchment_combat::is_actively_entrenched(e)
        || (e.kind == EntityKind::MachineGunner
            && matches!(
                e.weapon_setup(),
                WeaponSetup::SettingUp { .. } | WeaponSetup::Deployed
            ))
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
    blockers: &ShotBlockerIndex,
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
    mode: CombatMode,
    attacker_can_fire_while_moving: bool,
    target_filter: &dyn Fn(u32) -> bool,
) -> Option<u32> {
    let attacker = entities.get(self_id)?;
    let profile = effective_attack_profile(attacker);
    let weapon_range_px = profile.range_tiles * crate::config::TILE_SIZE as f32
        + if attacker.kind == EntityKind::MortarTeam {
            0.0
        } else {
            attacker.radius() + super::RANGE_SLACK
        };
    resolve_target_for_weapon(
        map,
        entities,
        blockers,
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
        mode,
        attacker_can_fire_while_moving,
        profile
            .weapon
            .map(|weapon| weapon.weapon_class)
            .unwrap_or(crate::rules::defs::WeaponClass::None),
        weapon_range_px,
        target_filter,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_target_for_weapon(
    map: &Map,
    entities: &EntityStore,
    blockers: &ShotBlockerIndex,
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
    mode: CombatMode,
    attacker_can_fire_while_moving: bool,
    attacker_weapon_class: crate::rules::defs::WeaponClass,
    weapon_range_px: f32,
    target_filter: &dyn Fn(u32) -> bool,
) -> Option<u32> {
    // Ordered attackers keep command intent outside the ranker. If the target is still
    // explicitly attackable and visible, retain it without letting auto-acquisition steal focus.
    if mode == CombatMode::Ordered {
        if let Some(e) = entities.get(self_id) {
            if let Some(target) = e.order().attack_target() {
                if world_query::unit_explicit_attack_target_valid(
                    world_query::ExplicitAttackQuery {
                        map,
                        entities,
                        teams,
                        fog,
                        smokes: Some(smokes),
                        attacker_owner: owner,
                    },
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
        attacker_weapon_class,
        policy_id: combat_rules::default_target_priority_policy(attacker.kind),
        can_retain_moving_target: attacker_can_fire_while_moving,
    };
    let candidates = target_candidates(
        map,
        entities,
        teams,
        spatial,
        self_id,
        owner,
        px,
        py,
        acquire_px,
        weapon_range_px,
        attacker.target_id(),
    );
    for candidate in super::priority::ranked_candidates(
        &context,
        candidates,
        mode_requires_currently_fireable_targets(mode),
        aggressive_auto_acquisition_prefers_currently_fireable_targets(mode),
    ) {
        if !target_filter(candidate.id)
            || !auto_retention_target_inside_field_of_fire(
                attacker,
                (candidate.pos_y - py).atan2(candidate.pos_x - px),
            )
        {
            continue;
        }
        let Some(target) = entities.get(candidate.id) else {
            continue;
        };
        if auto_target_has_legal_shot(
            map,
            entities,
            blockers,
            teams,
            los,
            fog,
            smokes,
            attacker,
            owner,
            (px, py),
            target,
        ) {
            return Some(candidate.id);
        }
    }
    None
}

fn mode_requires_currently_fireable_targets(mode: CombatMode) -> bool {
    mode == CombatMode::Opportunistic
}

fn aggressive_auto_acquisition_prefers_currently_fireable_targets(mode: CombatMode) -> bool {
    mode == CombatMode::Aggressive
}

#[allow(clippy::too_many_arguments)]
fn target_candidates(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    spatial: &SpatialIndex,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    acquire_px: f32,
    weapon_range_px: f32,
    retained_target_id: Option<u32>,
) -> Vec<TargetCandidate> {
    let mut candidates = Vec::new();
    let Some(attacker) = entities.get(self_id) else {
        return candidates;
    };
    let query_radius = acquire_px + max_building_combat_extent_px();
    for id in spatial.ids_in_circle_bbox(px, py, query_radius) {
        let Some(target) = entities.get(id) else {
            continue;
        };
        let retained_target = retained_target_id == Some(id);
        let Some(legality) = auto_target_candidate(
            map,
            teams,
            attacker,
            owner,
            (px, py),
            acquire_px,
            weapon_range_px,
            target,
        ) else {
            continue;
        };
        candidates.push(TargetCandidate {
            id,
            owner: target.owner,
            pos_x: target.pos_x,
            pos_y: target.pos_y,
            distance_sq: legality.distance_sq,
            facts: target_rules::target_facts(target.kind),
            in_weapon_range: legality.in_weapon_range,
            retained_target,
        });
    }
    candidates
}
