use std::collections::HashMap;

use crate::game::entity::{EntityKind, EntityStore};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::line_of_sight::LineOfSight;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::protocol::Event;
use crate::rules::combat as combat_rules;
use crate::rules::projection as projection_rules;
use crate::rules::terrain::TerrainKind;
use rand::rngs::SmallRng;
use rand::Rng;

use super::events::{
    attack_reveal_for, emit_attack_event, push_under_attack_notice,
    push_under_attack_notices_for_visible_attack,
};
use super::projection::{resolve_shot_victim, shot_blocker_intersection};
use super::RANGE_SLACK;

/// Apply `dmg` to `victim` from `attacker`, emitting an `Attack` event for every fired shot.
/// Death itself is handled by the death system (we only zero hp here).
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_damage(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    rng: &mut SmallRng,
    attacker: u32,
    victim: u32,
    dmg: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
    range_px: f32,
    extra_miss_chance: f32,
    tick: u32,
) {
    if entities.get(victim).map(|e| e.is_node()).unwrap_or(false) {
        return;
    }
    let shot_victim = resolve_shot_victim(
        map,
        entities,
        teams,
        attacker,
        victim,
        attacker_owner,
        ax,
        ay,
    );
    let Some(shot_victim) = shot_victim else {
        return;
    };
    let shot_victim_pos = entities
        .get(shot_victim)
        .map(|e| (e.pos_x, e.pos_y))
        .unwrap_or((vx, vy));
    let reveal = attack_reveal_for(entities.get(attacker));
    let attacker_kind = entities.get(attacker).map(|e| e.kind);
    let victim_kind = entities.get(shot_victim).map(|e| e.kind);
    let victim_facing = entities.get(shot_victim).map(|e| e.facing());
    let victim_owner = entities.get(shot_victim).map(|e| e.owner).unwrap_or(0);
    emit_attack_event(
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
    );

    // Roll for miss before computing damage.
    if let (Some(ak), Some(vk)) = (attacker_kind, victim_kind) {
        let mc = combat_rules::miss_chance(ak, vk).max(extra_miss_chance);
        if mc > 0.0 && rng.gen::<f32>() < mc {
            return;
        }
    }
    let effective_dmg = match (attacker_kind, victim_kind) {
        (Some(ak), Some(vk)) => combat_rules::effective_damage_with_facing(
            ak,
            vk,
            dmg,
            Some(TerrainKind::Open),
            victim_facing,
            shot_victim_pos,
            (ax, ay),
        ),
        _ => dmg,
    };
    let damaged = if let Some(v) = entities.get_mut(shot_victim) {
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
        apply_overpenetration(
            map,
            entities,
            teams,
            events,
            fog,
            smokes,
            attacker,
            shot_victim,
            effective_dmg,
            attacker_owner,
            ax,
            ay,
            shot_victim_pos.0,
            shot_victim_pos.1,
            range_px,
            tick,
        );
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
}

#[allow(clippy::too_many_arguments)]
fn apply_overpenetration(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    attacker: u32,
    primary_victim: u32,
    primary_dmg: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
    range_px: f32,
    tick: u32,
) {
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

    let overpenetration_factor = match entities.get(attacker).map(|e| e.kind) {
        Some(EntityKind::AtTeam) => 0.50,
        _ => 0.25,
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
        if target.is_node() || !teams.is_enemy_owner(attacker_owner, target.owner) || target.hp == 0
        {
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
        let attacker_kind = entities.get(attacker).map(|e| e.kind);
        let effective_dmg = entities
            .get(id)
            .map(|e| match attacker_kind {
                Some(ak) => combat_rules::effective_damage_with_facing(
                    ak,
                    e.kind,
                    splash_dmg,
                    Some(TerrainKind::Open),
                    Some(e.facing()),
                    (e.pos_x, e.pos_y),
                    (ax, ay),
                ),
                None => splash_dmg,
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
            v.apply_damage(effective_dmg, Some((attacker_owner, (ax, ay), tick)));
        }
        let reveal = attack_reveal_for(entities.get(attacker));
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
            events.entry(*pid).or_default().push(Event::Attack {
                from: attacker,
                to: id,
                reveal: reveal.clone(),
                to_pos: Some([tx, ty]),
            });
            push_under_attack_notice(events, teams, *pid, victim_owner, attacker_owner, tx, ty);
        }
        if shot_blocked {
            break;
        }
    }
}
