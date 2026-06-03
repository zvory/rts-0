use std::collections::{HashMap, HashSet};

use crate::config;
use crate::game::command::SimCommand;
use crate::game::entity::{BuildPhase, EntityKind, EntityStore, ProdItem};
use crate::game::map::Map;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::standability;
use crate::game::services::world_query;
use crate::game::PlayerState;
use crate::protocol::Event;
use crate::rules;

/// Max unique unit ids honored per multi-unit command. Caps the per-id work a single command can
/// force, so a repeated/huge id list can't be used to stall the tick loop.
const MAX_UNITS_PER_COMMAND: usize = 256;

/// Drain + apply queued commands (validate ownership / cost / supply / tech / placement).
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_commands(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    pending: Vec<(u32, SimCommand)>,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    for (player, cmd) in pending {
        match cmd {
            SimCommand::Move { units, x, y } => {
                let valid: Vec<u32> = dedupe_cap_units(units)
                    .into_iter()
                    .filter(|id| {
                        owns_unit(entities, player, *id) && !is_constructing(entities, *id)
                    })
                    .collect();
                coordinator.order_group_move(entities, player, &valid, (x, y), false);
            }
            SimCommand::AttackMove { units, x, y } => {
                let valid: Vec<u32> = dedupe_cap_units(units)
                    .into_iter()
                    .filter(|id| {
                        owns_unit(entities, player, *id) && !is_constructing(entities, *id)
                    })
                    .collect();
                coordinator.order_group_move(entities, player, &valid, (x, y), true);
            }
            SimCommand::Attack { units, target } => {
                for id in dedupe_cap_units(units) {
                    if let Some(e) = entities.get(id) {
                        if !e.is_unit() || e.owner != player {
                            continue;
                        }
                    } else {
                        continue;
                    }
                    if is_constructing(entities, id) {
                        continue;
                    }
                    let target_ok = matches!(entities.get(target),
                        Some(t) if world_query::is_enemy_targetable(t, player, id));
                    if !target_ok {
                        continue;
                    }
                    coordinator.order_attack(entities, id, target);
                }
            }
            SimCommand::Gather { units, node } => {
                for id in dedupe_cap_units(units) {
                    if !owns_unit(entities, player, id) {
                        continue;
                    }
                    if is_constructing(entities, id) {
                        continue;
                    }
                    let is_worker =
                        matches!(entities.get(id), Some(e) if e.kind == EntityKind::Worker);
                    let node_ok = matches!(entities.get(node), Some(n)
                        if n.is_node() && n.remaining().unwrap_or(0) > 0);
                    if !is_worker || !node_ok {
                        continue;
                    }
                    if !world_query::resource_has_completed_mining_ic(entities, player, node) {
                        continue;
                    }
                    if matches!(world_query::node_holder(entities, node), Some(holder) if holder != id)
                    {
                        continue;
                    }
                    coordinator.order_gather(entities, id, node);
                }
            }
            SimCommand::Build {
                worker,
                building,
                tile_x,
                tile_y,
            } => {
                order_build(
                    map,
                    entities,
                    players,
                    spatial,
                    coordinator,
                    player,
                    worker,
                    building,
                    tile_x,
                    tile_y,
                    events,
                );
            }
            SimCommand::Train { building, unit } => {
                order_train(entities, players, player, building, unit, events);
            }
            SimCommand::Cancel { building } => {
                order_cancel(entities, players, player, building);
            }
            SimCommand::Stop { units } => {
                for id in dedupe_cap_units(units) {
                    if owns_unit(entities, player, id) && !is_constructing(entities, id) {
                        entities.release_miner(id);
                        if let Some(e) = entities.get_mut(id) {
                            e.clear_orders();
                            if let Some(w) = e.worker.as_mut() {
                                w.carry = None;
                            }
                        }
                    }
                }
            }
            SimCommand::Rejected { reason } => {
                notice(events, player, reason.notice_message());
            }
        }
    }
}

/// Dedupe a command's unit ids (preserving first-seen order) and cap the count at
/// `MAX_UNITS_PER_COMMAND`.
fn dedupe_cap_units(units: Vec<u32>) -> Vec<u32> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(units.len().min(MAX_UNITS_PER_COMMAND));
    for id in units {
        if out.len() >= MAX_UNITS_PER_COMMAND {
            break;
        }
        if seen.insert(id) {
            out.push(id);
        }
    }
    out
}

