use std::collections::HashMap;

use crate::game::entity::EntityStore;
use crate::game::entrenchment_combat;
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::teams::TeamRelations;
use crate::protocol::Event;
use crate::rules::combat as combat_rules;
use crate::rules::projection as projection_rules;
use crate::rules::terrain::TerrainKind;
use rand::Rng;

use super::events::{
    attack_reveal_for, emit_attack_event, emit_miss_event,
    push_under_attack_notices_for_visible_attack,
};
use super::projection::resolve_shot_victim;
use super::shot_blocker_index::ShotBlockerIndex;

#[derive(Clone, Copy)]
pub(super) struct ShotOutcome {
    pub(super) victim_owner: u32,
    pub(super) reveals_attacker: bool,
}

/// Apply `dmg` to `victim` from `attacker`, emitting an `Attack` event for every fired shot.
/// Returns the resolved shot outcome when a shot was emitted. Death itself is
/// handled by the death system (we only zero hp here).
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_damage(
    map: &Map,
    entities: &mut EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    rng: &mut impl Rng,
    attacker: u32,
    victim: u32,
    weapon_profile: &combat_rules::WeaponProfile,
    dmg: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
    extra_miss_chance: f32,
    tick: u32,
) -> Option<ShotOutcome> {
    if entities
        .get(victim)
        .map(|e| !e.is_targetable())
        .unwrap_or(false)
    {
        return None;
    }
    let infantry_shell = weapon_profile.id == combat_rules::WeaponKind::TankCannon
        && entities.get(victim).is_some_and(|target| {
            crate::rules::target::is_anti_tank_gun_infantry_target(target.kind)
        });
    let shot_victim = resolve_shot_victim(
        map,
        entities,
        blockers,
        teams,
        attacker,
        victim,
        attacker_owner,
        ax,
        ay,
    );
    let shot_victim = shot_victim?;
    let shot_victim_pos = entities
        .get(shot_victim)
        .map(|e| (e.pos_x, e.pos_y))
        .unwrap_or((vx, vy));
    let victim = entities.get(shot_victim);
    let victim_kind = victim.map(|e| e.kind);
    let reveals_attacker = victim_kind.is_some_and(projection_rules::shot_reveals_attacker);
    let reveal = reveals_attacker
        .then(|| attack_reveal_for(entities.get(attacker)))
        .flatten();
    let attack_recipients = emit_attack_event(
        events,
        fog,
        teams,
        attacker,
        shot_victim,
        attacker_owner,
        ax,
        ay,
        shot_victim_pos.0,
        shot_victim_pos.1,
        reveal.clone(),
        Some(weapon_profile.id.stable_id()),
    );

    // Intended targets have no intrinsic weapon miss roll. Movement penalties may still miss.
    let primary_missed = if entities.get(shot_victim).is_some() {
        let mc = extra_miss_chance.clamp(0.0, 1.0);
        if mc > 0.0 && rng.gen::<f32>() < mc {
            emit_miss_event(events, &attack_recipients, shot_victim);
            true
        } else {
            false
        }
    } else {
        false
    };
    let outcome = if primary_missed {
        victim.map(|v| ShotOutcome {
            victim_owner: v.owner,
            reveals_attacker,
        })
    } else {
        apply_direct_hit(
            map,
            entities,
            teams,
            events,
            fog,
            attacker,
            shot_victim,
            weapon_profile,
            dmg,
            attacker_owner,
            (ax, ay),
            0,
            tick,
        )
    };
    if !primary_missed
        && infantry_shell
        && victim_kind.is_some_and(crate::rules::target::is_anti_tank_gun_infantry_target)
    {
        apply_tank_he_splash(
            map,
            entities,
            teams,
            events,
            fog,
            attacker,
            shot_victim,
            attacker_owner,
            (ax, ay),
            shot_victim_pos,
            tick,
        );
    }
    outcome
}

