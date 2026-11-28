//! Common immediate-fire lifecycle. Acquisition/aiming belongs to the caller; geometry may
//! specialize, but weapon readiness, reaction gates, reveal effects and reload live here.
use std::collections::HashMap;

use rand::Rng;

use crate::game::entity::EntityStore;
use crate::game::firing_reveal::{record_firing_reveals_for_victim_team, FiringRevealSource};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::protocol::Event;
use crate::rules::combat::WeaponProfile;

use super::{
    damage, machine_gun, weapons, LineOfSight, ShotBlockerIndex, SpatialIndex,
    FIRING_REVEAL_RESPONSE_DELAY_TICKS,
};

/// Called only after the caller has established target legality, range and weapon alignment.
/// Returns whether a firing cycle was committed (including a miss).
#[allow(clippy::too_many_arguments)]
pub(super) fn try_fire(
    map: &Map,
    entities: &mut EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
    spatial: &SpatialIndex,
    los: &LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    disciple_shots: &mut crate::game::disciple_shot::DiscipleShotStore,
    rng: &mut impl Rng,
    events: &mut HashMap<u32, Vec<Event>>,
    firing_reveals: &mut Vec<FiringRevealSource>,
    attacker: u32,
    target: u32,
    weapon: &WeaponProfile,
    range: f32,
    damage: u32,
    cooldown: u32,
    tick: u32,
) -> bool {
    let Some(source) = entities.get(attacker) else {
        return false;
    };
    if source.hp == 0 {
        return false;
    }
    let (owner, start, is_unit, miss_chance) = (
        source.owner,
        (source.pos_x(), source.pos_y()),
        source.is_unit(),
        weapons::moving_fire_miss_chance(source),
    );
    let Some(victim) = entities.get(target) else {
        return false;
    };
    if victim.hp == 0 || !victim.is_targetable() {
        return false;
    }
    let end = (victim.pos_x(), victim.pos_y());
    if let Some(episode) = fog.team_firing_reveal_only_source(owner, end, teams) {
        if !entities.get_mut(attacker).is_some_and(|source| {
            source.weapon_firing_reveal_reaction_ready(
                weapon.id,
                target,
                episode,
                tick,
                FIRING_REVEAL_RESPONSE_DELAY_TICKS,
            )
        }) {
            return false;
        }
    }
    // Observe the reveal episode during reload, as the primary combat path always has.
    if !entities
        .get(attacker)
        .is_some_and(|source| source.weapon_cooldown(weapon.id) == 0)
    {
        return false;
    }
    let outcomes = if machine_gun::is_machine_gun(weapon.id) {
        machine_gun::fire(
            map, entities, teams, spatial, los, fog, smokes, rng, events, attacker, target, weapon,
            range, damage, tick,
        )
    } else {
        damage::apply_damage(
            map,
            entities,
            blockers,
            teams,
            events,
            fog,
            disciple_shots,
            rng,
            attacker,
            target,
            weapon,
            damage,
            owner,
            start.0,
            start.1,
            end.0,
            end.1,
            miss_chance,
            tick,
        )
        .into_iter()
        .collect()
    };
    if is_unit {
        for outcome in outcomes.into_iter().filter(|shot| shot.reveals_attacker) {
            record_firing_reveals_for_victim_team(
                firing_reveals,
                events.keys().copied().collect::<Vec<_>>(),
                fog,
                map,
                teams,
                outcome.victim_owner,
                owner,
                attacker,
                start,
                tick,
            );
        }
    }
    if let Some(source) = entities.get_mut(attacker) {
        let cooldown = crate::rules::combat::randomized_attack_reload(
            source.kind,
            attacker,
            weapon.id.stable_id(),
            tick,
            cooldown,
        );
        source.set_weapon_cooldown(weapon.id, cooldown);
        if source.default_weapon_kind() == Some(weapon.id) {
            source.end_war_charge();
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_orbs(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    events: &mut HashMap<u32, Vec<Event>>,
    store: &mut crate::game::disciple_shot::DiscipleShotStore,
    tick: u32,
) {
    let shots = std::mem::take(&mut store.shots);
    for shot in shots {
        if shot.impact_tick > tick {
            store.shots.push(shot);
            continue;
        }
        if !entities.get(shot.target).is_some_and(|target| {
            target.hp > 0
                && target.is_targetable()
                && (target.owner == shot.owner
                    || teams.is_enemy_owner(shot.owner, target.owner)
                    || target.is_neutral_obstacle())
        }) {
            continue;
        }
        if let Some(mut weapon) =
            crate::rules::combat::weapon_profile(crate::rules::combat::WeaponKind::DiscipleOrb)
                .copied()
        {
            weapon.armor_penetration = shot.penetration;
            damage::apply_direct_hit(
                map,
                entities,
                teams,
                events,
                fog,
                shot.attacker,
                shot.target,
                &weapon,
                shot.damage,
                shot.owner,
                (shot.source[0].to_world(), shot.source[1].to_world()),
                0,
                tick,
            );
        }
    }
}

#[cfg(test)]
mod tests;
