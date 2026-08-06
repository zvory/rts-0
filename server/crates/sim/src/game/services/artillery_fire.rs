//! Shared artillery shell launch mechanics for manual and autonomous fire orders.

use crate::config;
use crate::game::ability::{self, AbilityKind};
use crate::game::artillery::ArtilleryShellStore;
use crate::game::entity::{EntityKind, EntityStore, WeaponSetup};
use crate::game::firing_reveal::{
    record_global_firing_reveals_for_enemy_players, FiringRevealSource,
};
use crate::game::fog::Fog;
use crate::game::teams::TeamRelations;
use crate::game::upgrade::UpgradeKind;
use crate::game::PlayerState;
use crate::protocol::{self, AttackReveal, Event, NoticeSeverity};
use crate::rules::combat::WeaponKind;
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
pub(in crate::game) fn try_fire_artillery(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    teams: &TeamRelations,
    fog: &Fog,
    artillery_shells: &mut ArtilleryShellStore,
    firing_reveals: &mut Vec<FiringRevealSource>,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
    tick: u32,
    ability: AbilityKind,
    radius_tiles: f32,
) -> bool {
    if !matches!(ability, AbilityKind::PointFire | AbilityKind::BlanketFire) {
        return false;
    }
    let ready = matches!(entities.get(unit), Some(e)
        if e.owner == player
            && e.kind == EntityKind::Artillery
            && e.hp > 0
            && e.attack_cd() == 0
            && matches!(e.weapon_setup(), WeaponSetup::Deployed));
    if !ready {
        return false;
    }
    let faction_id = players
        .iter()
        .find(|candidate| candidate.id == player)
        .map(|candidate| candidate.faction_id.as_str())
        .unwrap_or(crate::rules::faction::DEFAULT_FACTION_ID);
    if !matches!(entities.get(unit), Some(entity)
        if crate::rules::faction::catalog_for(faction_id)
            .is_some_and(|catalog| catalog.allows_ability(ability, entity.kind)))
    {
        return false;
    }
    let ammo_cost = ability::definition(ability).cost;
    let Some(player_state) = players.iter_mut().find(|candidate| candidate.id == player) else {
        return false;
    };
    let min_fire_radius_tiles =
        artillery_min_fire_radius_tiles(player_state.has_upgrade(UpgradeKind::BallisticTables));
    if !player_state.can_afford(ammo_cost.steel, ammo_cost.oil) {
        notice(events, player, protocol::notices::ARTILLERY_STEEL_SHORTAGE);
        if let Some(entity) = entities.get_mut(unit) {
            entity.set_attack_cd(config::ARTILLERY_RELOAD_TICKS);
        }
        return false;
    }
    if !player_state.spend_cost(ammo_cost) {
        notice(events, player, protocol::notices::ARTILLERY_STEEL_SHORTAGE);
        return false;
    }
    let (target_x, target_y) = {
        let Some(entity) = entities.get_mut(unit) else {
            player_state.refund_cost(ammo_cost);
            return false;
        };
        let shot_number = entity.increment_artillery_blanket_shots_fired();
        entity.set_attack_cd(config::ARTILLERY_RELOAD_TICKS);
        let fire_radius_tiles = match ability {
            AbilityKind::PointFire => min_fire_radius_tiles,
            AbilityKind::BlanketFire => radius_tiles.clamp(
                min_fire_radius_tiles,
                config::ARTILLERY_BLANKET_RADIUS_TILES,
            ),
            _ => return false,
        };
        artillery_blanket_point(unit, player, tick, (x, y), shot_number, fire_radius_tiles)
    };
    let reveal = entities.get(unit).map(|attacker| AttackReveal {
        owner: attacker.owner,
        kind: protocol::kind_to_wire(attacker.kind).to_string(),
        x: attacker.pos_x,
        y: attacker.pos_y,
        facing: Some(attacker.facing()),
        weapon_facing: attacker.weapon_facing(),
        setup_state: Some(attacker.weapon_setup().to_protocol_str().to_string()),
    });
    artillery_shells.schedule(player, unit, target_x, target_y, tick);
    if let Some(reveal) = reveal.as_ref() {
        let facing = reveal.weapon_facing.or(reveal.facing).unwrap_or(0.0);
        let player_ids: Vec<u32> = events.keys().copied().collect();
        record_global_firing_reveals_for_enemy_players(
            firing_reveals,
            &player_ids,
            teams,
            player,
            unit,
            tick,
            config::ARTILLERY_RELOAD_TICKS,
        );
        for player_id in player_ids {
            events
                .entry(player_id)
                .or_default()
                .push(Event::ArtilleryFiring {
                    owner: reveal.owner,
                    x: reveal.x,
                    y: reveal.y,
                    facing,
                });
        }
    }
    for player_id in events.keys().copied().collect::<Vec<_>>() {
        if teams.same_team_or_same_owner(player_id, player) {
            events
                .entry(player_id)
                .or_default()
                .push(Event::ArtilleryTarget {
                    from: unit,
                    x: target_x,
                    y: target_y,
                    radius_tiles: config::ARTILLERY_OUTER_RADIUS_TILES,
                    delay_ticks: config::ARTILLERY_SHELL_DELAY_TICKS,
                });
        }
    }
    if let Some(reveal) = reveal {
        let player_ids: Vec<u32> = events.keys().copied().collect();
        for player_id in player_ids {
            if teams.same_team_or_same_owner(player_id, player)
                || !crate::rules::projection::team_visible_world(
                    player_id, reveal.x, reveal.y, fog, teams,
                )
            {
                continue;
            }
            events.entry(player_id).or_default().push(Event::Attack {
                from: unit,
                to: unit,
                reveal: Some(reveal.clone()),
                to_pos: None,
                weapon_kind: Some(WeaponKind::ArtilleryGun.stable_id().to_string()),
            });
        }
    }
    true
}