/// Shared direct-impact effects for resolved hits, independent of trajectory geometry.
/// MG bursts pass their aggregate damage and a one-point floor after protection rounding.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_direct_hit(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    attacker: u32,
    victim: u32,
    weapon: &combat_rules::WeaponProfile,
    damage: u32,
    owner: u32,
    source: (f32, f32),
    minimum_damage: u32,
    tick: u32,
) -> Option<ShotOutcome> {
    let target = entities.get(victim)?;
    let pos = (target.pos_x, target.pos_y);
    let outcome = ShotOutcome {
        victim_owner: target.owner,
        reveals_attacker: projection_rules::shot_reveals_attacker(target.kind),
    };
    let damage = combat_rules::effective_damage_with_facing_for_weapon(
        weapon,
        target.kind,
        damage,
        Some(TerrainKind::Open),
        Some(target.facing()),
        pos,
        source,
    );
    let damage = entrenchment_combat::reduce_direct_damage(target, damage);
    let damage = map
        .damage_after_reduction_tile(pos.0, pos.1, damage)
        .max(minimum_damage);
    let hostile = teams.is_enemy_owner(owner, outcome.victim_owner);
    let damaged = entities.get_mut(victim).is_some_and(|target| {
        if hostile {
            target.apply_damage_from_entity(damage, owner, attacker, source, tick)
        } else {
            target.apply_damage(damage, None)
        }
    });
    if damaged {
        if hostile && combat_rules::weapon_triggers_tank_armor_reaction(weapon) {
            if let Some(target) = entities.get_mut(victim) {
                target.lock_tank_armor_reaction_source(source, tick);
            }
        }
        push_under_attack_notices_for_visible_attack(
            events,
            fog,
            teams,
            outcome.victim_owner,
            owner,
            source.0,
            source.1,
            pos.0,
            pos.1,
        );
    }
    Some(outcome)
}

/// Mortar-style center-distance splash, with direct-fire entrenchment protection.
/// The resolved primary already received its normal cannon hit and is excluded here.
#[allow(clippy::too_many_arguments)]
fn apply_tank_he_splash(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    attacker: u32,
    primary: u32,
    owner: u32,
    source: (f32, f32),
    impact: (f32, f32),
    tick: u32,
) {
    let radius = crate::config::TANK_HE_RADIUS_TILES * crate::config::TILE_SIZE as f32;
    let mut victims: Vec<_> = entities
        .ids()
        .into_iter()
        .filter(|id| *id != primary)
        .collect();
    victims.sort_unstable();
    for id in victims {
        let Some(target) = entities.get(id) else {
            continue;
        };
        let dx = target.pos_x - impact.0;
        let dy = target.pos_y - impact.1;
        if (target.owner == 0 && !target.is_neutral_obstacle())
            || target.hp == 0
            || !target.is_targetable()
            || dx * dx + dy * dy > radius * radius
        {
            continue;
        }
        let damage = combat_rules::effective_damage(
            crate::game::entity::EntityKind::MortarTeam,
            target.kind,
            crate::config::TANK_HE_SPLASH_DAMAGE,
            Some(TerrainKind::Open),
        );
        let damage = entrenchment_combat::reduce_direct_damage(target, damage);
        let damage = map.damage_after_reduction_tile(target.pos_x, target.pos_y, damage);
        let victim_owner = target.owner;
        let pos = (target.pos_x, target.pos_y);
        let damaged = entities.get_mut(id).is_some_and(|target| {
            if teams.is_enemy_owner(owner, target.owner) {
                target.apply_damage_from_entity(damage, owner, attacker, source, tick)
            } else {
                target.apply_damage(damage, None)
            }
        });
        if damaged {
            push_under_attack_notices_for_visible_attack(
                events,
                fog,
                teams,
                victim_owner,
                owner,
                source.0,
                source.1,
                pos.0,
                pos.1,
            );
        }
    }
    for (&pid, recipient_events) in events.iter_mut() {
        if projection_rules::team_visible_world(pid, impact.0, impact.1, fog, teams) {
            recipient_events.push(Event::MortarImpact {
                from: None,
                x: impact.0,
                y: impact.1,
                radius_tiles: crate::config::TANK_HE_RADIUS_TILES,
                reveal: None,
                rocket: false,
            });
        }
    }
}
