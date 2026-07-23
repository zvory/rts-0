use std::collections::HashMap;

use crate::game::entity::{EntityKind, EntityStore};
use crate::game::entrenchment_combat;
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::line_of_sight::LineOfSight;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::protocol::Event;
use crate::rules::combat as combat_rules;
use crate::rules::projection as projection_rules;
use crate::rules::terrain::TerrainKind;
use rand::Rng;

use super::events::{
    attack_reveal_for, emit_attack_event, emit_miss_event, push_under_attack_notice,
    push_under_attack_notices_for_visible_attack,
};
use super::projection::{resolve_shot_victim, shot_blocker_intersection};
use super::shot_blocker_index::ShotBlockerIndex;
use super::RANGE_SLACK;

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
    smokes: &SmokeCloudStore,
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
    range_px: f32,
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
    let reveals_attacker = victim_kind != Some(EntityKind::TankTrap);
    let reveal = reveals_attacker
        .then(|| attack_reveal_for(entities.get(attacker)))
        .flatten();
    let victim_facing = victim.map(|e| e.facing());
    let victim_entrenched = victim.is_some_and(entrenchment_combat::is_actively_entrenched);
    let victim_owner = entities.get(shot_victim).map(|e| e.owner).unwrap_or(0);
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

    // Resolve weapon-specific accuracy before computing damage. Entrenchment is deterministic
    // damage reduction, not another miss source. A miss still leaves the shell path live so each
    // overpenetration candidate can make its own independent accuracy roll.
    let primary_missed = if let Some(v) = entities.get(shot_victim) {
        let mc = combat_rules::miss_chance_for_weapon(weapon_profile, v.kind)
            .max(extra_miss_chance.clamp(0.0, 1.0));
        if mc > 0.0 && rng.gen::<f32>() < mc {
            emit_miss_event(events, &attack_recipients, shot_victim);
            true
        } else {
            false
        }
    } else {
        false
    };
    let unentrenched_dmg = match victim_kind {
        Some(vk) => combat_rules::effective_damage_with_facing_for_weapon(
            weapon_profile,
            vk,
            dmg,
            Some(TerrainKind::Open),
            victim_facing,
            shot_victim_pos,
            (ax, ay),
        ),
        _ => dmg,
    };
    let effective_dmg = entities
        .get(shot_victim)
        .map(|victim| entrenchment_combat::reduce_direct_damage(victim, unentrenched_dmg))
        .unwrap_or(unentrenched_dmg);
    let damaged = if primary_missed {
        false
    } else if let Some(v) = entities.get_mut(shot_victim) {
        let attribution = teams.is_enemy_owner(attacker_owner, v.owner).then_some((
            attacker_owner,
            (ax, ay),
            tick,
        ));
        v.apply_damage(effective_dmg, attribution)
    } else {
        false
    };
    if damaged {
        if teams.is_enemy_owner(attacker_owner, victim_owner)
            && combat_rules::weapon_triggers_tank_armor_reaction(weapon_profile)
        {
            if let Some(victim) = entities.get_mut(shot_victim) {
                victim.lock_tank_armor_reaction_source((ax, ay), tick);
            }
        }
        push_under_attack_notices_for_visible_attack(
            events,
            fog,
            teams,
            victim_owner,
            attacker_owner,
            ax,
            ay,
            shot_victim_pos.0,
            shot_victim_pos.1,
        );
    }
    if damaged || primary_missed {
        apply_overpenetration(
            map,
            entities,
            teams,
            events,
            fog,
            smokes,
            rng,
            attacker,
            shot_victim,
            weapon_profile,
            damaged && victim_entrenched,
            if primary_missed {
                unentrenched_dmg
            } else {
                effective_dmg
            },
            attacker_owner,
            ax,
            ay,
            shot_victim_pos.0,
            shot_victim_pos.1,
            range_px,
            tick,
        );
    }
    victim_kind.map(|_| ShotOutcome {
        victim_owner,
        reveals_attacker,
    })
}