/// Whether `player` owns a *unit* with this id. Local re-export of
/// [`world_query::owns_unit`] to keep call sites in this module terse.
fn owns_unit(entities: &EntityStore, player: u32, id: u32) -> bool {
    world_query::owns_unit(entities, player, id)
}

/// True if this unit is a worker that has already begun laying concrete — it cannot
/// be pulled away until the building finishes or is destroyed.
fn is_constructing(entities: &EntityStore, id: u32) -> bool {
    matches!(
        entities.get(id),
        Some(e) if matches!(e.build_phase(), Some(BuildPhase::Constructing { .. }))
    )
}

/// Issue a build order under the "reserve on arrival" model. Validates intent, emits
/// best-effort feedback notices to the player, then walks the worker toward the target
/// tile. Resources are not deducted and no building is spawned here; that happens in the
/// construction system when the worker arrives, at which point placement and affordability
/// are re-checked. Other units may walk through the tile and other build commands may race
/// for it — first arrival wins.
#[allow(clippy::too_many_arguments)]
fn order_build(
    map: &Map,
    entities: &mut EntityStore,
    players: &[PlayerState],
    _spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    player: u32,
    worker: u32,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    if !owns_unit(entities, player, worker) {
        return;
    }
    if !matches!(entities.get(worker), Some(e) if e.kind == EntityKind::Worker) {
        notice(events, player, "Only workers can build");
        return;
    }
    if is_constructing(entities, worker) {
        return;
    }
    if config::building_stats(building).is_none() {
        notice(events, player, "Unknown building");
        return;
    }

    let owned = world_query::completed_building_kinds(entities, player);
    if !rules::economy::build_requirement_met(building, &owned) {
        notice(events, player, "Requirement not met");
        return;
    }

    if tile_x >= map.size || tile_y >= map.size {
        notice(events, player, "Cannot build there");
        return;
    }

    // Feedback only; construction repeats a stricter final-placement check at arrival.
    if !standability::building_site_clear_for_build_intent(
        map, entities, building, tile_x, tile_y, worker,
    ) {
        notice(events, player, "Cannot build there");
        return;
    }

    let ps = match players.iter().find(|p| p.id == player) {
        Some(p) => p,
        None => return,
    };
    let (cost_steel, cost_oil) = rules::economy::cost(building);
    if ps.steel < cost_steel || ps.oil < cost_oil {
        notice(events, player, "Not enough resources");
        return;
    }

    let built = coordinator.order_build(entities, worker, building, tile_x, tile_y);
    if !built {
        notice(events, player, "Cannot build there");
    }
}

/// Queue a unit at a production building. Reserves cost + supply on enqueue.
fn order_train(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    player: u32,
    building: u32,
    unit: EntityKind,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    let ok = matches!(entities.get(building), Some(b)
        if b.owner == player && b.is_building() && !b.under_construction()
        && rules::economy::trainable_units(b.kind).contains(&unit));
    if !ok {
        notice(events, player, "Cannot train that here");
        return;
    }
    let owned_complete = world_query::completed_building_kinds(entities, player);
    if !rules::economy::train_requirement_met(unit, &owned_complete) {
        notice(events, player, "Requirement not met");
        return;
    }
    let stats = match config::unit_stats(unit) {
        Some(s) => s,
        None => {
            notice(events, player, "Unknown unit");
            return;
        }
    };

    let ps = match players.iter_mut().find(|p| p.id == player) {
        Some(p) => p,
        None => return,
    };
    let (cost_steel, cost_oil) = rules::economy::cost(unit);
    let supply = rules::economy::supply_cost(unit);
    if ps.steel < cost_steel || ps.oil < cost_oil {
        notice(events, player, "Not enough resources");
        return;
    }
    if ps.supply_used + supply > ps.supply_cap {
        notice(events, player, "Not enough supply");
        return;
    }
    ps.steel -= cost_steel;
    ps.oil -= cost_oil;
    ps.supply_used += supply;

    if let Some(b) = entities.get_mut(building) {
        if let Some(queue) = b.prod_queue_mut() {
            queue.push(ProdItem {
                unit,
                progress: 0,
                total: stats.build_ticks,
            });
        }
    }
}

