use std::collections::HashMap;

use crate::config;
use crate::game::entity::{EntityKind, EntityStore, Order};
use crate::game::fog::{Fog, LingeringSightSource};
use crate::game::ground_decal::GroundDecalStore;
use crate::game::map::Map;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::game::PlayerState;
use crate::protocol::Event;
use crate::rules::{economy, projection};

/// Remove entities whose hp has hit zero, emitting a fog-respecting `Death` event: a player
/// gets the poof only if they owned the entity or its death position is currently visible to
/// them (events are best-effort flavor). `death_system` runs before the fog recompute, so the
/// current fog still reflects who could see the unit while it was alive — exactly the players
/// who should see it die. A dead building refunds prepaid units and research before its queues are
/// removed, except for linked extractor construction that dies with its producer. Workers building
/// a since-removed site are reset elsewhere.
#[allow(clippy::too_many_arguments)]
pub(crate) fn death_system(
    map: &Map,
    entities: &mut EntityStore,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    teams: &TeamRelations,
    players: &mut [PlayerState],
    lingering_sight: &mut Vec<LingeringSightSource>,
    ground_decals: &mut GroundDecalStore,
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
            facing: e.facing(),
            weapon_facing: e.weapon_facing(),
            killer: e.last_damage_owner(),
            construction_producer: e.construction_producer_id(),
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
        let linked_extractor_scaffolds = entities
            .iter()
            .filter(|entity| entity.construction_producer_id() == Some(dead.id))
            .map(|entity| (entity.id, entity.kind))
            .collect::<Vec<_>>();
        if let Some(producer_id) = dead.construction_producer {
            let lost_item = entities.get_mut(producer_id).and_then(|producer| {
                (producer.owner == dead.owner
                    && producer
                        .prod_queue()
                        .first()
                        .is_some_and(|front| front.unit == dead.kind))
                .then(|| producer.remove_front_production())
                .flatten()
            });
            if let Some(item) = lost_item {
                if let Some(player) = players.iter_mut().find(|player| player.id == dead.owner) {
                    player.release_supply(economy::supply_cost(item.unit));
                }
            }
        }
        if let Some(player) = players.iter_mut().find(|player| player.id == dead.owner) {
            for unit in dead.queued_units {
                let linked_construction_died = linked_extractor_scaffolds
                    .iter()
                    .any(|(_, scaffold_kind)| *scaffold_kind == unit);
                if !linked_construction_died
                    && (config::unit_stats(unit).is_some() || unit.is_resource_extractor())
                {
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
        for (scaffold, _) in linked_extractor_scaffolds {
            entities.remove(scaffold);
        }
        record_score_death(players, dead.owner, dead.kind, dead.killer);
        let concealed_unit =
            config::unit_stats(dead.kind).is_some() && map.world_point_is_stealth(dead.x, dead.y);
        if !concealed_unit {
            ground_decals.create_death(
                dead.kind,
                dead.x,
                dead.y,
                dead.owner,
                Some(dead.facing),
                dead.weapon_facing,
            );
        }
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
            let hidden_from_team = concealed_unit
                && !teams.same_team_or_same_owner(pid, dead.owner)
                && !teams
                    .same_team_player_ids(pid)
                    .into_iter()
                    .any(|player| fog.active_firing_reveal_episode(player, dead.id).is_some());
            if hidden_from_team {
                continue;
            }
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
        let (stale, next_cluster_target) = {
            let Some(e) = entities.get(id) else { continue };
            match e.order() {
                Order::Attack(order) => {
                    let stale = !entities.contains(order.intent.target);
                    let next = stale
                        .then(|| {
                            order
                                .intent
                                .remaining_targets
                                .iter()
                                .copied()
                                .find(|target| {
                                    entities.get(*target).is_some_and(|target| {
                                        target.hp > 0 && target.is_neutral_obstacle()
                                    })
                                })
                        })
                        .flatten();
                    (stale, next)
                }
                Order::Build(_) => e
                    .order()
                    .build_site()
                    .map(|site| !entities.contains(site))
                    .map(|stale| (stale, None))
                    .unwrap_or((false, None)),
                Order::Deconstruct(_) => e
                    .order()
                    .deconstruct_target()
                    .map(|target| !entities.contains(target))
                    .map(|stale| (stale, None))
                    .unwrap_or((false, None)),
                _ => (false, None),
            }
        };
        if stale {
            if let Some(e) = entities.get_mut(id) {
                if !next_cluster_target
                    .is_some_and(|target| e.advance_attack_cluster_target(target))
                {
                    e.clear_active_order();
                }
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
    facing: f32,
    weapon_facing: Option<f32>,
    killer: Option<u32>,
    construction_producer: Option<u32>,
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