pub(in crate::game) fn artillery_min_fire_radius_tiles(has_fire_control: bool) -> f32 {
    if has_fire_control {
        config::ARTILLERY_FIRE_CONTROL_MIN_FIRE_RADIUS_TILES
    } else {
        config::ARTILLERY_MIN_FIRE_RADIUS_TILES
    }
}

type WorldPoint = (f32, f32);

pub(in crate::game) fn artillery_blanket_point(
    unit: u32,
    owner: u32,
    tick: u32,
    center: WorldPoint,
    shot_number: u16,
    fire_radius_tiles: f32,
) -> (f32, f32) {
    let radius_px = fire_radius_tiles.clamp(
        config::ARTILLERY_FIRE_CONTROL_MIN_FIRE_RADIUS_TILES,
        config::ARTILLERY_BLANKET_RADIUS_TILES,
    ) * config::TILE_SIZE as f32;
    let seed = mix32(
        unit.wrapping_mul(0x9E37_79B9)
            ^ owner.wrapping_mul(0x85EB_CA6B)
            ^ tick.rotate_left(7)
            ^ (shot_number as u32).wrapping_mul(0xC2B2_AE35),
    );
    offset_inside_circle(center, radius_px, seed)
}

fn offset_inside_circle(center: WorldPoint, radius_px: f32, seed: u32) -> WorldPoint {
    if radius_px <= f32::EPSILON {
        return center;
    }
    let angle = unit_float(seed) * std::f32::consts::TAU;
    let radial = unit_float(mix32(seed ^ 0xA5A5_5A5A)).sqrt() * radius_px;
    (
        center.0 + angle.cos() * radial,
        center.1 + angle.sin() * radial,
    )
}

fn mix32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^ (x >> 16)
}

fn unit_float(x: u32) -> f32 {
    ((x >> 8) as f32) / 16_777_215.0
}

fn notice(events: &mut HashMap<u32, Vec<Event>>, player: u32, msg: &str) {
    events.entry(player).or_default().push(Event::Notice {
        msg: msg.to_string(),
        x: None,
        y: None,
        severity: NoticeSeverity::Info,
    });
}
