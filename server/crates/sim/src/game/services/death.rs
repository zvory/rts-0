use std::collections::HashMap;

use crate::config;
use crate::game::entity::{EntityKind, EntityStore, Order};
use crate::game::fog::{Fog, LingeringSightSource};
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::game::PlayerState;
use crate::protocol::Event;
use crate::rules::{economy, projection};

/// Remove entities whose hp has hit zero, emitting a fog-respecting `Death` event: a player
/// gets the poof only if they owned the entity or its death position is currently visible to
/// them (events are best-effort flavor). `death_system` runs before the fog recompute, so the
/// current fog still reflects who could see the unit while it was alive — exactly the players
/// who should see it die. A dead building refunds every prepaid unit and research item before its
/// queues are removed. Workers building a since-removed site are reset elsewhere.
#[allow(clippy::too_many_arguments)]
pub(crate) fn death_system(
    entities: &mut EntityStore,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    teams: &TeamRelations,
    players: &mut [PlayerState],
    lingering_sight: &mut Vec<LingeringSightSource>,
    events: &mut HashMap<u32, Vec<Event>>,
    tick: u32,
) {
    let dead: Vec<DeadEntity> = entities
        .iter()
        .filter(|e| e.is_targetable() && e.hp == 0)
        .map(|e| DeadEntity {
            id: e.id,
            owner: e.owner,
            x: e.pos_x,
            y: e.pos_y,
            sight_tiles: e.sight_tiles(),
            kind: e.kind,
            killer: e.last_damage_owner(),
            queued_units: e
                .prod_queue()
                .iter()
                .filter(|item| item.paid)
                .map(|item| item.unit)
                .collect(),
            queued_upgrades: e
                .research_queue()
                .iter()
                .filter(|item| item.paid)
                .map(|item| item.upgrade)
                .collect(),
        })
        .collect();

    for dead in dead {
        if let Some(player) = players.iter_mut().find(|player| player.id == dead.owner) {
            for unit in dead.queued_units {
                if config::unit_stats(unit).is_some() {
                    player.refund_cost(economy::resource_cost(unit));
                    player.release_supply(economy::supply_cost(unit));
                }
            }
            for queued_upgrade in dead.queued_upgrades {
                let definition = crate::game::upgrade::definition(queued_upgrade);
                player.refund_cost(economy::ResourceCost::new(
                    definition.cost_steel,
                    definition.cost_oil,
                ));
            }
        }
        entities.release_miner(dead.id);
        entities.remove(dead.id);
        record_score_death(players, dead.owner, dead.kind, dead.killer);
        if let Some(source) = LingeringSightSource::new(
            dead.owner,
            dead.x,
            dead.y,
            dead.sight_tiles,
            tick.saturating_add(config::TICK_HZ * 5),
        ) {
            lingering_sight.push(source);
        }
        // Deliver the death only to players who owned the entity or could see where it died,
        // so a death poof never reveals an entity hidden in a player's fog.
        let pids: Vec<u32> = events.keys().copied().collect();
        for pid in pids {
            if !projection::event_visible_to_team_with_smoke(
                pid, dead.x, dead.y, dead.owner, fog, teams, smokes,
            ) {
                continue;
            }
            events.entry(pid).or_default().push(Event::Death {
                id: dead.id,
                x: dead.x,
                y: dead.y,
                kind: crate::protocol::kind_to_wire(dead.kind).to_string(),
            });
        }
    }

    // Remove fully depleted resource nodes so they disappear from the world (and from
    // client snapshots). Gather orders pointing at a since-removed node self-heal via
    // the missing-node branches in `economy::gather_*`.
    let depleted: Vec<(u32, f32, f32, EntityKind)> = entities
        .iter()
        .filter(|e| e.is_node() && e.remaining().unwrap_or(0) == 0)
        .map(|e| (e.id, e.pos_x, e.pos_y, e.kind))
        .collect();
    for (id, x, y, kind) in depleted {
        let pids: Vec<u32> = events.keys().copied().collect();
        for pid in pids {
            if smokes.point_inside(x, y) || !projection::team_visible_world(pid, x, y, fog, teams) {
                continue;
            }
            events.entry(pid).or_default().push(Event::Death {
                id,
                x,
                y,
                kind: crate::protocol::kind_to_wire(kind).to_string(),
            });
        }
        entities.remove(id);
    }

    // Clear stale node reservations through the authoritative slot predicate.
    entities.clear_stale_miner_slots();

    // Clean up dangling orders that reference removed entities (build sites, attack targets)
    // so units don't keep stale combat intent. Gather orders self-heal via `retarget_or_idle`.
    for id in entities.ids() {
        let stale = {
            let Some(e) = entities.get(id) else { continue };
            match e.order() {
                Order::Attack(_) => e
                    .order()
                    .attack_target()
                    .map(|target| !entities.contains(target))
                    .unwrap_or(false),
                Order::Build(_) => e
                    .order()
                    .build_site()
                    .map(|site| !entities.contains(site))
                    .unwrap_or(false),
                Order::Deconstruct(_) => e
                    .order()
                    .deconstruct_target()
                    .map(|target| !entities.contains(target))
                    .unwrap_or(false),
                _ => false,
            }
        };
        if stale {
            if let Some(e) = entities.get_mut(id) {
                e.clear_active_order();
            }
        }
    }
}

struct DeadEntity {
    id: u32,
    owner: u32,
    x: f32,
    y: f32,
    sight_tiles: u32,
    kind: EntityKind,
    killer: Option<u32>,
    queued_units: Vec<EntityKind>,
    queued_upgrades: Vec<crate::game::upgrade::UpgradeKind>,
}

fn record_score_death(
    players: &mut [PlayerState],
    owner: u32,
    kind: EntityKind,
    killer: Option<u32>,
) {
    if let Some(player) = players.iter_mut().find(|p| p.id == owner) {
        player.record_entity_lost(kind);
    }
    let Some(killer) = killer.filter(|killer| *killer != owner) else {
        return;
    };
    if let Some(player) = players.iter_mut().find(|p| p.id == killer) {
        player.record_entity_killed(kind);
    }
}

#[cfg(test)]
mod tests;
