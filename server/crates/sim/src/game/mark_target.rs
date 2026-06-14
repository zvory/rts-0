use std::collections::HashMap;

use crate::config;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::fog::Fog;
use crate::game::services::dist2;
use crate::game::teams::TeamRelations;
use crate::protocol::{Event, NoticeSeverity};
use crate::rules::combat;
use crate::rules::projection;
use crate::rules::terrain::TerrainKind;

#[derive(Debug, Clone)]
struct MarkTargetPulse {
    owner: u32,
    caster: u32,
    x: f32,
    y: f32,
    impact_tick: u32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MarkTargetStore {
    pulses: Vec<MarkTargetPulse>,
}

impl MarkTargetStore {
    pub(crate) fn schedule(&mut self, request: MarkTargetSchedule<'_>) {
        let MarkTargetSchedule {
            events,
            fog,
            teams,
            owner,
            caster,
            x,
            y,
            tick,
        } = request;
        self.pulses.push(MarkTargetPulse {
            owner,
            caster,
            x,
            y,
            impact_tick: tick.saturating_add(config::MARK_TARGET_DELAY_TICKS),
        });
        emit_marker(events, fog, teams, owner, caster, x, y);
    }

    pub(crate) fn resolve_due(
        &mut self,
        entities: &mut EntityStore,
        teams: &TeamRelations,
        fog: &Fog,
        events: &mut HashMap<u32, Vec<Event>>,
        tick: u32,
    ) {
        let mut pending = Vec::new();
        let due = std::mem::take(&mut self.pulses);
        for pulse in due {
            if pulse.impact_tick <= tick {
                resolve_pulse(entities, teams, fog, events, &pulse, tick);
            } else {
                pending.push(pulse);
            }
        }
        self.pulses = pending;
    }
}

pub(crate) struct MarkTargetSchedule<'a> {
    pub(crate) events: &'a mut HashMap<u32, Vec<Event>>,
    pub(crate) fog: &'a Fog,
    pub(crate) teams: &'a TeamRelations,
    pub(crate) owner: u32,
    pub(crate) caster: u32,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) tick: u32,
}

fn emit_marker(
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    teams: &TeamRelations,
    owner: u32,
    caster: u32,
    x: f32,
    y: f32,
) {
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for pid in player_ids {
        let allied = teams.same_team_or_same_owner(pid, owner);
        let point_visible = projection::team_visible_world(pid, x, y, fog, teams);
        if !allied && !point_visible {
            continue;
        }
        events.entry(pid).or_default().push(Event::MarkTarget {
            from: allied.then_some(caster),
            x,
            y,
            radius_tiles: config::MARK_TARGET_RADIUS_TILES,
            delay_ticks: config::MARK_TARGET_DELAY_TICKS,
        });
    }
}

fn resolve_pulse(
    entities: &mut EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    events: &mut HashMap<u32, Vec<Event>>,
    pulse: &MarkTargetPulse,
    tick: u32,
) {
    let caster_alive = matches!(
        entities.get(pulse.caster),
        Some(e) if e.owner == pulse.owner && e.kind == EntityKind::EkaterinaSignalTeam && e.hp > 0
    );
    if !caster_alive {
        return;
    }

    emit_impact(events, teams, fog, pulse.owner, pulse.x, pulse.y);

    let radius = config::MARK_TARGET_RADIUS_TILES * config::TILE_SIZE as f32;
    let radius2 = radius * radius;
    let mut hits = Vec::new();
    for id in entities.ids() {
        let Some(target) = entities.get(id) else {
            continue;
        };
        if target.owner == 0 || target.hp == 0 || !target.is_unit() || target.is_node() {
            continue;
        }
        if dist2(pulse.x, pulse.y, target.pos_x, target.pos_y) <= radius2 {
            hits.push((id, target.owner, target.pos_x, target.pos_y));
        }
    }
    hits.sort_by_key(|(id, _, _, _)| *id);
    for (id, victim_owner, x, y) in hits {
        let damage = entities
            .get(id)
            .map(|target| {
                combat::effective_damage(
                    EntityKind::EkaterinaSignalTeam,
                    target.kind,
                    config::MARK_TARGET_DAMAGE,
                    Some(TerrainKind::Open),
                )
            })
            .unwrap_or(0);
        if damage == 0 {
            continue;
        }
        let damaged = entities.get_mut(id).is_some_and(|target| {
            let attribution = teams.is_enemy_owner(pulse.owner, target.owner).then_some((
                pulse.owner,
                (pulse.x, pulse.y),
                tick,
            ));
            target.apply_damage(damage, attribution)
        });
        if damaged {
            push_under_attack_notice(events, teams, fog, pulse.owner, victim_owner, x, y);
        }
    }
}

fn emit_impact(
    events: &mut HashMap<u32, Vec<Event>>,
    teams: &TeamRelations,
    fog: &Fog,
    owner: u32,
    x: f32,
    y: f32,
) {
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for pid in player_ids {
        if !teams.same_team_or_same_owner(pid, owner)
            && !projection::team_visible_world(pid, x, y, fog, teams)
        {
            continue;
        }
        events
            .entry(pid)
            .or_default()
            .push(Event::MarkTargetImpact {
                x,
                y,
                radius_tiles: config::MARK_TARGET_RADIUS_TILES,
            });
    }
}

fn push_under_attack_notice(
    events: &mut HashMap<u32, Vec<Event>>,
    teams: &TeamRelations,
    fog: &Fog,
    attacker_owner: u32,
    victim_owner: u32,
    x: f32,
    y: f32,
) {
    if victim_owner == 0 || !teams.is_enemy_owner(attacker_owner, victim_owner) {
        return;
    }
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for pid in player_ids {
        if !teams.same_team_or_same_owner(pid, victim_owner)
            || !projection::event_visible_to_team(pid, x, y, attacker_owner, fog, teams)
        {
            continue;
        }
        events.entry(pid).or_default().push(Event::Notice {
            msg: "alert:under_attack".to_string(),
            x: Some(x),
            y: Some(y),
            severity: NoticeSeverity::Alert,
        });
    }
}
