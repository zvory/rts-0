use std::collections::HashMap;

use crate::config;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::dist2;
use crate::protocol::Event;
use crate::rules::combat;
use crate::rules::projection;
use crate::rules::terrain::TerrainKind;

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

impl MortarShellStore {
    pub(crate) fn schedule(&mut self, owner: u32, attacker: u32, x: f32, y: f32, tick: u32) {
        self.shells.push(MortarShell {
            owner,
            attacker,
            x,
            y,
            impact_tick: tick.saturating_add(config::MORTAR_SHELL_DELAY_TICKS),
        });
    }

    pub(crate) fn resolve_due(
        &mut self,
        map: &Map,
        entities: &mut EntityStore,
        fog: &Fog,
        events: &mut HashMap<u32, Vec<Event>>,
        tick: u32,
    ) {
        let mut pending = Vec::new();
        let due = std::mem::take(&mut self.shells);
        for shell in due {
            if shell.impact_tick <= tick {
                resolve_shell(map, entities, fog, events, &shell, tick);
            } else {
                pending.push(shell);
            }
        }
        self.shells = pending;
    }
}

fn resolve_shell(
    _map: &Map,
    entities: &mut EntityStore,
    fog: &Fog,
    events: &mut HashMap<u32, Vec<Event>>,
    shell: &MortarShell,
    tick: u32,
) {
    let outer_radius = config::MORTAR_OUTER_RADIUS_TILES * config::TILE_SIZE as f32;
    let inner_radius = config::MORTAR_INNER_RADIUS_TILES * config::TILE_SIZE as f32;
    let outer2 = outer_radius * outer_radius;
    let inner2 = inner_radius * inner_radius;
    let attacker_alive = matches!(
        entities.get(shell.attacker),
        Some(e) if e.owner == shell.owner && e.kind == EntityKind::MortarTeam && e.hp > 0
    );
    if !attacker_alive {
        return;
    }

    emit_impact(events, fog, shell.owner, shell.x, shell.y);

    let mut hits = Vec::new();
    for id in entities.ids() {
        let Some(target) = entities.get(id) else {
            continue;
        };
        if target.owner == shell.owner || target.owner == 0 || target.hp == 0 || target.is_node() {
            continue;
        }
        let d2 = dist2(shell.x, shell.y, target.pos_x, target.pos_y);
        if d2 <= outer2 {
            let base = if d2 <= inner2 {
                config::MORTAR_INNER_DAMAGE
            } else {
                config::MORTAR_OUTER_DAMAGE
            };
            hits.push((id, base, target.owner, target.pos_x, target.pos_y));
        }
    }
    hits.sort_by_key(|(id, _, _, _, _)| *id);
    for (id, base, victim_owner, tx, ty) in hits {
        let effective = entities
            .get(id)
            .map(|target| mortar_damage(target.kind, base))
            .unwrap_or(0);
        if effective == 0 {
            continue;
        }
        let damaged = entities.get_mut(id).is_some_and(|target| {
            target.apply_damage(effective, Some((shell.owner, (shell.x, shell.y), tick)))
        });
        if damaged {
            push_under_attack_notice(events, fog, shell.owner, victim_owner, tx, ty);
        }
    }
}

fn mortar_damage(victim_kind: EntityKind, base: u32) -> u32 {
    if !combat::is_armored(victim_kind) {
        return combat::effective_damage(
            EntityKind::MortarTeam,
            victim_kind,
            base,
            Some(TerrainKind::Open),
        );
    }
    ((base as f32) * 0.625).round() as u32
}

fn emit_impact(events: &mut HashMap<u32, Vec<Event>>, fog: &Fog, owner: u32, x: f32, y: f32) {
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for pid in player_ids {
        if pid != owner && !fog.is_visible_world(pid, x, y) {
            continue;
        }
        events.entry(pid).or_default().push(Event::MortarImpact {
            x,
            y,
            radius_tiles: config::MORTAR_OUTER_RADIUS_TILES,
        });
    }
}

fn push_under_attack_notice(
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    attacker_owner: u32,
    victim_owner: u32,
    x: f32,
    y: f32,
) {
    if victim_owner == 0 || victim_owner == attacker_owner {
        return;
    }
    if !projection::event_visible_to(victim_owner, x, y, attacker_owner, fog) {
        return;
    }
    events.entry(victim_owner).or_default().push(Event::Notice {
        msg: "alert:under_attack".to_string(),
        x: Some(x),
        y: Some(y),
        severity: crate::protocol::NoticeSeverity::Alert,
    });
}
