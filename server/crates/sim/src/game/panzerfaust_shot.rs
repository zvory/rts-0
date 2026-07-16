use std::collections::HashMap;

use crate::config;
use crate::game::entity::EntityStore;
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
        let triggers_tank_armor_reaction =
            combat::weapon_profile(combat::WeaponKind::PanzerfaustLoadedShot)
                .is_some_and(combat::weapon_triggers_tank_armor_reaction);
        let damage = combat::panzerfaust_loaded_shot_damage(victim_kind, Some(TerrainKind::Open));
        let source_pos = (shot.source_x, shot.source_y);
        let attribution = teams
            .is_enemy_owner(shot.owner, victim_owner)
            .then_some((shot.owner, source_pos, tick));
        let enemy_hit = attribution.is_some();
        let damaged = entities.get_mut(shot.target).is_some_and(|target| {
            let damaged = target.apply_damage(damage, attribution);
            if damaged && enemy_hit && triggers_tank_armor_reaction {
                target.lock_tank_armor_reaction_source(source_pos, tick);
            }
            damaged
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::EntityKind;

    #[test]
    fn enemy_panzerfaust_impact_records_launch_origin_after_attacker_displacement() {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("Rifleman should spawn");
        let tank = entities
            .spawn_unit(2, EntityKind::Tank, 140.0, 100.0)
            .expect("Tank should spawn");
        let mut shots = PanzerfaustShotStore::default();
        shots.schedule(1, attacker, tank, (100.0, 100.0), (140.0, 100.0), 0);
        entities
            .get_mut(attacker)
            .expect("Rifleman should still exist")
            .set_position(220.0, 100.0);
        let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
        let fog = Fog::new(24);
        let smokes = SmokeCloudStore::new();
        let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);

        shots.resolve_due(
            &mut entities,
            &teams,
            &fog,
            &smokes,
            &mut events,
            config::PANZERFAUST_TRAVEL_TICKS,
        );

        let lock = entities
            .get(tank)
            .and_then(|tank| tank.combat.as_ref())
            .and_then(|combat| combat.tank_armor_reaction_lock)
            .expect("Panzerfaust impact should lock its launch origin");
        assert_eq!((lock.source_x, lock.source_y), (100.0, 100.0));
        assert_eq!(
            entities.get(tank).and_then(|tank| tank.last_damage_pos()),
            Some((100.0, 100.0)),
            "damage attribution should use the same immutable launch origin"
        );
    }
}
