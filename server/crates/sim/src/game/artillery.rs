use std::collections::HashMap;

use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::game::services::geometry::RectBody;
use crate::protocol::Event;
use crate::rules::combat;
use crate::rules::terrain::TerrainKind;

#[derive(Debug, Clone)]
struct ArtilleryShell {
    owner: u32,
    x: f32,
    y: f32,
    impact_tick: u32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ArtilleryShellStore {
    shells: Vec<ArtilleryShell>,
}

impl ArtilleryShellStore {
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
        events: &mut HashMap<u32, Vec<Event>>,
        tick: u32,
    ) {
        let mut pending = Vec::new();
        let due = std::mem::take(&mut self.shells);
        for shell in due {
            if shell.impact_tick <= tick {
                resolve_shell(entities, events, &shell, tick);
            } else {
                pending.push(shell);
            }
        }
        self.shells = pending;
    }
}

fn resolve_shell(
    entities: &mut EntityStore,
    events: &mut HashMap<u32, Vec<Event>>,
    shell: &ArtilleryShell,
    tick: u32,
) {
    emit_impact(events, shell.x, shell.y);

    let outer_radius = config::ARTILLERY_OUTER_RADIUS_TILES * config::TILE_SIZE as f32;
    let inner_radius = config::ARTILLERY_INNER_RADIUS_TILES * config::TILE_SIZE as f32;
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
        let damage = artillery_damage(target.kind, d2, inner2, outer2);
        if damage == 0 {
            continue;
        }
        if let Some(target) = entities.get_mut(id) {
            target.apply_damage(damage, Some((shell.owner, (shell.x, shell.y), tick)));
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

fn emit_impact(events: &mut HashMap<u32, Vec<Event>>, x: f32, y: f32) {
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for pid in player_ids {
        events.entry(pid).or_default().push(Event::ArtilleryImpact {
            x,
            y,
            radius_tiles: config::ARTILLERY_OUTER_RADIUS_TILES,
        });
    }
}