#[allow(clippy::too_many_arguments)]
fn apply_overpenetration(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    rng: &mut impl Rng,
    attacker: u32,
    primary_victim: u32,
    weapon_profile: &combat_rules::WeaponProfile,
    primary_victim_was_entrenched: bool,
    primary_dmg: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
    range_px: f32,
    tick: u32,
) {
    if primary_victim_was_entrenched {
        return;
    }
    if entities
        .get(primary_victim)
        .map(|e| e.kind == EntityKind::Tank || e.is_building())
        .unwrap_or(false)
    {
        return;
    }
    let dx = vx - ax;
    let dy = vy - ay;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist <= f32::EPSILON {
        return;
    }

    let overpenetration_factor = match weapon_profile.overpenetration {
        combat_rules::OverpenetrationPolicy::DirectFire { range_factor } => range_factor,
        combat_rules::OverpenetrationPolicy::None => return,
    };
    let overpenetration_limit = dist + range_px * overpenetration_factor;
    let ux = dx / dist;
    let uy = dy / dist;
    let shot_end = (
        ax + ux * overpenetration_limit,
        ay + uy * overpenetration_limit,
    );
    let perpendicular_slack = RANGE_SLACK + 8.0;
    let splash_dmg = primary_dmg / 2;
    if splash_dmg == 0 {
        return;
    }

    let player_ids: Vec<u32> = events.keys().copied().collect();
    let mut hits: Vec<(u32, f32, f32, f32)> = Vec::new();
    let los = LineOfSight::with_smoke(map, smokes);
    for id in entities.ids() {
        if id == attacker || id == primary_victim {
            continue;
        }
        let Some(target) = entities.get(id) else {
            continue;
        };
        if !target.is_targetable()
            || !teams.is_enemy_owner(attacker_owner, target.owner)
            || target.hp == 0
        {
            continue;
        }
        if entrenchment_combat::is_actively_entrenched(target) {
            continue;
        }
        let along = if target.kind == EntityKind::Tank || target.is_building() {
            let Some(hit_t) = shot_blocker_intersection(map, target, (ax, ay), shot_end) else {
                continue;
            };
            hit_t * overpenetration_limit
        } else {
            let tx = target.pos_x - ax;
            let ty = target.pos_y - ay;
            let along = tx * ux + ty * uy;
            if along <= dist || along > overpenetration_limit {
                continue;
            }
            let perp = (tx * uy - ty * ux).abs();
            if perp > target.radius() + perpendicular_slack {
                continue;
            }
            along
        };
        if along <= dist || along > overpenetration_limit {
            continue;
        }
        if !los.clear_between_world_points((ax, ay), (target.pos_x, target.pos_y)) {
            continue;
        }
        hits.push((id, target.pos_x, target.pos_y, along));
    }

    hits.sort_by(|a, b| a.3.total_cmp(&b.3).then_with(|| a.0.cmp(&b.0)));
    for (id, tx, ty, _) in hits {
        let missed = entities.get(id).is_some_and(|target| {
            let miss_chance = combat_rules::miss_chance_for_weapon(weapon_profile, target.kind);
            miss_chance > 0.0 && rng.gen::<f32>() < miss_chance
        });
        if missed {
            continue;
        }
        let effective_dmg = entities
            .get(id)
            .map(|e| {
                combat_rules::effective_damage_with_facing_for_weapon(
                    weapon_profile,
                    e.kind,
                    splash_dmg,
                    Some(TerrainKind::Open),
                    Some(e.facing()),
                    (e.pos_x, e.pos_y),
                    (ax, ay),
                )
            })
            .unwrap_or(0);
        if effective_dmg == 0 {
            continue;
        }
        let victim_owner = entities.get(id).map(|e| e.owner).unwrap_or(0);
        let shot_blocked = entities
            .get(id)
            .map(|e| e.kind == EntityKind::Tank || e.is_building())
            .unwrap_or(false);
        if let Some(v) = entities.get_mut(id) {
            if v.apply_damage(effective_dmg, Some((attacker_owner, (ax, ay), tick)))
                && combat_rules::weapon_triggers_tank_armor_reaction(weapon_profile)
            {
                v.lock_tank_armor_reaction_source((ax, ay), tick);
            }
        }
        for pid in &player_ids {
            if !projection_rules::attack_event_visible_to_team(
                *pid,
                ax,
                ay,
                tx,
                ty,
                attacker_owner,
                fog,
                teams,
            ) {
                continue;
            }
            events
                .entry(*pid)
                .or_default()
                .push(Event::Overpenetration { to: id });
            push_under_attack_notice(events, teams, *pid, victim_owner, attacker_owner, tx, ty);
        }
        if shot_blocked {
            break;
        }
    }
}
