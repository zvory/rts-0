use std::collections::HashMap;

use crate::config;
use crate::game::entity::{EntityKind, EntityStore, PanzerfaustState};
use crate::game::fog::Fog;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::protocol::{Event, NoticeSeverity};
use crate::rules::combat;
use crate::rules::projection;
use crate::rules::terrain::TerrainKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PanzerfaustShot {
    owner: u32,
    attacker: u32,
    target: u32,
    source_x: f32,
    source_y: f32,
    impact_x: f32,
    impact_y: f32,
    impact_tick: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct PanzerfaustShotStore {
    shots: Vec<PanzerfaustShot>,
}

impl PanzerfaustShotStore {
    pub(in crate::game) fn checkpoint_len(&self) -> usize {
        self.shots.len()
    }

    #[allow(clippy::type_complexity)]
    pub(in crate::game) fn checkpoint_entries(
        &self,
    ) -> impl Iterator<Item = (u32, u32, u32, f32, f32, f32, f32, u32)> + '_ {
        self.shots.iter().map(|shot| {
            (
                shot.owner,
                shot.attacker,
                shot.target,
                shot.source_x,
                shot.source_y,
                shot.impact_x,
                shot.impact_y,
                shot.impact_tick,
            )
        })
    }

    pub(in crate::game) fn backfill_legacy_in_flight(
        mut self,
        entities: &EntityStore,
        tick: u32,
    ) -> Self {
        if !self.shots.is_empty() {
            return self;
        }
        for entity in entities.iter() {
            let Some(PanzerfaustState::InFlight {
                target,
                impact_x,
                impact_y,
                ticks_remaining,
            }) = entity.combat.as_ref().and_then(|combat| combat.panzerfaust)
            else {
                continue;
            };
            if entity.kind != EntityKind::Panzerfaust
                || entity.owner == 0
                || entity.hp == 0
                || target == 0
                || !entity.pos_x.is_finite()
                || !entity.pos_y.is_finite()
                || !impact_x.is_finite()
                || !impact_y.is_finite()
            {
                continue;
            }
            self.shots.push(PanzerfaustShot {
                owner: entity.owner,
                attacker: entity.id,
                target,
                source_x: entity.pos_x,
                source_y: entity.pos_y,
                impact_x,
                impact_y,
                impact_tick: tick.saturating_add(ticks_remaining),
            });
        }
        self
    }

    pub(in crate::game) fn schedule(
        &mut self,
        owner: u32,
        attacker: u32,
        target: u32,
        source: (f32, f32),
        impact: (f32, f32),
        tick: u32,
    ) {
        if owner == 0
            || attacker == 0
            || target == 0
            || !source.0.is_finite()
            || !source.1.is_finite()
            || !impact.0.is_finite()
            || !impact.1.is_finite()
        {
            return;
        }
        self.shots.push(PanzerfaustShot {
            owner,
            attacker,
            target,
            source_x: source.0,
            source_y: source.1,
            impact_x: impact.0,
            impact_y: impact.1,
            impact_tick: tick.saturating_add(config::PANZERFAUST_TRAVEL_TICKS),
        });
    }

    pub(in crate::game) fn resolve_due(
        &mut self,
        entities: &mut EntityStore,
        teams: &TeamRelations,
        fog: &Fog,
        smokes: &SmokeCloudStore,
        events: &mut HashMap<u32, Vec<Event>>,
        tick: u32,
    ) {
        let mut pending = Vec::new();
        let due = std::mem::take(&mut self.shots);
        for shot in due {
            if shot.impact_tick <= tick {
                resolve(entities, teams, fog, smokes, events, &shot, tick);
            } else {
                pending.push(shot);
            }
        }
        self.shots = pending;
    }
}

fn resolve(
    entities: &mut EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    events: &mut HashMap<u32, Vec<Event>>,
    shot: &PanzerfaustShot,
    tick: u32,
) {
    let damage_result = entities.get(shot.target).and_then(|target| {
        let target_attackable =
            target.owner == shot.owner || teams.is_enemy_owner(shot.owner, target.owner);
        if target.hp == 0
            || !target_attackable
            || !combat::is_panzerfaust_loaded_shot_target(target.kind)
        {
            return None;
        }
        Some((target.owner, target.kind, (target.pos_x, target.pos_y)))
    });
    let visual_impact = damage_result
        .and_then(|(_, _, pos)| {
            (projection::team_visible_world(shot.owner, pos.0, pos.1, fog, teams)
                && !smokes.point_inside(pos.0, pos.1))
            .then_some(pos)
        })
        .unwrap_or((shot.impact_x, shot.impact_y));
    emit_impact(events, fog, smokes, teams, shot.owner, visual_impact);

    if let Some((victim_owner, victim_kind, victim_pos)) = damage_result {
        let damage = combat::effective_damage(
            EntityKind::Panzerfaust,
            victim_kind,
            config::PANZERFAUST_DAMAGE,
            Some(TerrainKind::Open),
        );
        let attacker_pos = entities
            .get(shot.attacker)
            .map(|attacker| (attacker.pos_x, attacker.pos_y))
            .unwrap_or((shot.source_x, shot.source_y));
        let attribution = teams.is_enemy_owner(shot.owner, victim_owner).then_some((
            shot.owner,
            attacker_pos,
            tick,
        ));
        let damaged = entities
            .get_mut(shot.target)
            .is_some_and(|target| target.apply_damage(damage, attribution));
        if damaged {
            push_under_attack_notice(events, teams, victim_owner, shot.owner, victim_pos);
        }
    }
}

fn emit_impact(
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    teams: &TeamRelations,
    owner: u32,
    impact: (f32, f32),
) {
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for player_id in player_ids {
        if projection::event_visible_to_team_with_smoke(
            player_id, impact.0, impact.1, owner, fog, teams, smokes,
        ) {
            events
                .entry(player_id)
                .or_default()
                .push(Event::PanzerfaustImpact {
                    x: impact.0,
                    y: impact.1,
                });
        }
    }
}

fn push_under_attack_notice(
    events: &mut HashMap<u32, Vec<Event>>,
    teams: &TeamRelations,
    victim_owner: u32,
    attacker_owner: u32,
    victim_pos: (f32, f32),
) {
    if victim_owner == 0 || !teams.is_enemy_owner(attacker_owner, victim_owner) {
        return;
    }
    events.entry(victim_owner).or_default().push(Event::Notice {
        msg: "alert:under_attack".to_string(),
        x: Some(victim_pos.0),
        y: Some(victim_pos.1),
        severity: NoticeSeverity::Alert,
    });
}
