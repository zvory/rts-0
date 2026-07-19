use std::collections::HashMap;

use rand::Rng;

use crate::config;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::firing_reveal::{record_firing_reveals_for_victim_team, FiringRevealSource};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::protocol::Event;
use crate::rules::combat as combat_rules;
use crate::rules::target as target_rules;
use crate::rules::terrain::{self, TerrainKind};

use super::activation::{
    secondary_weapon_target_passes_activation, SecondaryWeaponActivationConstraints,
};
use super::damage::apply_damage;
use super::priority::{self, AttackPriorityContext, TargetCandidate};
use super::shot_blocker_index::ShotBlockerIndex;
use super::target_legality::DirectFireLegality;
use super::{FIRING_REVEAL_RESPONSE_DELAY_TICKS, RANGE_SLACK};

const TANK_COAX_HALF_ARC_RAD: f32 = std::f32::consts::PI / 18.0;

#[derive(Clone, Copy)]
struct TankCoaxSnapshot {
    owner: u32,
    pos_x: f32,
    pos_y: f32,
    weapon_facing: f32,
    range_px: f32,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fire_tank_coax_system(
    map: &Map,
    entities: &mut EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
    spatial: &crate::game::services::spatial::SpatialIndex,
    los: &crate::game::services::line_of_sight::LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    rng: &mut impl Rng,
    events: &mut HashMap<u32, Vec<Event>>,
    firing_reveals: &mut Vec<FiringRevealSource>,
    tick: u32,
) {
    let Some(weapon_profile) = combat_rules::weapon_profile(combat_rules::WeaponKind::TankCoax)
    else {
        return;
    };
    for id in entities.ids() {
        let ready =
            matches!(entities.get(id), Some(e) if e.weapon_cooldown(weapon_profile.id) == 0);
        if !ready {
            continue;
        }
        let Some(snapshot) = tank_coax_snapshot(entities, id, weapon_profile) else {
            continue;
        };
        let Some(tid) = resolve_tank_coax_target(
            map,
            entities,
            blockers,
            teams,
            spatial,
            los,
            fog,
            smokes,
            id,
            snapshot,
            weapon_profile,
        ) else {
            continue;
        };
        let (tx, ty) = match entities.get(tid) {
            Some(target) => (target.pos_x, target.pos_y),
            None => continue,
        };
        if let Some(episode) = fog.team_firing_reveal_only_source(snapshot.owner, (tx, ty), teams) {
            let reaction_ready = entities.get_mut(id).is_some_and(|e| {
                e.weapon_firing_reveal_reaction_ready(
                    weapon_profile.id,
                    tid,
                    episode,
                    tick,
                    FIRING_REVEAL_RESPONSE_DELAY_TICKS,
                )
            });
            if !reaction_ready {
                continue;
            }
        }
        let shot_victim_owner = apply_damage(
            map,
            entities,
            blockers,
            teams,
            events,
            fog,
            smokes,
            rng,
            id,
            tid,
            weapon_profile,
            weapon_profile.dmg,
            snapshot.owner,
            snapshot.pos_x,
            snapshot.pos_y,
            tx,
            ty,
            snapshot.range_px,
            0.0,
            tick,
        );
        if let Some(victim_owner) = shot_victim_owner {
            let player_ids = events.keys().copied().collect::<Vec<_>>();
            record_firing_reveals_for_victim_team(
                firing_reveals,
                player_ids,
                fog,
                teams,
                victim_owner,
                snapshot.owner,
                id,
                (snapshot.pos_x, snapshot.pos_y),
                tick,
                weapon_profile.cooldown,
            );
        }
        if let Some(e) = entities.get_mut(id) {
            e.set_weapon_cooldown(weapon_profile.id, weapon_profile.cooldown);
        }
    }
}

fn tank_coax_snapshot(
    entities: &EntityStore,
    id: u32,
    weapon_profile: &combat_rules::WeaponProfile,
) -> Option<TankCoaxSnapshot> {
    let tank = entities.get(id)?;
    if tank.kind != EntityKind::Tank || tank.hp == 0 || !tank.can_attack() {
        return None;
    }
    let weapon_facing = tank.weapon_facing().filter(|facing| facing.is_finite())?;
    let range_px =
        weapon_profile.range_tiles as f32 * config::TILE_SIZE as f32 + tank.radius() + RANGE_SLACK;
    if !range_px.is_finite() {
        return None;
    }
    Some(TankCoaxSnapshot {
        owner: tank.owner,
        pos_x: tank.pos_x,
        pos_y: tank.pos_y,
        weapon_facing,
        range_px,
    })
}

#[allow(clippy::too_many_arguments)]
fn resolve_tank_coax_target(
    map: &Map,
    entities: &EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
    spatial: &crate::game::services::spatial::SpatialIndex,
    los: &crate::game::services::line_of_sight::LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    attacker: u32,
    snapshot: TankCoaxSnapshot,
    weapon_profile: &combat_rules::WeaponProfile,
) -> Option<u32> {
    if smokes.point_inside(snapshot.pos_x, snapshot.pos_y) {
        return None;
    }
    let context = AttackPriorityContext {
        attacker_is_unit: true,
        attacker_weapon_class: weapon_profile.weapon_class,
        policy_id: combat_rules::TargetPriorityPolicyId::TankCoaxMachineGun,
        can_retain_moving_target: false,
    };
    let candidates = tank_coax_target_candidates(
        map, entities, blockers, teams, spatial, los, fog, smokes, attacker, snapshot,
    );
    priority::choose_target(&context, &candidates)
}

#[allow(clippy::too_many_arguments)]
fn tank_coax_target_candidates(
    map: &Map,
    entities: &EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
    spatial: &crate::game::services::spatial::SpatialIndex,
    los: &crate::game::services::line_of_sight::LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    attacker: u32,
    snapshot: TankCoaxSnapshot,
) -> Vec<TargetCandidate> {
    let mut candidates = Vec::new();
    for id in spatial.ids_in_circle_bbox(snapshot.pos_x, snapshot.pos_y, snapshot.range_px) {
        let Some(target) = entities.get(id) else {
            continue;
        };
        if !crate::game::services::world_query::is_enemy_targetable(
            target,
            teams,
            snapshot.owner,
            attacker,
        ) {
            continue;
        }
        let dx = target.pos_x - snapshot.pos_x;
        let dy = target.pos_y - snapshot.pos_y;
        let distance_sq = dx * dx + dy * dy;
        if !distance_sq.is_finite() {
            continue;
        }
        let concealment = terrain::concealment_modifier(target.kind, TerrainKind::Open).max(0.0);
        let effective_weapon_range_px = snapshot.range_px * concealment;
        if !secondary_weapon_target_passes_activation(
            map,
            entities,
            blockers,
            teams,
            los,
            fog,
            smokes,
            attacker,
            snapshot.owner,
            (snapshot.pos_x, snapshot.pos_y),
            target.id,
            SecondaryWeaponActivationConstraints {
                facing_rad: snapshot.weapon_facing,
                half_arc_rad: TANK_COAX_HALF_ARC_RAD,
                range_px: effective_weapon_range_px,
                direct_fire_legality: DirectFireLegality::intended_target(),
            },
        ) {
            continue;
        }
        candidates.push(TargetCandidate {
            id,
            owner: target.owner,
            pos_x: target.pos_x,
            pos_y: target.pos_y,
            distance_sq,
            facts: target_rules::target_facts(target.kind),
            in_weapon_range: true,
            tank_trap_obstructs_vehicle_route: false,
            retained_target: false,
        });
    }
    candidates
}
