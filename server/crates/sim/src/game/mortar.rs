use std::collections::HashMap;

use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::game::firing_reveal::{record_mortar_impact_firing_reveals, FiringRevealSource};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::dist2;
use crate::game::teams::TeamRelations;
use crate::protocol::{self, AttackReveal, Event};
use crate::rules::combat;
use crate::rules::projection;
use crate::rules::terrain::TerrainKind;

pub(crate) const FIRE_TOLERANCE_RAD: f32 = 15.0_f32.to_radians();
pub(crate) const HALF_TURN_TICKS: u32 = config::TICK_HZ / 5;
pub(crate) const TURN_RATE_RAD_PER_TICK: f32 =
    std::f32::consts::PI / HALF_TURN_TICKS as f32;

#[derive(Debug, Clone)]
struct MortarShell {
    owner: u32,
    attacker: u32,
    x: f32,
    y: f32,
    impact_tick: u32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MortarShellStore {
    shells: Vec<MortarShell>,
}

pub(crate) fn rotate_mortar_for_fire(e: &mut Entity, target_angle: f32) -> bool {
    if !target_angle.is_finite() {
        return false;
    }
    e.set_desired_weapon_facing(target_angle);
    let current = e
        .weapon_facing()
        .filter(|facing| facing.is_finite())
        .unwrap_or_else(|| e.facing());
    let rotated = rotate_toward(current, target_angle, TURN_RATE_RAD_PER_TICK);
    if rotated.is_finite() {
        e.set_facing(rotated);
        e.set_weapon_facing(rotated);
    }
    angle_delta(rotated, target_angle).abs() <= FIRE_TOLERANCE_RAD
}

pub(crate) fn mortar_current_facing_ready(e: &Entity, target_angle: f32) -> bool {
    let current = e
        .weapon_facing()
        .filter(|facing| facing.is_finite())
        .unwrap_or_else(|| e.facing());
    target_angle.is_finite()
        && current.is_finite()
        && angle_delta(current, target_angle).abs() <= FIRE_TOLERANCE_RAD
}

fn angle_delta(from: f32, to: f32) -> f32 {
    (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

fn rotate_toward(current: f32, desired: f32, max_delta: f32) -> f32 {
    if !desired.is_finite() || !max_delta.is_finite() {
        return current;
    }
    if !current.is_finite() {
        return desired;
    }
    let delta = angle_delta(current, desired);
    if delta.abs() <= max_delta {
        desired
    } else {
        current + delta.signum() * max_delta
    }
}

impl MortarShellStore {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn schedule(
        &mut self,
        events: &mut HashMap<u32, Vec<Event>>,
        fog: &Fog,
        teams: &TeamRelations,
        owner: u32,
        attacker: u32,
        from_x: f32,
        from_y: f32,
        x: f32,
        y: f32,
        tick: u32,
        reveal_launch_to_enemies: bool,
    ) {
        self.shells.push(MortarShell {
            owner,
            attacker,
            x,
            y,
            impact_tick: tick.saturating_add(config::MORTAR_SHELL_DELAY_TICKS),
        });
        emit_launch(
            events,
            fog,
            teams,
            owner,
            attacker,
            from_x,
            from_y,
            x,
            y,
            reveal_launch_to_enemies,
        );
    }

    pub(in crate::game) fn resolve_due(
        &mut self,
        map: &Map,
        entities: &mut EntityStore,
        teams: &TeamRelations,
        fog: &Fog,
        events: &mut HashMap<u32, Vec<Event>>,
        firing_reveals: &mut Vec<FiringRevealSource>,
        tick: u32,
    ) {
        let mut pending = Vec::new();
        let due = std::mem::take(&mut self.shells);
        for shell in due {
            if shell.impact_tick <= tick {
                resolve(map, entities, teams, fog, events, firing_reveals, &shell, tick);
            } else {
                pending.push(shell);
            }
        }
        self.shells = pending;
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_launch(
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    teams: &TeamRelations,
    owner: u32,
    attacker: u32,
    from_x: f32,
    from_y: f32,
    to_x: f32,
    to_y: f32,
    reveal_launch_to_enemies: bool,
) {
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for pid in player_ids {
        let allied = teams.same_team_or_same_owner(pid, owner);
        if !allied
            && (!reveal_launch_to_enemies
                || !projection::team_visible_world(pid, from_x, from_y, fog, teams))
        {
            continue;
        }
        events.entry(pid).or_default().push(Event::MortarLaunch {
            from: attacker,
            from_x,
            from_y,
            to_x,
            to_y,
            radius_tiles: config::MORTAR_OUTER_RADIUS_TILES,
            delay_ticks: config::MORTAR_SHELL_DELAY_TICKS,
        });
    }
}

fn resolve(
    _map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    events: &mut HashMap<u32, Vec<Event>>,
    firing_reveals: &mut Vec<FiringRevealSource>,
    shell: &MortarShell,
    tick: u32,
) {
    let outer_radius = config::MORTAR_OUTER_RADIUS_TILES * config::TILE_SIZE as f32;
    let inner_radius = config::MORTAR_INNER_RADIUS_TILES * config::TILE_SIZE as f32;
    let outer2 = outer_radius * outer_radius;
    let inner2 = inner_radius * inner_radius;
    let mut hits = Vec::new();
    for id in entities.ids() {
        let Some(target) = entities.get(id) else {
            continue;
        };
        if target.owner == 0 || target.hp == 0 || target.is_node() {
            continue;
        }
        let d2 = dist2(shell.x, shell.y, target.pos_x, target.pos_y);
        if d2 <= outer2 {
            let inner_hit = d2 <= inner2;
            let base = if inner_hit {
                config::MORTAR_INNER_DAMAGE
            } else {
                config::MORTAR_OUTER_DAMAGE
            };
            hits.push((id, base, inner_hit, target.owner, target.pos_x, target.pos_y));
        }
    }
    hits.sort_by_key(|(id, _, _, _, _, _)| *id);
    let reveal = mortar_reveal_for(entities.get(shell.attacker), shell.owner);
    let mut reveal_recipients = Vec::new();
    for (id, base, inner_hit, victim_owner, tx, ty) in hits {
        let effective = entities
            .get(id)
            .map(|target| mortar_damage(target.kind, base, inner_hit))
            .unwrap_or(0);
        if effective == 0 {
            continue;
        }
        let damaged = entities.get_mut(id).is_some_and(|target| {
            let attribution = teams.is_enemy_owner(shell.owner, target.owner).then_some((
                shell.owner,
                (shell.x, shell.y),
                tick,
            ));
            target.apply_damage(effective, attribution)
        });
        if damaged {
            if teams.is_enemy_owner(shell.owner, victim_owner) && reveal.is_some() {
                reveal_recipients.push(victim_owner);
            }
            push_under_attack_notice(events, teams, fog, shell.owner, victim_owner, tx, ty);
        }
    }
    reveal_recipients.sort_unstable();
    reveal_recipients.dedup();
    record_mortar_impact_firing_reveals(
        firing_reveals,
        events,
        fog,
        teams,
        &reveal_recipients,
        shell.owner,
        shell.attacker,
        reveal.as_ref(),
        tick,
    );
    emit_impact(
        events,
        fog,
        teams,
        shell.owner,
        shell.attacker,
        reveal.as_ref(),
        &reveal_recipients,
        shell.x,
        shell.y,
    );
}

fn mortar_reveal_for(attacker: Option<&Entity>, owner: u32) -> Option<AttackReveal> {
    let attacker = attacker?;
    if attacker.owner != owner || attacker.kind != EntityKind::MortarTeam || attacker.hp == 0 {
        return None;
    }
    Some(AttackReveal {
        owner: attacker.owner,
        kind: protocol::kind_to_wire(attacker.kind).to_string(),
        x: attacker.pos_x,
        y: attacker.pos_y,
        facing: Some(attacker.facing()),
        weapon_facing: attacker.weapon_facing(),
        setup_state: Some(attacker.weapon_setup().to_protocol_str().to_string()),
    })
}

fn mortar_damage(victim_kind: EntityKind, base: u32, inner_hit: bool) -> u32 {
    if !combat::is_armored(victim_kind) {
        return combat::effective_damage(
            EntityKind::MortarTeam,
            victim_kind,
            base,
            Some(TerrainKind::Open),
        );
    }
    if inner_hit {
        return base;
    }
    ((base as f32) * 0.625).round() as u32
}

#[allow(clippy::too_many_arguments)]
fn emit_impact(
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    teams: &TeamRelations,
    owner: u32,
    attacker: u32,
    reveal: Option<&AttackReveal>,
    reveal_recipients: &[u32],
    x: f32,
    y: f32,
) {
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for pid in player_ids {
        let reveal_to_recipient = reveal_recipients.binary_search(&pid).is_ok();
        if !teams.same_team_or_same_owner(pid, owner)
            && !reveal_to_recipient
            && !projection::team_visible_world(pid, x, y, fog, teams)
        {
            continue;
        }
        events.entry(pid).or_default().push(Event::MortarImpact {
            from: reveal_to_recipient.then_some(attacker),
            x,
            y,
            radius_tiles: config::MORTAR_OUTER_RADIUS_TILES,
            reveal: reveal_to_recipient.then(|| reveal.cloned()).flatten(),
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
        if pid != victim_owner
            || !projection::event_visible_to_team(pid, x, y, attacker_owner, fog, teams)
        {
            continue;
        }
        events.entry(pid).or_default().push(Event::Notice {
            msg: "alert:under_attack".to_string(),
            x: Some(x),
            y: Some(y),
            severity: crate::protocol::NoticeSeverity::Alert,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::terrain;

    fn open_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![(4, 4), (size - 5, size - 5)],
            expansion_sites: Vec::new(),
        }
    }

    fn visible_team_fog(map: &Map, entities: &EntityStore) -> Fog {
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2, 3], entities, map);
        fog
    }

    fn has_under_attack_notice(events: &HashMap<u32, Vec<Event>>, player: u32) -> bool {
        events.get(&player).is_some_and(|player_events| {
            player_events
                .iter()
                .any(|event| matches!(event, Event::Notice { msg, .. } if msg == "alert:under_attack"))
        })
    }

    #[test]
    fn half_turn_completes_in_two_hundred_ms() {
        assert_eq!(
            HALF_TURN_TICKS * 1000 / config::TICK_HZ,
            200,
            "mortar half-turn timing should stay at 200 ms"
        );

        let mut entities = EntityStore::new();
        let mortar_id = entities
            .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
            .expect("mortar should spawn");
        if let Some(mortar) = entities.get_mut(mortar_id) {
            mortar.set_facing(0.0);
            mortar.set_weapon_facing(0.0);
        }

        let target_angle = std::f32::consts::PI;
        for tick in 1..=HALF_TURN_TICKS {
            let ready = {
                let mortar = entities.get_mut(mortar_id).expect("mortar should exist");
                rotate_mortar_for_fire(mortar, target_angle)
            };
            if tick < HALF_TURN_TICKS {
                assert!(
                    !ready,
                    "mortar should still be rotating on half-turn tick {tick}"
                );
            } else {
                assert!(ready, "mortar should complete a 180-degree turn in 200 ms");
            }
        }

        let mortar = entities.get(mortar_id).expect("mortar should exist");
        assert!(
            angle_delta(mortar.facing(), target_angle).abs() <= FIRE_TOLERANCE_RAD + 0.001,
            "mortar should finish the half-turn aligned with the target, got {:.4}",
            mortar.facing()
        );
    }

    #[test]
    fn mortar_under_attack_notice_goes_to_victim_owner_not_teammate() {
        let map = open_map(20);
        let mut entities = EntityStore::new();
        entities
            .spawn_unit(2, EntityKind::Worker, 160.0, 160.0)
            .expect("victim should spawn");
        entities
            .spawn_unit(3, EntityKind::Worker, 176.0, 160.0)
            .expect("victim ally should spawn");
        let fog = visible_team_fog(&map, &entities);
        let teams = TeamRelations::from_player_teams([(1, 1), (2, 7), (3, 7)]);
        let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new()), (3, Vec::new())]);

        push_under_attack_notice(&mut events, &teams, &fog, 1, 2, 160.0, 160.0);

        assert!(has_under_attack_notice(&events, 2));
        assert!(!has_under_attack_notice(&events, 3));
        assert!(!has_under_attack_notice(&events, 1));
    }
}
