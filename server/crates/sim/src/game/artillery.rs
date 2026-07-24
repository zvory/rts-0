use std::collections::HashMap;

use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::game::entrenchment_combat;
use crate::game::fog::Fog;
use crate::game::services::geometry::RectBody;
use crate::game::teams::TeamRelations;
use crate::protocol::{Event, NoticeSeverity};
use crate::rules::combat;
use crate::rules::projection;
use crate::rules::terrain::TerrainKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArtilleryShell {
    owner: u32,
    x: f32,
    y: f32,
    impact_tick: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ArtilleryShellStore {
    shells: Vec<ArtilleryShell>,
}

impl ArtilleryShellStore {
    pub(in crate::game) fn checkpoint_len(&self) -> usize {
        self.shells.len()
    }

    pub(crate) fn schedule(&mut self, owner: u32, _attacker: u32, x: f32, y: f32, tick: u32) {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        self.shells.push(ArtilleryShell {
            owner,
            x,
            y,
            impact_tick: tick.saturating_add(config::ARTILLERY_SHELL_DELAY_TICKS),
        });
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
        let due = std::mem::take(&mut self.shells);
        for shell in due {
            if shell.impact_tick <= tick {
                resolve_shell(entities, teams, fog, events, &shell, tick);
            } else {
                pending.push(shell);
            }
        }
        self.shells = pending;
    }
}

fn resolve_shell(
    entities: &mut EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    events: &mut HashMap<u32, Vec<Event>>,
    shell: &ArtilleryShell,
    tick: u32,
) {
    emit_impact(events, teams, fog, shell.owner, shell.x, shell.y);

    let outer_radius = config::ARTILLERY_OUTER_RADIUS_TILES * config::TILE_SIZE as f32;
    let inner_radius = config::ARTILLERY_INNER_RADIUS_TILES * config::TILE_SIZE as f32;
    let outer2 = outer_radius * outer_radius;
    let inner2 = inner_radius * inner_radius;
    let mut hits = Vec::new();
    for id in entities.ids() {
        let Some(target) = entities.get(id) else {
            continue;
        };
        if target.owner == 0 || target.hp == 0 || !target.is_targetable() {
            continue;
        }
        let d2 = impact_distance2_to_target(shell.x, shell.y, target);
        if d2 <= outer2 {
            hits.push((id, d2));
        }
    }
    hits.sort_by_key(|(id, _)| *id);
    for (id, d2) in hits {
        let Some(target) = entities.get(id) else {
            continue;
        };
        let damage = entrenchment_combat::reduce_area_damage(
            target,
            artillery_damage(target.kind, d2, inner2, outer2),
        );
        if damage == 0 {
            continue;
        }
        let mut damaged_owner = None;
        if let Some(target) = entities.get_mut(id) {
            let attribution = teams.is_enemy_owner(shell.owner, target.owner).then_some((
                shell.owner,
                (shell.x, shell.y),
                tick,
            ));
            if target.apply_damage(damage, attribution) {
                damaged_owner = Some((target.owner, target.pos_x, target.pos_y));
            }
        }
        if let Some((victim_owner, x, y)) = damaged_owner {
            push_under_attack_notice(events, teams, fog, shell.owner, victim_owner, x, y);
        }
    }
}

fn artillery_damage(victim_kind: EntityKind, d2: f32, inner2: f32, outer2: f32) -> u32 {
    if d2 <= inner2 {
        return combat::effective_damage(
            EntityKind::Artillery,
            victim_kind,
            config::ARTILLERY_INNER_DAMAGE,
            Some(TerrainKind::Open),
        );
    }
    if d2 > outer2 {
        return 0;
    }
    let inner = inner2.sqrt();
    let outer = outer2.sqrt();
    if outer <= inner {
        return 0;
    }
    let d = d2.sqrt();
    let t = ((d - inner) / (outer - inner)).clamp(0.0, 1.0);
    let base = (config::ARTILLERY_INNER_DAMAGE as f32
        + (config::ARTILLERY_OUTER_MIN_DAMAGE as f32 - config::ARTILLERY_INNER_DAMAGE as f32) * t)
        .round()
        .max(0.0) as u32;
    combat::effective_damage(
        EntityKind::Rifleman,
        victim_kind,
        base,
        Some(TerrainKind::Open),
    )
}

fn impact_distance2_to_target(x: f32, y: f32, target: &Entity) -> f32 {
    if let Some(rect) = building_rect_for_entity_center(target) {
        return point_rect_distance2(x, y, rect);
    }
    let dx = x - target.pos_x;
    let dy = y - target.pos_y;
    dx * dx + dy * dy
}

fn building_rect_for_entity_center(e: &Entity) -> Option<RectBody> {
    let stats = config::building_stats(e.kind)?;
    if !e.pos_x.is_finite() || !e.pos_y.is_finite() || stats.foot_w == 0 || stats.foot_h == 0 {
        return None;
    }
    let ts = config::TILE_SIZE as f32;
    let half_w = stats.foot_w as f32 * ts * 0.5;
    let half_h = stats.foot_h as f32 * ts * 0.5;
    Some(RectBody {
        min_x: e.pos_x - half_w,
        min_y: e.pos_y - half_h,
        max_x: e.pos_x + half_w,
        max_y: e.pos_y + half_h,
    })
}

fn point_rect_distance2(x: f32, y: f32, rect: RectBody) -> f32 {
    let nearest_x = x.clamp(rect.min_x, rect.max_x);
    let nearest_y = y.clamp(rect.min_y, rect.max_y);
    let dx = x - nearest_x;
    let dy = y - nearest_y;
    dx * dx + dy * dy
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
        events.entry(pid).or_default().push(Event::ArtilleryImpact {
            x,
            y,
            radius_tiles: config::ARTILLERY_OUTER_RADIUS_TILES,
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
            severity: NoticeSeverity::Alert,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::map::Map;
    use crate::protocol::terrain;

    fn open_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![(4, 4), (size - 5, size - 5)],
            base_sites: Vec::new(),
        }
    }

    fn visible_team_fog(map: &Map, entities: &EntityStore) -> Fog {
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2, 3], entities, map);
        fog
    }

    fn has_under_attack_notice(events: &HashMap<u32, Vec<Event>>, player: u32) -> bool {
        events.get(&player).is_some_and(|player_events| {
            player_events.iter().any(
                |event| matches!(event, Event::Notice { msg, .. } if msg == "alert:under_attack"),
            )
        })
    }

    fn mark_entrenched(entities: &mut EntityStore, id: u32) {
        entities
            .get_mut(id)
            .expect("entity should exist")
            .movement
            .as_mut()
            .expect("entity should have movement")
            .occupied_trench_id = Some(1);
    }

    #[test]
    fn artillery_outer_area_damage_is_reduced_against_entrenched_infantry() {
        let map = open_map(20);
        let mut entities = EntityStore::new();
        let victim = entities
            .spawn_unit(2, EntityKind::Rifleman, 224.0, 160.0)
            .expect("victim should spawn");
        mark_entrenched(&mut entities, victim);
        let before = entities.get(victim).expect("victim should exist").hp;
        let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
        let fog = visible_team_fog(&map, &entities);
        let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);
        let shell = ArtilleryShell {
            owner: 1,
            x: 160.0,
            y: 160.0,
            impact_tick: 0,
        };

        resolve_shell(&mut entities, &teams, &fog, &mut events, &shell, 10);

        let after = entities.get(victim).expect("victim should survive").hp;
        let inner = config::ARTILLERY_INNER_RADIUS_TILES * config::TILE_SIZE as f32;
        let outer = config::ARTILLERY_OUTER_RADIUS_TILES * config::TILE_SIZE as f32;
        let expected_base = artillery_damage(
            EntityKind::Rifleman,
            64.0_f32.powi(2),
            inner.powi(2),
            outer.powi(2),
        );
        let expected_damage =
            combat::area_damage_after_entrenchment(EntityKind::Rifleman, expected_base, true);
        assert_eq!(
            before - after,
            expected_damage,
            "entrenched infantry should take 75% of outer artillery splash"
        );
    }

    #[test]
    fn artillery_under_attack_notice_goes_to_victim_owner_not_teammate() {
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