/// Cancel the front item of a building's production queue, refunding its cost + supply.
fn order_cancel(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    player: u32,
    building: u32,
) {
    let unit = {
        let b = match entities.get_mut(building) {
            Some(b) if b.owner == player && b.is_building() && !b.prod_queue().is_empty() => b,
            _ => return,
        };
        match b.prod_queue_mut() {
            Some(queue) => queue.remove(0).unit,
            None => return,
        }
    };
    if config::unit_stats(unit).is_some() {
        if let Some(ps) = players.iter_mut().find(|p| p.id == player) {
            let (cost_steel, cost_oil) = rules::economy::cost(unit);
            ps.steel += cost_steel;
            ps.oil += cost_oil;
            ps.supply_used = ps
                .supply_used
                .saturating_sub(rules::economy::supply_cost(unit));
        }
    }
}

/// Push a best-effort `Notice` event to a player.
pub(crate) fn notice(events: &mut HashMap<u32, Vec<Event>>, player: u32, msg: &str) {
    events.entry(player).or_default().push(Event::Notice {
        msg: msg.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, EntityStore, Order};
    use crate::game::map::Map;
    use crate::game::services::move_coordinator::MoveCoordinator;
    use crate::game::services::occupancy::{footprint_center, footprint_tiles, Occupancy};
    use crate::game::services::pathing::PathingService;
    use crate::game::services::spatial::SpatialIndex;
    use crate::game::ScoreState;
    use crate::protocol::terrain;

    #[test]
    fn build_order_can_start_when_worker_inside_intent_but_stages_outside() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let (wx, wy) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, wx, wy)
            .expect("worker should spawn");
        let spatial = SpatialIndex::build(&entities, map.size);
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(1024, 32);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
        let mut players = vec![player_state(1)];
        let mut events = HashMap::new();

        apply_commands(
            &map,
            &mut entities,
            &mut players,
            &spatial,
            &mut coordinator,
            vec![(
                1,
                SimCommand::Build {
                    worker,
                    building: EntityKind::Depot,
                    tile_x: 4,
                    tile_y: 4,
                },
            )],
            &mut events,
        );

        let worker = entities.get(worker).expect("worker should remain alive");
        assert!(
            matches!(worker.order(), Order::Build(_)),
            "worker should keep the accepted build order"
        );
        let goal = worker
            .path_goal()
            .expect("build order should set a staging goal");
        let goal_tile = map.tile_of(goal.0, goal.1);
        assert!(
            !footprint_tiles(EntityKind::Depot, 4, 4).contains(&goal_tile),
            "build-over-self order should stage outside the requested footprint"
        );
        assert!(
            events.get(&1).is_none_or(Vec::is_empty),
            "valid build-over-self intent should not emit a failure notice"
        );
    }

    #[test]
    fn build_order_does_not_pull_worker_off_active_construction() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let (site_x, site_y) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, site_x, site_y)
            .expect("worker should spawn");
        let site = entities
            .spawn_building(1, EntityKind::Depot, site_x, site_y, false)
            .expect("scaffold should spawn");
        let worker_entity = entities.get_mut(worker).expect("worker should exist");
        worker_entity.set_order(Order::build(EntityKind::Depot, 4, 4));
        worker_entity.mark_build_phase(BuildPhase::Constructing { site });
        worker_entity.set_target_id(Some(site));

        let spatial = SpatialIndex::build(&entities, map.size);
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(1024, 32);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
        let mut players = vec![player_state(1)];
        let mut events = HashMap::new();

        apply_commands(
            &map,
            &mut entities,
            &mut players,
            &spatial,
            &mut coordinator,
            vec![(
                1,
                SimCommand::Build {
                    worker,
                    building: EntityKind::Barracks,
                    tile_x: 8,
                    tile_y: 8,
                },
            )],
            &mut events,
        );

        let worker = entities.get(worker).expect("worker should remain alive");
        assert_eq!(
            worker.build_phase(),
            Some(BuildPhase::Constructing { site }),
            "active build command should keep constructing the original scaffold"
        );
        assert_eq!(
            worker.order().build_intent_tile(),
            Some((EntityKind::Depot, 4, 4)),
            "second build order must not replace the active construction intent"
        );
        assert_eq!(
            worker.target_id(),
            Some(site),
            "worker should stay latched to the scaffold it is building"
        );
        assert!(
            events.get(&1).is_none_or(Vec::is_empty),
            "ignored build command should not emit a failure notice"
        );
    }

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![],
            expansion_sites: vec![],
        }
    }

    fn player_state(id: u32) -> PlayerState {
        PlayerState {
            id,
            name: format!("Player {id}"),
            color: "#fff".to_string(),
            start_tile: (0, 0),
            steel: 1_000,
            oil: 1_000,
            supply_used: 0,
            supply_cap: 20,
            is_ai: false,
            score: ScoreState::default(),
        }
    }
}
