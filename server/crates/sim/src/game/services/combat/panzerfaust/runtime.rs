use std::collections::HashMap;

use crate::config;
use crate::game::entity::{AttackPhase, EntityKind, EntityStore, PanzerfaustState};
use crate::protocol::Event;
use crate::rules::combat as combat_rules;
use crate::rules::projection as projection_rules;
use crate::rules::terrain::TerrainKind;

use super::super::events::push_under_attack_notice;
use super::events::{emit_conversion, emit_impact, emit_launch, LaunchEvent};
use super::{
    convert_panzerfaust_to_rifleman, mirror_weapon_to_body, panzerfaust_state,
    panzerfaust_target_fireable, panzerfaust_target_in_range, panzerfaust_target_valid,
    recovery_ticks, set_panzerfaust_state, Fog, LineOfSight, Map, PanzerfaustFireContext,
    SmokeCloudStore, TeamRelations,
};

#[allow(clippy::too_many_arguments)]
pub(in crate::game::services::combat) fn tick_states(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    methamphetamines_researched: &dyn Fn(u32) -> bool,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    events: &mut HashMap<u32, Vec<Event>>,
    tick: u32,
) {
    let los = LineOfSight::with_smoke(map, smokes);
    for id in entities.ids() {
        let Some(state) = entities.get(id).and_then(panzerfaust_state) else {
            continue;
        };
        if entities.get(id).is_none_or(|entity| entity.hp == 0) {
            continue;
        }
        match state {
            PanzerfaustState::Loaded => {}
            PanzerfaustState::Windup {
                target,
                ticks_remaining,
            } => tick_windup(
                map,
                entities,
                teams,
                fog,
                smokes,
                events,
                &los,
                id,
                target,
                ticks_remaining,
            ),
            PanzerfaustState::InFlight {
                target,
                impact_x,
                impact_y,
                ticks_remaining,
            } => tick_in_flight(
                entities,
                teams,
                methamphetamines_researched,
                fog,
                smokes,
                events,
                tick,
                id,
                target,
                (impact_x, impact_y),
                ticks_remaining,
            ),
            PanzerfaustState::Recovery { ticks_remaining } => {
                tick_recovery(entities, teams, fog, smokes, events, id, ticks_remaining);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn tick_windup(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    events: &mut HashMap<u32, Vec<Event>>,
    los: &LineOfSight<'_>,
    id: u32,
    target: u32,
    ticks_remaining: u16,
) {
    let Some((owner, ax, ay, target_angle)) = entities.get(id).and_then(|attacker| {
        let target = entities.get(target)?;
        Some((
            attacker.owner,
            attacker.pos_x,
            attacker.pos_y,
            (target.pos_y - attacker.pos_y).atan2(target.pos_x - attacker.pos_x),
        ))
    }) else {
        cancel_windup(entities, id);
        return;
    };
    if !panzerfaust_target_valid(entities, teams, fog, smokes, owner, id, target)
        || !panzerfaust_target_in_range(map, entities, id, target)
        || !panzerfaust_target_fireable(
            &PanzerfaustFireContext::new(map, entities, teams, los, fog, smokes),
            id,
            owner,
            target,
        )
    {
        cancel_windup(entities, id);
        return;
    }

    if let Some(attacker) = entities.get_mut(id) {
        attacker.clear_path();
        if target_angle.is_finite() {
            attacker.set_facing(target_angle);
            mirror_weapon_to_body(attacker, target_angle);
        }
        attacker.set_target_id(Some(target));
        attacker.mark_attack_phase(AttackPhase::Firing);
    }
    if ticks_remaining > 1 {
        if let Some(attacker) = entities.get_mut(id) {
            set_panzerfaust_state(
                attacker,
                PanzerfaustState::Windup {
                    target,
                    ticks_remaining: ticks_remaining - 1,
                },
            );
        }
        return;
    }

    let (impact_x, impact_y) = entities
        .get(target)
        .map(|target| (target.pos_x, target.pos_y))
        .unwrap_or((ax, ay));
    if let Some(attacker) = entities.get_mut(id) {
        set_panzerfaust_state(
            attacker,
            PanzerfaustState::InFlight {
                target,
                impact_x,
                impact_y,
                ticks_remaining: config::PANZERFAUST_TRAVEL_TICKS,
            },
        );
    }
    emit_launch(
        events,
        fog,
        smokes,
        teams,
        LaunchEvent {
            owner,
            from: id,
            from_pos: (ax, ay),
            to_pos: (impact_x, impact_y),
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn tick_in_flight(
    entities: &mut EntityStore,
    teams: &TeamRelations,
    methamphetamines_researched: &dyn Fn(u32) -> bool,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    events: &mut HashMap<u32, Vec<Event>>,
    tick: u32,
    id: u32,
    target: u32,
    stored_impact: (f32, f32),
    ticks_remaining: u32,
) {
    if ticks_remaining > 1 {
        if let Some(attacker) = entities.get_mut(id) {
            set_panzerfaust_state(
                attacker,
                PanzerfaustState::InFlight {
                    target,
                    impact_x: stored_impact.0,
                    impact_y: stored_impact.1,
                    ticks_remaining: ticks_remaining - 1,
                },
            );
        }
        return;
    }

    let (owner, attacker_pos) = entities
        .get(id)
        .map(|attacker| (attacker.owner, (attacker.pos_x, attacker.pos_y)))
        .unwrap_or((0, stored_impact));
    let damage_result = entities.get(target).and_then(|target_entity| {
        if target_entity.hp == 0
            || !teams.is_enemy_owner(owner, target_entity.owner)
            || !combat_rules::is_panzerfaust_loaded_shot_target(target_entity.kind)
        {
            return None;
        }
        Some((
            target_entity.owner,
            target_entity.kind,
            (target_entity.pos_x, target_entity.pos_y),
        ))
    });
    let visual_impact = damage_result
        .and_then(|(_, _, pos)| {
            (projection_rules::team_visible_world(owner, pos.0, pos.1, fog, teams)
                && !smokes.point_inside(pos.0, pos.1))
            .then_some(pos)
        })
        .unwrap_or(stored_impact);
    emit_impact(events, fog, smokes, teams, owner, visual_impact);

    if let Some((victim_owner, victim_kind, victim_pos)) = damage_result {
        let damage = combat_rules::effective_damage(
            EntityKind::Panzerfaust,
            victim_kind,
            config::PANZERFAUST_DAMAGE,
            Some(TerrainKind::Open),
        );
        let damaged = entities.get_mut(target).is_some_and(|target_entity| {
            target_entity.apply_damage(damage, Some((owner, attacker_pos, tick)))
        });
        if damaged {
            push_under_attack_notice(
                events,
                teams,
                victim_owner,
                victim_owner,
                owner,
                victim_pos.0,
                victim_pos.1,
            );
        }
    }

    if let Some(attacker) = entities.get_mut(id) {
        set_panzerfaust_state(
            attacker,
            PanzerfaustState::Recovery {
                ticks_remaining: recovery_ticks(methamphetamines_researched(owner)),
            },
        );
    }
}

fn tick_recovery(
    entities: &mut EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    events: &mut HashMap<u32, Vec<Event>>,
    id: u32,
    ticks_remaining: u16,
) {
    if ticks_remaining > 1 {
        if let Some(attacker) = entities.get_mut(id) {
            set_panzerfaust_state(
                attacker,
                PanzerfaustState::Recovery {
                    ticks_remaining: ticks_remaining - 1,
                },
            );
        }
        return;
    }
    let Some((owner, x, y)) = entities
        .get(id)
        .map(|entity| (entity.owner, entity.pos_x, entity.pos_y))
    else {
        return;
    };
    let converted = entities
        .get_mut(id)
        .is_some_and(convert_panzerfaust_to_rifleman);
    if converted {
        emit_conversion(events, fog, smokes, teams, owner, id, (x, y));
    }
}

fn cancel_windup(entities: &mut EntityStore, id: u32) {
    if let Some(attacker) = entities.get_mut(id) {
        set_panzerfaust_state(attacker, PanzerfaustState::Loaded);
        attacker.set_target_id(None);
    }
}
