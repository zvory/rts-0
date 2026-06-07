use std::collections::{HashMap, HashSet};

use crate::config;
use crate::game::command::SimCommand;
use crate::game::entity::{
    BuildPhase, EntityKind, EntityStore, OrderIntent, ProdItem, WeaponSetup,
};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::construction::resumable_site_for_build_intent;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::movement::angle_delta;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::standability;
use crate::game::services::world_query;
use crate::game::PlayerState;
use crate::protocol::{Event, NoticeSeverity};
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
    fog: &Fog,
    pending: Vec<(u32, SimCommand)>,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    for (player, cmd) in pending {
        match cmd {
            SimCommand::Move {
                units,
                x,
                y,
                queued,
            } => {
                if queued {
                    append_queued_point_order(entities, player, units, x, y, false);
                    continue;
                }
                let valid: Vec<u32> = dedupe_cap_units(units)
                    .into_iter()
                    .filter(|id| {
                        owns_unit(entities, player, *id)
                            && !is_constructing(entities, *id)
                            && can_accept_move_order(entities, *id)
                    })
                    .collect();
                clear_queued_orders(entities, &valid);
                clear_staged_at_gun_setup(entities, &valid);
                coordinator.order_group_move(entities, player, &valid, (x, y), false);
            }
            SimCommand::AttackMove {
                units,
                x,
                y,
                queued,
            } => {
                if queued {
                    append_queued_point_order(entities, player, units, x, y, true);
                    continue;
                }
                let valid: Vec<u32> = dedupe_cap_units(units)
                    .into_iter()
                    .filter(|id| {
                        owns_unit(entities, player, *id)
                            && !is_constructing(entities, *id)
                            && can_accept_move_order(entities, *id)
                    })
                    .collect();
                clear_queued_orders(entities, &valid);
                clear_staged_at_gun_setup(entities, &valid);
                coordinator.order_group_move(entities, player, &valid, (x, y), true);
            }
            SimCommand::Attack {
                units,
                target,
                queued,
            } => {
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
                        Some(t) if world_query::is_enemy_targetable(t, player, id)
                            && fog.is_visible_world(player, t.pos_x, t.pos_y));
                    if !target_ok {
                        continue;
                    }
                    if deployed_at_gun_target_outside_arc(entities, id, target) {
                        continue;
                    }
                    if queued {
                        if !matches!(entities.get(id), Some(e) if e.can_attack()) {
                            continue;
                        }
                        if let Some(e) = entities.get_mut(id) {
                            e.append_queued_order(OrderIntent::attack(target));
                        }
                        continue;
                    }
                    if let Some(e) = entities.get_mut(id) {
                        e.clear_queued_orders();
                    }
                    clear_staged_at_gun_setup(entities, &[id]);
                    coordinator.order_attack(entities, id, target);
                }
            }
            SimCommand::SetupAtGuns { units, x, y } => {
                if !x.is_finite() || !y.is_finite() {
                    continue;
                }
                for id in dedupe_cap_units(units) {
                    if !owns_unit(entities, player, id) || is_constructing(entities, id) {
                        continue;
                    }
                    let Some(e) = entities.get_mut(id) else {
                        continue;
                    };
                    if e.kind != EntityKind::AtTeam {
                        continue;
                    }
                    let facing = (y - e.pos_y).atan2(x - e.pos_x);
                    if !facing.is_finite() {
                        continue;
                    }
                    entities.release_miner(id);
                    let Some(e) = entities.get_mut(id) else {
                        continue;
                    };
                    e.clear_orders();
                    e.set_path_goal(None);
                    if matches!(e.weapon_setup(), WeaponSetup::Packed) {
                        e.set_emplacement_facing(Some(facing));
                        e.set_desired_weapon_facing(facing);
                    } else {
                        e.set_pending_redeploy_facing(Some(facing));
                        e.set_weapon_setup(WeaponSetup::TearingDownToRedeploy {
                            ticks: config::AT_TEAM_SETUP_TICKS,
                        });
                    }
                    e.reset_gather_state();
                    let (px, py) = (e.pos_x, e.pos_y);
                    e.reset_stuck(px, py);
                }
            }
            SimCommand::TearDownAtGuns { units } => {
                for id in dedupe_cap_units(units) {
                    if !owns_unit(entities, player, id) || is_constructing(entities, id) {
                        continue;
                    }
                    let Some(e) = entities.get_mut(id) else {
                        continue;
                    };
                    if e.kind != EntityKind::AtTeam {
                        continue;
                    }
                    if matches!(
                        e.weapon_setup(),
                        WeaponSetup::SettingUp { .. } | WeaponSetup::Deployed
                    ) {
                        e.clear_orders();
                        e.set_path_goal(None);
                        e.set_weapon_setup(WeaponSetup::TearingDown {
                            ticks: config::AT_TEAM_SETUP_TICKS,
                        });
                    } else if matches!(e.weapon_setup(), WeaponSetup::Packed) {
                        e.set_emplacement_facing(None);
                        e.set_pending_redeploy_facing(None);
                    }
                }
            }
            SimCommand::Charge { units } => {
                if !player_has_completed_training_centre(entities, player) {
                    continue;
                }
                for id in dedupe_cap_units(units) {
                    if !owns_unit(entities, player, id) || is_constructing(entities, id) {
                        continue;
                    }
                    let Some(e) = entities.get_mut(id) else {
                        continue;
                    };
                    if e.kind == EntityKind::Rifleman && e.charge_cooldown_ticks() == 0 {
                        e.start_charge(config::RIFLEMAN_CHARGE_TICKS);
                        e.start_charge_cooldown(config::RIFLEMAN_CHARGE_COOLDOWN_TICKS);
                    }
                }
            }
            SimCommand::Gather {
                units,
                node,
                queued,
            } => {
                for id in dedupe_cap_units(units) {
                    if !owns_unit(entities, player, id) {
                        continue;
                    }
                    let is_worker =
                        matches!(entities.get(id), Some(e) if e.kind == EntityKind::Worker);
                    let node_ok = matches!(entities.get(node), Some(n)
                        if n.is_node() && n.remaining().unwrap_or(0) > 0);
                    if !is_worker || !node_ok {
                        continue;
                    }
                    if !world_query::resource_has_completed_mining_cc(entities, player, node) {
                        continue;
                    }
                    if matches!(entities.node_slot_holder(node), Some(holder) if holder != id) {
                        continue;
                    }
                    if queued {
                        if let Some(e) = entities.get_mut(id) {
                            e.append_queued_order(OrderIntent::gather(node));
                        }
                        continue;
                    }
                    if is_constructing(entities, id) {
                        continue;
                    }
                    if let Some(e) = entities.get_mut(id) {
                        e.clear_queued_orders();
                    }
                    coordinator.order_gather(entities, id, node);
                }
            }
            SimCommand::Build {
                worker,
                building,
                tile_x,
                tile_y,
                queued,
            } => {
                if queued {
                    append_queued_build_order(entities, player, worker, building, tile_x, tile_y);
                    continue;
                }
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
            SimCommand::SetRally {
                building,
                x,
                y,
                queued,
            } => {
                order_set_rally(map, entities, player, building, x, y, queued);
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

fn clear_queued_orders(entities: &mut EntityStore, ids: &[u32]) {
    for id in ids {
        if let Some(e) = entities.get_mut(*id) {
            e.clear_queued_orders();
        }
    }
}

fn append_queued_point_order(
    entities: &mut EntityStore,
    player: u32,
    units: Vec<u32>,
    x: f32,
    y: f32,
    attack_move: bool,
) {
    if !x.is_finite() || !y.is_finite() {
        return;
    }
    let intent = if attack_move {
        OrderIntent::attack_move_to(x, y)
    } else {
        OrderIntent::move_to(x, y)
    };
    for id in dedupe_cap_units(units) {
        if !owns_unit(entities, player, id) {
            continue;
        }
        if let Some(e) = entities.get_mut(id) {
            e.append_queued_order(intent.clone());
        }
    }
}

fn append_queued_build_order(
    entities: &mut EntityStore,
    player: u32,
    worker: u32,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) {
    if !owns_unit(entities, player, worker) {
        return;
    }
    if !matches!(entities.get(worker), Some(e) if e.kind == EntityKind::Worker) {
        return;
    }
    if let Some(e) = entities.get_mut(worker) {
        e.append_queued_order(OrderIntent::build(building, tile_x, tile_y));
    }
}

/// True if this unit is a worker that has already begun laying concrete — it cannot
/// be pulled away until the building finishes or is destroyed.
fn is_constructing(entities: &EntityStore, id: u32) -> bool {
    matches!(
        entities.get(id),
        Some(e) if matches!(e.build_phase(), Some(BuildPhase::Constructing { .. }))
    )
}

fn can_accept_move_order(entities: &EntityStore, id: u32) -> bool {
    matches!(
        entities.get(id),
        Some(e)
            if e.kind != EntityKind::AtTeam || matches!(e.weapon_setup(), WeaponSetup::Packed)
    )
}

fn clear_staged_at_gun_setup(entities: &mut EntityStore, ids: &[u32]) {
    for id in ids {
        let Some(e) = entities.get_mut(*id) else {
            continue;
        };
        if e.kind == EntityKind::AtTeam && matches!(e.weapon_setup(), WeaponSetup::Packed) {
            e.set_emplacement_facing(None);
            e.set_pending_redeploy_facing(None);
        }
    }
}

fn player_has_completed_training_centre(entities: &EntityStore, player: u32) -> bool {
    world_query::completed_building_kinds(entities, player).contains(&EntityKind::TrainingCentre)
}

fn deployed_at_gun_target_outside_arc(entities: &EntityStore, id: u32, target: u32) -> bool {
    let Some(attacker) = entities.get(id) else {
        return false;
    };
    if attacker.kind != EntityKind::AtTeam
        || !matches!(attacker.weapon_setup(), WeaponSetup::Deployed)
    {
        return false;
    }
    let Some(center) = attacker
        .emplacement_facing()
        .or_else(|| attacker.weapon_facing())
        .filter(|facing| facing.is_finite())
    else {
        return false;
    };
    let Some(target) = entities.get(target) else {
        return false;
    };
    let target_angle = (target.pos_y - attacker.pos_y).atan2(target.pos_x - attacker.pos_x);
    if !target_angle.is_finite() {
        return true;
    }
    angle_delta(center, target_angle).abs() > config::AT_GUN_FIELD_OF_FIRE_RAD * 0.5
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
    let can_resume_existing =
        resumable_site_for_build_intent(map, entities, player, building, tile_x, tile_y).is_some();
    if !can_resume_existing
        && !standability::building_site_clear_for_build_intent(
            map, entities, building, tile_x, tile_y, worker,
        )
    {
        notice(events, player, "Cannot build there");
        return;
    }

    let ps = match players.iter().find(|p| p.id == player) {
        Some(p) => p,
        None => return,
    };
    let (cost_steel, cost_oil) = rules::economy::cost(building);
    if ps.steel < cost_steel || ps.oil < cost_oil {
        notice(
            events,
            player,
            rules::economy::resource_shortage_notice(ps.steel, ps.oil, cost_steel, cost_oil),
        );
        return;
    }

    let built = coordinator.order_build(entities, worker, building, tile_x, tile_y);
    if !built {
        notice(events, player, "Cannot build there");
    } else if let Some(e) = entities.get_mut(worker) {
        e.clear_queued_orders();
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
        notice(
            events,
            player,
            rules::economy::resource_shortage_notice(ps.steel, ps.oil, cost_steel, cost_oil),
        );
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

/// Set a unit-producing building's rally point. Validates ownership and that the building is a
/// completed producer; sanitizes/clamps the point to the map. Invalid requests are ignored
/// silently (consistent with movement commands), so a hostile client cannot wedge the tick loop.
fn order_set_rally(
    map: &Map,
    entities: &mut EntityStore,
    player: u32,
    building: u32,
    x: f32,
    y: f32,
    _queued: bool,
) {
    let ok = matches!(entities.get(building), Some(b)
        if b.owner == player && b.is_building() && !b.under_construction()
        && !rules::economy::trainable_units(b.kind).is_empty());
    if !ok {
        return;
    }
    if !x.is_finite() || !y.is_finite() {
        return;
    }
    let max = (map.world_size_px() - 1.0).max(0.0);
    let rally = (x.clamp(0.0, max), y.clamp(0.0, max));
    if let Some(b) = entities.get_mut(building) {
        b.clear_rally_stages();
        b.set_rally_point(Some(rally));
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
        x: None,
        y: None,
        severity: NoticeSeverity::Info,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, EntityStore, Order, OrderIntent, WeaponSetup};
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
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1], &entities, &map);
        let mut events = HashMap::new();

        apply_commands(
            &map,
            &mut entities,
            &mut players,
            &spatial,
            &mut coordinator,
            &fog,
            vec![(
                1,
                SimCommand::Build {
                    worker,
                    building: EntityKind::Depot,
                    tile_x: 4,
                    tile_y: 4,
                    queued: false,
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
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1], &entities, &map);
        let mut events = HashMap::new();

        apply_commands(
            &map,
            &mut entities,
            &mut players,
            &spatial,
            &mut coordinator,
            &fog,
            vec![(
                1,
                SimCommand::Build {
                    worker,
                    building: EntityKind::Barracks,
                    tile_x: 8,
                    tile_y: 8,
                    queued: false,
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

    #[test]
    fn build_order_accepts_resuming_owned_scaffold() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let (site_x, site_y) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, 64.0, 64.0)
            .expect("worker should spawn");
        let scaffold = entities
            .spawn_building(1, EntityKind::Depot, site_x, site_y, false)
            .expect("scaffold should spawn");
        let spatial = SpatialIndex::build(&entities, map.size);
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(1024, 32);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
        let mut players = vec![player_state(1)];
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1], &entities, &map);
        let mut events = HashMap::new();

        apply_commands(
            &map,
            &mut entities,
            &mut players,
            &spatial,
            &mut coordinator,
            &fog,
            vec![(
                1,
                SimCommand::Build {
                    worker,
                    building: EntityKind::Depot,
                    tile_x: 4,
                    tile_y: 4,
                    queued: false,
                },
            )],
            &mut events,
        );

        let worker = entities.get(worker).expect("worker should remain alive");
        assert!(
            matches!(worker.order(), Order::Build(_)),
            "worker should accept the resume order"
        );
        assert_eq!(
            worker.order().build_intent_tile(),
            Some((EntityKind::Depot, 4, 4)),
            "resume order should keep the scaffold footprint intent"
        );
        assert_ne!(
            worker.path_goal(),
            None,
            "resume order should still path the worker to the scaffold"
        );
        assert!(
            entities
                .get(scaffold)
                .expect("scaffold should survive")
                .under_construction(),
            "existing scaffold should remain available for resume"
        );
        assert!(
            events.get(&1).is_none_or(Vec::is_empty),
            "resume order should not emit a placement failure notice"
        );
    }

    #[test]
    fn set_rally_stores_point_on_producer_and_rejects_others() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
        let barracks = entities
            .spawn_building(1, EntityKind::Barracks, bx, by, true)
            .expect("barracks should spawn");
        let (dx, dy) = footprint_center(&map, EntityKind::Depot, 12, 6);
        let depot = entities
            .spawn_building(1, EntityKind::Depot, dx, dy, true)
            .expect("depot should spawn");
        let (ex, ey) = footprint_center(&map, EntityKind::Barracks, 6, 12);
        let enemy_barracks = entities
            .spawn_building(2, EntityKind::Barracks, ex, ey, true)
            .expect("enemy barracks should spawn");

        apply(
            &map,
            &mut entities,
            vec![
                (
                    1,
                    SimCommand::SetRally {
                        building: barracks,
                        x: 100.0,
                        y: 200.0,
                        queued: false,
                    },
                ),
                // Depot trains nothing -> rejected.
                (
                    1,
                    SimCommand::SetRally {
                        building: depot,
                        x: 50.0,
                        y: 50.0,
                        queued: false,
                    },
                ),
                // Not the owner -> rejected.
                (
                    1,
                    SimCommand::SetRally {
                        building: enemy_barracks,
                        x: 10.0,
                        y: 10.0,
                        queued: false,
                    },
                ),
            ],
        );

        assert_eq!(
            entities.get(barracks).unwrap().rally_point(),
            Some((100.0, 200.0)),
            "owned producer should store the rally point"
        );
        assert_eq!(
            entities.get(depot).unwrap().rally_point(),
            None,
            "non-producer building should not accept a rally point"
        );
        assert_eq!(
            entities.get(enemy_barracks).unwrap().rally_point(),
            None,
            "rally on an enemy building should be ignored"
        );
    }

    #[test]
    fn set_rally_clamps_out_of_bounds_point() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
        let barracks = entities
            .spawn_building(1, EntityKind::Barracks, bx, by, true)
            .expect("barracks should spawn");

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::SetRally {
                    building: barracks,
                    x: 1.0e9,
                    y: -50.0,
                    queued: false,
                },
            )],
        );

        let max = map.world_size_px() - 1.0;
        assert_eq!(
            entities.get(barracks).unwrap().rally_point(),
            Some((max, 0.0)),
            "rally point should be clamped into the map bounds"
        );
    }

    #[test]
    fn queued_move_appends_until_cap_and_normal_move_clears_queue() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");

        let queued_moves = (0..10)
            .map(|i| {
                (
                    1,
                    SimCommand::Move {
                        units: vec![unit],
                        x: 120.0 + i as f32,
                        y: 140.0,
                        queued: true,
                    },
                )
            })
            .collect();
        apply(&map, &mut entities, queued_moves);

        let entity = entities.get(unit).expect("unit should exist");
        assert_eq!(
            entity.queued_orders().len(),
            8,
            "unit queue should enforce the phase-0 cap"
        );
        assert!(
            matches!(entity.order(), Order::Idle),
            "queued command should not interrupt the active order in phase 0"
        );

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Move {
                    units: vec![unit],
                    x: 200.0,
                    y: 220.0,
                    queued: false,
                },
            )],
        );

        let entity = entities.get(unit).expect("unit should exist");
        assert!(
            entity.queued_orders().is_empty(),
            "replacement move should clear queued intents"
        );
        assert!(
            matches!(entity.order(), Order::Move(_)),
            "replacement move should still issue the active order"
        );
    }

    #[test]
    fn stop_clears_active_order_and_queued_orders() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        {
            let entity = entities.get_mut(unit).expect("unit should exist");
            entity.set_order(Order::move_to(300.0, 300.0));
            entity.append_queued_order(OrderIntent::move_to(400.0, 400.0));
        }

        apply(
            &map,
            &mut entities,
            vec![(1, SimCommand::Stop { units: vec![unit] })],
        );

        let entity = entities.get(unit).expect("unit should exist");
        assert!(matches!(entity.order(), Order::Idle));
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn charge_requires_training_centre_and_filters_to_owned_riflemen() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, 120.0, 100.0)
            .expect("worker should spawn");
        let enemy_rifle = entities
            .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
            .expect("enemy rifleman should spawn");

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Charge {
                    units: vec![rifle, worker, enemy_rifle, rifle],
                },
            )],
        );

        assert_eq!(
            entities.get(rifle).unwrap().charge_ticks(),
            0,
            "charge should be locked before Training Centre is complete"
        );

        let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 6, 6);
        entities
            .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
            .expect("training centre should spawn");

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Charge {
                    units: vec![rifle, worker, enemy_rifle, rifle],
                },
            )],
        );

        assert_eq!(
            entities.get(rifle).unwrap().charge_ticks(),
            config::RIFLEMAN_CHARGE_TICKS
        );
        assert_eq!(
            entities.get(rifle).unwrap().charge_cooldown_ticks(),
            config::RIFLEMAN_CHARGE_COOLDOWN_TICKS
        );
        assert_eq!(
            entities.get(worker).unwrap().charge_ticks(),
            0,
            "non-riflemen in the selected list are ignored"
        );
        assert_eq!(
            entities.get(enemy_rifle).unwrap().charge_ticks(),
            0,
            "enemy riflemen are ignored"
        );
    }

    #[test]
    fn charge_respects_cooldown_before_allowing_reuse() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 6, 6);
        entities
            .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
            .expect("training centre should spawn");

        apply(
            &map,
            &mut entities,
            vec![(1, SimCommand::Charge { units: vec![rifle] })],
        );
        let first_charge_ticks = entities.get(rifle).unwrap().charge_ticks();
        let first_cooldown_ticks = entities.get(rifle).unwrap().charge_cooldown_ticks();

        apply(
            &map,
            &mut entities,
            vec![(1, SimCommand::Charge { units: vec![rifle] })],
        );
        assert_eq!(
            entities.get(rifle).unwrap().charge_ticks(),
            first_charge_ticks,
            "cooldown should block immediate charge reuse"
        );
        assert_eq!(
            entities.get(rifle).unwrap().charge_cooldown_ticks(),
            first_cooldown_ticks,
            "retrying during cooldown must not refresh the cooldown"
        );

        for _ in 0..config::RIFLEMAN_CHARGE_COOLDOWN_TICKS {
            entities.get_mut(rifle).unwrap().tick_charge_cooldown();
        }
        entities.get_mut(rifle).unwrap().tick_charge();

        apply(
            &map,
            &mut entities,
            vec![(1, SimCommand::Charge { units: vec![rifle] })],
        );
        assert_eq!(
            entities.get(rifle).unwrap().charge_ticks(),
            config::RIFLEMAN_CHARGE_TICKS,
            "charge should become available again after cooldown expiry"
        );
    }

    #[test]
    fn queued_move_ignores_stale_ids_and_invalid_coordinates() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Move {
                    units: vec![unit, 99_999],
                    x: f32::NAN,
                    y: 140.0,
                    queued: true,
                },
            )],
        );

        assert!(
            entities
                .get(unit)
                .expect("unit should exist")
                .queued_orders()
                .is_empty(),
            "invalid queued point should be ignored without appending or panicking"
        );
    }

    #[test]
    fn oversized_queued_unit_lists_are_deduped_and_capped_before_appending() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let owned = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("owned unit should spawn");
        let enemy = entities
            .spawn_unit(2, EntityKind::Rifleman, 130.0, 100.0)
            .expect("enemy unit should spawn");
        let mut units = vec![owned; 20_000];
        units.extend([99_999, enemy, owned]);

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Move {
                    units,
                    x: 180.0,
                    y: 180.0,
                    queued: true,
                },
            )],
        );

        assert_eq!(
            entities
                .get(owned)
                .expect("owned unit should exist")
                .queued_orders()
                .len(),
            1,
            "repeated ids, stale ids, and enemy ids should not multiply queued state"
        );
        assert!(
            entities
                .get(enemy)
                .expect("enemy unit should exist")
                .queued_orders()
                .is_empty(),
            "enemy ids in a hostile queued command must be ignored"
        );
    }

    #[test]
    fn queued_attack_and_gather_reject_dead_or_depleted_targets_before_appending() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cc_x + 16.0, cc_y)
            .expect("worker should spawn");
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, cc_x + 32.0, cc_y)
            .expect("rifleman should spawn");
        let target = entities
            .spawn_unit(2, EntityKind::Rifleman, cc_x + 96.0, cc_y)
            .expect("target should spawn");
        let node = entities
            .spawn_node(EntityKind::Steel, cc_x + 64.0, cc_y)
            .expect("node should spawn");
        entities.get_mut(target).expect("target should exist").hp = 0;
        if let Some(resource) = entities
            .get_mut(node)
            .expect("node should exist")
            .resource_node
            .as_mut()
        {
            resource.remaining = 0;
        }

        apply(
            &map,
            &mut entities,
            vec![
                (
                    1,
                    SimCommand::Attack {
                        units: vec![rifle],
                        target,
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::Gather {
                        units: vec![worker],
                        node,
                        queued: true,
                    },
                ),
            ],
        );

        assert!(
            entities
                .get(rifle)
                .expect("rifleman should exist")
                .queued_orders()
                .is_empty(),
            "dead attack targets should not create queued attack intents"
        );
        assert!(
            entities
                .get(worker)
                .expect("worker should exist")
                .queued_orders()
                .is_empty(),
            "depleted resources should not create queued gather intents"
        );
    }

    #[test]
    fn repeated_invalid_queued_builds_stay_bounded() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
            .expect("worker should spawn");
        let pending = (0..32)
            .map(|_| {
                (
                    1,
                    SimCommand::Build {
                        worker,
                        building: EntityKind::Depot,
                        tile_x: u32::MAX,
                        tile_y: u32::MAX,
                        queued: true,
                    },
                )
            })
            .collect();

        apply(&map, &mut entities, pending);

        assert_eq!(
            entities
                .get(worker)
                .expect("worker should exist")
                .queued_orders()
                .len(),
            8,
            "queued build intents should enforce the per-unit queue cap even when invalid"
        );
    }

    #[test]
    fn queued_rally_replaces_the_single_rally_point() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
        let barracks = entities
            .spawn_building(1, EntityKind::Barracks, bx, by, true)
            .expect("barracks should spawn");

        apply(
            &map,
            &mut entities,
            vec![
                (
                    1,
                    SimCommand::SetRally {
                        building: barracks,
                        x: 100.0,
                        y: 100.0,
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::SetRally {
                        building: barracks,
                        x: 200.0,
                        y: 200.0,
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::SetRally {
                        building: barracks,
                        x: 300.0,
                        y: 300.0,
                        queued: true,
                    },
                ),
            ],
        );

        assert_eq!(
            entities.get(barracks).unwrap().rally_point(),
            Some((300.0, 300.0)),
            "queued rally commands should still replace the one active rally point"
        );
        assert!(entities.get(barracks).unwrap().rally_stages().is_empty());

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::SetRally {
                    building: barracks,
                    x: 400.0,
                    y: 400.0,
                    queued: false,
                },
            )],
        );

        let barracks = entities.get(barracks).expect("barracks should exist");
        assert!(barracks.rally_stages().is_empty());
        assert_eq!(barracks.rally_point(), Some((400.0, 400.0)));
    }

    #[test]
    fn setup_at_guns_filters_mixed_selection_and_records_facing() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let at = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("at gun should spawn");
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, 120.0, 100.0)
            .expect("rifleman should spawn");
        let enemy_at = entities
            .spawn_unit(2, EntityKind::AtTeam, 140.0, 100.0)
            .expect("enemy at gun should spawn");

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::SetupAtGuns {
                    units: vec![at, rifle, enemy_at, at],
                    x: 100.0,
                    y: 140.0,
                },
            )],
        );

        let at = entities.get(at).expect("at gun should exist");
        assert_eq!(at.weapon_setup(), WeaponSetup::Packed);
        assert!(
            (at.emplacement_facing().unwrap_or_default() - std::f32::consts::FRAC_PI_2).abs()
                < 0.001,
            "setup command should store a finite facing toward the target point"
        );
        assert!(
            at.facing().abs() < 0.001,
            "setup command should not snap the AT gun body to the target facing"
        );
        assert_eq!(
            entities
                .get(rifle)
                .expect("rifleman should exist")
                .weapon_setup(),
            WeaponSetup::Packed,
            "non-AT units in the selected list are ignored"
        );
        assert_eq!(
            entities
                .get(enemy_at)
                .expect("enemy at gun should exist")
                .weapon_setup(),
            WeaponSetup::Packed,
            "enemy AT guns are ignored"
        );
    }

    #[test]
    fn teardown_at_guns_only_affects_setting_up_or_deployed_at_guns() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let deployed = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("at gun should spawn");
        let packed = entities
            .spawn_unit(1, EntityKind::AtTeam, 130.0, 100.0)
            .expect("at gun should spawn");
        entities
            .get_mut(deployed)
            .unwrap()
            .set_weapon_setup(WeaponSetup::Deployed);
        entities
            .get_mut(packed)
            .unwrap()
            .set_emplacement_facing(Some(1.0));

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::TearDownAtGuns {
                    units: vec![deployed, packed],
                },
            )],
        );

        assert!(matches!(
            entities.get(deployed).unwrap().weapon_setup(),
            WeaponSetup::TearingDown { .. }
        ));
        assert_eq!(
            entities.get(packed).unwrap().weapon_setup(),
            WeaponSetup::Packed
        );
        assert_eq!(
            entities.get(packed).unwrap().emplacement_facing(),
            None,
            "teardown should cancel a packed AT gun's staged setup facing"
        );
    }

    #[test]
    fn move_order_ignores_deployed_at_guns_without_tearing_down_or_rotating() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let deployed = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("at gun should spawn");
        let packed = entities
            .spawn_unit(1, EntityKind::AtTeam, 130.0, 100.0)
            .expect("at gun should spawn");
        {
            let at = entities.get_mut(deployed).unwrap();
            at.set_weapon_setup(WeaponSetup::Deployed);
            at.set_emplacement_facing(Some(0.25));
            at.set_facing(0.25);
            at.set_weapon_facing(0.25);
        }

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Move {
                    units: vec![deployed, packed],
                    x: 220.0,
                    y: 100.0,
                    queued: false,
                },
            )],
        );

        let deployed = entities.get(deployed).expect("at gun should exist");
        assert_eq!(
            deployed.weapon_setup(),
            WeaponSetup::Deployed,
            "move should not implicitly tear down deployed AT guns"
        );
        assert_eq!(
            deployed.facing(),
            0.25,
            "ignored move should not rotate deployed AT guns"
        );
        assert!(
            matches!(deployed.order(), Order::Idle),
            "ignored move should not replace the deployed AT gun order"
        );
        assert_eq!(
            deployed.path_goal(),
            None,
            "ignored move should not queue a movement path"
        );

        let packed = entities.get(packed).expect("packed at gun should exist");
        assert!(
            matches!(packed.order(), Order::Move(_)),
            "packed AT guns should still accept move orders"
        );
    }

    #[test]
    fn attack_move_order_ignores_deployed_at_guns_without_tearing_down_or_rotating() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let deployed = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("at gun should spawn");
        {
            let at = entities.get_mut(deployed).unwrap();
            at.set_weapon_setup(WeaponSetup::Deployed);
            at.set_emplacement_facing(Some(-0.5));
            at.set_facing(-0.5);
            at.set_weapon_facing(-0.5);
        }

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::AttackMove {
                    units: vec![deployed],
                    x: 220.0,
                    y: 100.0,
                    queued: false,
                },
            )],
        );

        let deployed = entities.get(deployed).expect("at gun should exist");
        assert_eq!(
            deployed.weapon_setup(),
            WeaponSetup::Deployed,
            "attack-move should not implicitly tear down deployed AT guns"
        );
        assert_eq!(
            deployed.facing(),
            -0.5,
            "ignored attack-move should not rotate deployed AT guns"
        );
        assert!(
            matches!(deployed.order(), Order::Idle),
            "ignored attack-move should not replace the deployed AT gun order"
        );
        assert_eq!(
            deployed.path_goal(),
            None,
            "ignored attack-move should not queue a movement path"
        );
    }

    #[test]
    fn deployed_at_gun_rejects_explicit_attack_outside_field_of_fire() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let at = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("at gun should spawn");
        let front_target = entities
            .spawn_unit(2, EntityKind::Tank, 220.0, 100.0)
            .expect("target should spawn");
        let side_target = entities
            .spawn_unit(2, EntityKind::Tank, 100.0, 220.0)
            .expect("target should spawn");
        {
            let at = entities.get_mut(at).unwrap();
            at.set_weapon_setup(WeaponSetup::Deployed);
            at.set_emplacement_facing(Some(0.0));
        }

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Attack {
                    units: vec![at],
                    target: side_target,
                    queued: false,
                },
            )],
        );
        assert!(
            !matches!(entities.get(at).unwrap().order(), Order::Attack(_)),
            "out-of-arc attack should be ignored for the deployed AT gun"
        );

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Attack {
                    units: vec![at],
                    target: front_target,
                    queued: false,
                },
            )],
        );
        assert!(
            matches!(entities.get(at).unwrap().order(), Order::Attack(_)),
            "in-arc attack should still be accepted"
        );
    }

    #[test]
    fn attack_command_rejects_hidden_targets() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let hidden_target = entities
            .spawn_unit(2, EntityKind::Tank, 420.0, 100.0)
            .expect("hidden target should spawn");

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Attack {
                    units: vec![rifle],
                    target: hidden_target,
                    queued: false,
                },
            )],
        );

        let rifle = entities.get(rifle).expect("rifleman should exist");
        assert!(
            !matches!(rifle.order(), Order::Attack(_)),
            "hidden target ids should not become attack orders"
        );
        assert_eq!(rifle.target_id(), None);
        assert_eq!(rifle.path_goal(), None);
    }

    #[test]
    fn train_resource_shortages_emit_specific_notices() {
        let map = flat_map(24);

        let mut oil_missing_entities = EntityStore::new();
        let (fx, fy) = footprint_center(&map, EntityKind::Factory, 6, 6);
        let factory = oil_missing_entities
            .spawn_building(1, EntityKind::Factory, fx, fy, true)
            .expect("factory should spawn");
        let mut oil_missing_players = vec![player_state(1), player_state(2)];
        oil_missing_players[0].oil = 0;
        let oil_missing_events = apply_with_players(
            &map,
            &mut oil_missing_entities,
            &mut oil_missing_players,
            vec![(
                1,
                SimCommand::Train {
                    building: factory,
                    unit: EntityKind::ScoutCar,
                },
            )],
        );
        assert!(
            matches!(
                oil_missing_events.get(&1).and_then(|events| events.first()),
                Some(Event::Notice { msg, .. }) if msg == "Not enough oil"
            ),
            "oil-gated units should emit the oil voice-line notice"
        );

        let mut steel_missing_entities = EntityStore::new();
        let (cx, cy) = footprint_center(&map, EntityKind::CityCentre, 6, 6);
        let city_centre = steel_missing_entities
            .spawn_building(1, EntityKind::CityCentre, cx, cy, true)
            .expect("city centre should spawn");
        let mut steel_missing_players = vec![player_state(1), player_state(2)];
        steel_missing_players[0].steel = 0;
        let steel_missing_events = apply_with_players(
            &map,
            &mut steel_missing_entities,
            &mut steel_missing_players,
            vec![(
                1,
                SimCommand::Train {
                    building: city_centre,
                    unit: EntityKind::Worker,
                },
            )],
        );
        assert!(
            matches!(
                steel_missing_events.get(&1).and_then(|events| events.first()),
                Some(Event::Notice { msg, .. }) if msg == "Not enough steel"
            ),
            "steel-only units should emit the steel voice-line notice"
        );
    }

    /// Run `apply_commands` with throwaway derived state for command-validation tests.
    fn apply(map: &Map, entities: &mut EntityStore, pending: Vec<(u32, SimCommand)>) {
        let mut players = vec![player_state(1), player_state(2)];
        let _ = apply_with_players(map, entities, &mut players, pending);
    }

    fn apply_with_players(
        map: &Map,
        entities: &mut EntityStore,
        players: &mut [PlayerState],
        pending: Vec<(u32, SimCommand)>,
    ) -> HashMap<u32, Vec<Event>> {
        let spatial = SpatialIndex::build(entities, map.size);
        let occ = Occupancy::build(map, entities);
        let mut pathing = PathingService::new(1024, 32);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, map, &occ, 1);
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], entities, map);
        let mut events = HashMap::new();
        apply_commands(
            map,
            entities,
            players,
            &spatial,
            &mut coordinator,
            &fog,
            pending,
            &mut events,
        );
        events
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
