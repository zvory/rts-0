use std::collections::{BTreeSet, HashMap};

use crate::config;
use crate::game::entity::tank_trap_deconstruction_ticks;
use crate::game::entity::{BuildPhase, DeconstructPhase, EntityKind, EntityStore, Order};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::occupancy::{footprint_center, Occupancy};
use crate::game::services::standability;
use crate::game::services::{dist2, interact_range_for_kind};
use crate::game::teams::TeamRelations;
use crate::game::PlayerState;
use crate::protocol::{Event, NoticeSeverity};
use crate::rules;
use crate::rules::projection;

mod pump_jack;

/// Advance build orders. Workers in `ToSite` or `WaitingAtSite` that are in arrival range of
/// their intended footprint re-validate placement and affordability, spawn the building, deduct
/// cost, or keep waiting for resources / short-lived unit blockers. Workers in `Constructing`
/// accumulate one tick of progress per tick; on completion the building leaves CONSTRUCT, the
/// worker is freed, and a `Build` event fires to the owner.
pub(crate) fn construction_system(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    active_construction_sites: &mut BTreeSet<u32>,
) {
    let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
    // ----- Arrival pass: workers that have reached and may be waiting at their target -----
    let arrivals: Vec<(u32, EntityKind, u32, u32, BuildPhase)> = entities
        .iter()
        .filter_map(|e| {
            if e.hp == 0 || !e.is_unit() {
                return None;
            }
            let phase = e.build_phase()?;
            if !matches!(phase, BuildPhase::ToSite | BuildPhase::WaitingAtSite) {
                return None;
            }
            let (kind, tx, ty) = e.order().build_intent_tile()?;
            let (cx, cy) = footprint_center(map, kind, tx, ty);
            let arrive = interact_range_for_kind(kind);
            if dist2(e.pos_x, e.pos_y, cx, cy).sqrt() <= arrive {
                Some((e.id, kind, tx, ty, phase))
            } else {
                None
            }
        })
        .collect();

    for (worker, kind, tx, ty, phase) in arrivals {
        let owner = match entities.get(worker) {
            Some(w) => w.owner,
            None => continue,
        };

        if let Some(site) = resumable_site_for_build_intent(map, entities, owner, kind, tx, ty) {
            if let Some(w) = entities.get_mut(worker) {
                w.clear_path();
                w.set_target_id(Some(site));
                w.mark_build_phase(BuildPhase::Constructing { site });
            }
            eject_worker_from_static_overlap(map, entities, worker);
            continue;
        }

        let owner_faction = players
            .iter()
            .find(|p| p.id == owner)
            .map(|p| p.faction_id.as_str())
            .unwrap_or(rules::faction::DEFAULT_FACTION_ID);
        if config::building_stats(kind).is_none()
            || !rules::economy::build_requirement_met_for_faction(
                owner_faction,
                kind,
                &crate::game::services::world_query::completed_building_kinds(entities, owner),
            )
        {
            if let Some(w) = entities.get_mut(worker) {
                w.clear_active_order();
            }
            continue;
        }

        let was_unit_blocked_wait = build_wait_unit_blocked_ticks(entities, worker) > 0;

        // Re-validate placement against the live entity set.
        let mut site_status = standability::building_site_status_for_build_intent(
            map, entities, kind, tx, ty, worker,
        );
        let can_eject_friendly_blockers = kind == EntityKind::PumpJack
            && site_status == standability::BuildSiteStatus::BlockedByUnit
            && pump_jack::all_site_unit_blockers_are_friendly(
                entities, &teams, owner, worker, tx, ty,
            );
        if can_eject_friendly_blockers {
            // Treat friendly bodies as provisionally clear. They are moved only after the
            // owner can afford to start, immediately before the final placement check.
            site_status = standability::BuildSiteStatus::Clear;
        }
        match site_status {
            standability::BuildSiteStatus::Clear => {
                if !can_eject_friendly_blockers {
                    reset_build_unit_blocked_timer(entities, worker);
                }
            }
            standability::BuildSiteStatus::BlockedByUnit => {
                let timed_out = mark_build_waiting_on_unit_blocker(entities, worker);
                if timed_out {
                    notice_build_failure(events, owner, "Cannot build there");
                    if let Some(w) = entities.get_mut(worker) {
                        w.clear_active_order();
                    }
                }
                continue;
            }
            standability::BuildSiteStatus::BlockedByBuilding
            | standability::BuildSiteStatus::BlockedByResourceNode
            | standability::BuildSiteStatus::InvalidFootprint => {
                notice_build_failure(events, owner, "Cannot build there");
                if let Some(w) = entities.get_mut(worker) {
                    w.clear_active_order();
                }
                continue;
            }
        }

        let cost = rules::economy::resource_cost(kind);
        let player_index = players.iter().position(|p| p.id == owner);
        let Some(player_index) = player_index else {
            mark_build_waiting_for_resources(entities, worker);
            continue;
        };
        if !players[player_index].can_afford(cost.steel, cost.oil) {
            if phase == BuildPhase::ToSite || was_unit_blocked_wait {
                notice_build_failure(
                    events,
                    owner,
                    rules::economy::resource_shortage_notice_for_cost(
                        players[player_index].steel,
                        players[player_index].oil,
                        cost,
                    ),
                );
            }
            mark_build_waiting_for_resources(entities, worker);
            continue;
        }

        if can_eject_friendly_blockers {
            pump_jack::eject_friendly_units_from_site(map, entities, &teams, owner, worker, tx, ty);
            match standability::building_site_status_for_build_intent(
                map, entities, kind, tx, ty, worker,
            ) {
                standability::BuildSiteStatus::Clear => {
                    reset_build_unit_blocked_timer(entities, worker);
                }
                standability::BuildSiteStatus::BlockedByUnit => {
                    let timed_out = mark_build_waiting_on_unit_blocker(entities, worker);
                    if timed_out {
                        notice_build_failure(events, owner, "Cannot build there");
                        if let Some(w) = entities.get_mut(worker) {
                            w.clear_active_order();
                        }
                    }
                    continue;
                }
                standability::BuildSiteStatus::BlockedByBuilding
                | standability::BuildSiteStatus::BlockedByResourceNode
                | standability::BuildSiteStatus::InvalidFootprint => {
                    notice_build_failure(events, owner, "Cannot build there");
                    if let Some(w) = entities.get_mut(worker) {
                        w.clear_active_order();
                    }
                    continue;
                }
            }
        }

        if !players[player_index].spend_cost(cost) {
            mark_build_waiting_for_resources(entities, worker);
            continue;
        }

        let (cx, cy) = footprint_center(map, kind, tx, ty);
        let site = match entities.spawn_building(owner, kind, cx, cy, false) {
            Some(id) => id,
            None => {
                players[player_index].refund_cost(cost);
                mark_build_waiting_for_resources(entities, worker);
                continue;
            }
        };
        let marked_paid = entities
            .get_mut(site)
            .is_some_and(|building| building.mark_construction_cost_paid());
        if !marked_paid {
            entities.remove(site);
            players[player_index].refund_cost(cost);
            mark_build_waiting_for_resources(entities, worker);
            continue;
        }
        players[player_index].record_entity_created(kind);
        if let Some(w) = entities.get_mut(worker) {
            w.clear_path();
            w.set_target_id(Some(site));
            w.mark_build_phase(BuildPhase::Constructing { site });
        }
        eject_worker_from_static_overlap(map, entities, worker);
    }

    // ----- Progress pass: workers actively constructing -----
    let working: Vec<(u32, u32)> = entities
        .iter()
        .filter_map(|e| {
            if e.hp == 0 || !e.is_unit() {
                return None;
            }
            match e.build_phase()? {
                BuildPhase::Constructing { site } => Some((e.id, site)),
                BuildPhase::ToSite | BuildPhase::WaitingAtSite => None,
            }
        })
        .collect();

    for (worker, site) in working {
        let completed = {
            let b = match entities.get_mut(site) {
                Some(b) if b.hp > 0 && b.under_construction() => b,
                _ => {
                    if let Some(w) = entities.get_mut(worker) {
                        w.clear_active_order();
                    }
                    continue;
                }
            };
            let completed = b.advance_construction().unwrap_or(false);
            active_construction_sites.insert(site);
            completed
        };
        if completed {
            let (owner, kind, x, y) = entities
                .get(site)
                .map(|b| (b.owner, b.kind, b.pos_x, b.pos_y))
                .unwrap_or((0, EntityKind::Worker, 0.0, 0.0));
            for pid in events.keys().copied().collect::<Vec<_>>() {
                if !teams.same_team_or_same_owner(pid, owner)
                    && !projection::team_visible_world(pid, x, y, fog, &teams)
                {
                    continue;
                }
                events.entry(pid).or_default().push(Event::Build {
                    id: site,
                    kind: crate::protocol::kind_to_wire(kind).to_string(),
                });
            }
            let builders = builders_for_site(entities, site);
            for builder in &builders {
                eject_worker_from_static_overlap(map, entities, *builder);
            }
            clear_build_orders(entities, &builders);
        }
    }
}

fn builders_for_site(entities: &EntityStore, site: u32) -> Vec<u32> {
    entities
        .iter()
        .filter(|entity| {
            entity.hp > 0 && entity.is_unit() && entity.order().build_site() == Some(site)
        })
        .map(|entity| entity.id)
        .collect()
}

fn clear_build_orders(entities: &mut EntityStore, builders: &[u32]) {
    for builder in builders {
        if let Some(worker) = entities.get_mut(*builder) {
            worker.clear_active_order();
        }
    }
}

fn notice_build_failure(events: &mut HashMap<u32, Vec<Event>>, owner: u32, msg: impl Into<String>) {
    events.entry(owner).or_default().push(Event::Notice {
        msg: msg.into(),
        x: None,
        y: None,
        severity: NoticeSeverity::Info,
    });
}

fn mark_build_waiting_for_resources(entities: &mut EntityStore, worker: u32) {
    if let Some(w) = entities.get_mut(worker) {
        w.clear_path();
        w.set_target_id(None);
        w.mark_build_phase(BuildPhase::WaitingAtSite);
        w.update_build_unit_blocked(false);
    }
}

fn mark_build_waiting_on_unit_blocker(entities: &mut EntityStore, worker: u32) -> bool {
    let Some(w) = entities.get_mut(worker) else {
        return false;
    };
    w.clear_path();
    w.set_target_id(None);
    w.mark_build_phase(BuildPhase::WaitingAtSite);
    w.update_build_unit_blocked(true).unwrap_or(false)
}

fn reset_build_unit_blocked_timer(entities: &mut EntityStore, worker: u32) {
    if let Some(w) = entities.get_mut(worker) {
        w.update_build_unit_blocked(false);
    }
}

fn build_wait_unit_blocked_ticks(entities: &EntityStore, worker: u32) -> u32 {
    let Some(entity) = entities.get(worker) else {
        return 0;
    };
    match entity.order() {
        Order::Build(order) if order.execution.phase == BuildPhase::WaitingAtSite => {
            order.execution.unit_blocked_ticks
        }
        _ => 0,
    }
}

/// Advance Tank Trap deconstruction orders. A worker must first reach the target trap, then spends
/// half of the trap's normal build time dismantling it. Completion refunds the trap cost to the
/// worker's owner and leaves removal/event fanout to the ordinary death system.
pub(crate) fn deconstruction_system(entities: &mut EntityStore, players: &mut [PlayerState]) {
    let arrivals: Vec<(u32, Option<u32>)> = entities
        .iter()
        .filter_map(|e| {
            if e.hp == 0
                || !e.is_unit()
                || e.deconstruct_phase() != Some(DeconstructPhase::ToTarget)
            {
                return None;
            }
            let target = e.order().deconstruct_target()?;
            let Some((tx, ty)) = live_completed_tank_trap_position(entities, target) else {
                return Some((e.id, None));
            };
            let arrive = interact_range_for_kind(EntityKind::TankTrap);
            if dist2(e.pos_x, e.pos_y, tx, ty).sqrt() <= arrive {
                Some((e.id, Some(target)))
            } else {
                None
            }
        })
        .collect();

    for (worker, target) in arrivals {
        let Some(target) = target else {
            if let Some(w) = entities.get_mut(worker) {
                w.clear_active_order();
            }
            continue;
        };
        if let Some(w) = entities.get_mut(worker) {
            w.clear_path();
            w.set_target_id(Some(target));
            w.mark_deconstruct_phase(DeconstructPhase::Deconstructing);
        }
    }

    let working: Vec<(u32, u32)> = entities
        .iter()
        .filter_map(|e| {
            if e.hp == 0
                || !e.is_unit()
                || e.deconstruct_phase() != Some(DeconstructPhase::Deconstructing)
            {
                return None;
            }
            Some((e.id, e.order().deconstruct_target()?))
        })
        .collect();
    let required_ticks = tank_trap_deconstruction_ticks();

    for (worker, target) in working {
        if live_completed_tank_trap_position(entities, target).is_none() {
            if let Some(w) = entities.get_mut(worker) {
                w.clear_active_order();
            }
            continue;
        }
        let progress = entities
            .get_mut(worker)
            .and_then(|w| w.tick_deconstruction())
            .unwrap_or(0);
        if progress < required_ticks {
            continue;
        }

        let cost = rules::economy::resource_cost(EntityKind::TankTrap);
        let dismantled = entities
            .get_mut(target)
            .map(|trap| {
                let hp = trap.hp;
                trap.apply_damage(hp, None)
            })
            .unwrap_or(false);
        if dismantled {
            if let Some(owner) = entities.get(worker).map(|w| w.owner) {
                if let Some(player) = players.iter_mut().find(|p| p.id == owner) {
                    player.refund_cost(cost);
                }
            }
        }
        if let Some(w) = entities.get_mut(worker) {
            w.clear_active_order();
        }
    }
}

fn live_completed_tank_trap_position(entities: &EntityStore, target: u32) -> Option<(f32, f32)> {
    let trap = entities.get(target)?;
    (trap.kind == EntityKind::TankTrap && trap.hp > 0 && !trap.under_construction())
        .then_some((trap.pos_x, trap.pos_y))
}

pub(crate) fn resumable_site_for_build_intent(
    map: &Map,
    entities: &EntityStore,
    owner: u32,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> Option<u32> {
    let (cx, cy) = footprint_center(map, kind, tile_x, tile_y);
    entities.iter().find_map(|entity| {
        (entity.owner == owner
            && entity.kind == kind
            && entity.under_construction()
            && (entity.pos_x - cx).abs() <= 0.01
            && (entity.pos_y - cy).abs() <= 0.01)
            .then_some(entity.id)
    })
}

/// Ensure a worker is in a body-standable position whenever it attaches to a scaffold and before
/// it drops its ghost phase. A worker may arrive inside its intended footprint because placement
/// deliberately ignores the chosen builder's body. Once the scaffold exists, leaving the worker
/// there would shelter it inside the building for the entire construction period. Tight placement
/// against neighbouring buildings can also leave the worker clipping an adjacent footprint.
///
/// We snap it to the closest tile center where its body is fully static-standable (terrain
/// passable, outside every building footprint). The scan covers the whole map: construction is
/// infrequent, and an arbitrary local radius can strand a released worker in a dense base.
fn eject_worker_from_static_overlap(map: &Map, entities: &mut EntityStore, worker: u32) {
    let (wx, wy, wkind) = match entities.get(worker) {
        Some(w) => (w.pos_x, w.pos_y, w.kind),
        None => return,
    };

    let occ = Occupancy::build(map, entities);
    if standability::unit_static_standable(map, &occ, wkind, wx, wy) {
        return;
    }

    let mut destination: Option<((f32, f32), f32)> = None;
    for ty in 0..map.size {
        for tx in 0..map.size {
            let (cx, cy) = map.tile_center(tx, ty);
            if !standability::unit_static_standable(map, &occ, wkind, cx, cy) {
                continue;
            }
            let distance2 = dist2(wx, wy, cx, cy);
            if destination.is_none_or(|(_, best_distance2)| distance2 < best_distance2) {
                destination = Some(((cx, cy), distance2));
            }
        }
    }

    if let (Some(w), Some(((x, y), _))) = (entities.get_mut(worker), destination) {
        w.set_position(x, y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityStore, Order, OrderIntent};
    use crate::game::services::occupancy::footprint_center;
    use crate::game::ScoreState;
    use crate::protocol::terrain;

    #[test]
    fn construction_ejects_builder_from_new_scaffold_and_keeps_building() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let (wx, wy) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, wx, wy)
            .expect("worker should spawn");
        entities
            .get_mut(worker)
            .expect("worker should exist")
            .set_order(Order::build(EntityKind::Depot, 4, 4));
        let mut players = vec![player_state(1)];
        let mut events = HashMap::new();

        let fog = Fog::new(map.size);
        let mut active_sites = BTreeSet::new();
        construction_system(
            &map,
            &mut entities,
            &mut players,
            &mut events,
            &fog,
            &mut active_sites,
        );

        assert!(
            entities
                .iter()
                .any(|entity| entity.kind == EntityKind::Depot && entity.under_construction()),
            "chosen builder body inside the footprint should not prevent scaffold creation"
        );
        assert!(
            matches!(
                entities
                    .get(worker)
                    .expect("worker should survive")
                    .build_phase(),
                Some(BuildPhase::Constructing { .. })
            ),
            "chosen builder should transition into active construction"
        );
        let built_site = entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Depot && entity.under_construction())
            .expect("depot scaffold should exist");
        let worker_entity = entities.get(worker).expect("worker should survive");
        let occupancy = Occupancy::build(&map, &entities);
        assert!(
            standability::unit_static_standable(
                &map,
                &occupancy,
                worker_entity.kind,
                worker_entity.pos_x,
                worker_entity.pos_y,
            ),
            "builder should be ejected to an attackable position outside the scaffold"
        );
        assert!(
            active_sites.contains(&built_site.id),
            "ejected builder should progress construction in the same tick"
        );
        assert!(
            events.get(&1).is_none_or(Vec::is_empty),
            "accepted build-over-self placement should not notify the owner"
        );
    }

    #[test]
    fn construction_waits_on_other_unit_body_intersecting_footprint() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let rect =
            crate::game::services::geometry::building_rect_for_footprint(EntityKind::Depot, 4, 4)
                .expect("depot rect");
        let radius = config::unit_stats(EntityKind::Tank)
            .expect("tank stats")
            .radius;
        let worker = entities
            .spawn_unit(
                1,
                EntityKind::Worker,
                rect.max_x + config::TILE_SIZE as f32,
                rect.min_y + config::TILE_SIZE as f32,
            )
            .expect("worker should spawn");
        entities
            .get_mut(worker)
            .expect("worker should exist")
            .set_order(Order::build(EntityKind::Depot, 4, 4));
        entities
            .spawn_unit(
                1,
                EntityKind::Tank,
                rect.max_x + radius - 1.0,
                rect.min_y + 32.0,
            )
            .expect("tank should spawn");
        let mut players = vec![player_state(1)];
        let mut events = HashMap::new();

        let fog = Fog::new(map.size);
        let mut active_sites = BTreeSet::new();
        construction_system(
            &map,
            &mut entities,
            &mut players,
            &mut events,
            &fog,
            &mut active_sites,
        );

        assert!(
            entities
                .iter()
                .all(|entity| entity.kind != EntityKind::Depot),
            "another living unit body intersecting the footprint must delay scaffold creation"
        );
        assert!(
            matches!(
                entities
                    .get(worker)
                    .expect("worker should survive")
                    .build_phase(),
                Some(BuildPhase::WaitingAtSite)
            ),
            "blocked final placement should hold the build order during the grace window"
        );
        assert!(
            events.get(&1).is_none_or(Vec::is_empty),
            "unit-blocked wait should not emit a failure notice before timeout"
        );
    }

    #[test]
    fn eject_helper_pushes_worker_off_neighbor_building_body_overlap() {
        // Regression: a worker that finishes constructing a building tightly packed against
        // neighbours could have its circular body poking into an adjacent building's footprint,
        // tripping the static-body invariant the moment it left the ghost phase.
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let ts = config::TILE_SIZE as f32;
        // Two adjacent factory footprints.
        let (ax, ay) = footprint_center(&map, EntityKind::Factory, 10, 10);
        let (bx, by) = footprint_center(&map, EntityKind::Factory, 13, 10);
        entities
            .spawn_building(1, EntityKind::Factory, ax, ay, false)
            .expect("factory A should spawn");
        entities
            .spawn_building(1, EntityKind::Factory, bx, by, false)
            .expect("factory B should spawn");
        // Place the worker on the seam between them so its circle body bleeds into a footprint.
        let rect_a = crate::game::services::geometry::building_rect_for_footprint(
            EntityKind::Factory,
            10,
            10,
        )
        .expect("factory A rect");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, rect_a.max_x - 0.5, ay)
            .expect("worker should spawn");

        let occ_before = Occupancy::build(&map, &entities);
        assert!(
            !standability::unit_static_standable(
                &map,
                &occ_before,
                EntityKind::Worker,
                entities.get(worker).unwrap().pos_x,
                entities.get(worker).unwrap().pos_y,
            ),
            "test setup must reproduce a body-overlap before the eject helper runs"
        );

        eject_worker_from_static_overlap(&map, &mut entities, worker);

        let occ_after = Occupancy::build(&map, &entities);
        let w = entities.get(worker).expect("worker should survive");
        assert!(
            standability::unit_static_standable(&map, &occ_after, w.kind, w.pos_x, w.pos_y),
            "eject helper must leave the worker in a body-standable position (pos=({:.1},{:.1}))",
            w.pos_x,
            w.pos_y,
        );
        // Sanity: it should have moved by at least a fraction of a tile.
        let dx = (w.pos_x - (rect_a.max_x - 0.5)).abs();
        let dy = (w.pos_y - ay).abs();
        assert!(
            dx + dy > ts * 0.25,
            "worker should have moved to escape the overlap"
        );
    }

    #[test]
    fn arrival_to_existing_scaffold_resumes_without_spawning_or_repaying() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let (sx, sy) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let site = entities
            .spawn_building(1, EntityKind::Depot, sx, sy, false)
            .expect("scaffold should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, sx, sy)
            .expect("worker should spawn");
        entities
            .get_mut(worker)
            .expect("worker should exist")
            .set_order(Order::build(EntityKind::Depot, 4, 4));

        let mut players = vec![player_state(1)];
        let starting_steel = players[0].steel;
        let starting_oil = players[0].oil;
        let mut events = HashMap::new();

        let fog = Fog::new(map.size);
        let mut active_sites = BTreeSet::new();
        construction_system(
            &map,
            &mut entities,
            &mut players,
            &mut events,
            &fog,
            &mut active_sites,
        );

        let owned_depots: Vec<_> = entities
            .iter()
            .filter(|entity| entity.owner == 1 && entity.kind == EntityKind::Depot)
            .map(|entity| entity.id)
            .collect();
        assert_eq!(
            owned_depots,
            vec![site],
            "resuming should reuse the existing scaffold instead of spawning another building"
        );
        assert_eq!(
            players[0].steel, starting_steel,
            "resuming an existing scaffold must not charge steel again"
        );
        assert_eq!(
            players[0].oil, starting_oil,
            "resuming an existing scaffold must not charge oil again"
        );
        assert_eq!(
            entities
                .get(worker)
                .expect("worker should survive")
                .build_phase(),
            Some(BuildPhase::Constructing { site }),
            "worker should latch onto the existing scaffold"
        );
        assert!(
            events.get(&1).is_none_or(Vec::is_empty),
            "resume should not emit a failure notice"
        );
    }

    #[test]
    fn build_completion_preserves_queued_orders_for_handoff() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let (sx, sy) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let site = entities
            .spawn_building(1, EntityKind::Depot, sx, sy, false)
            .expect("scaffold should spawn");
        // Drive the scaffold to one tick away from completion.
        if let Some(b) = entities.get_mut(site) {
            if let Some(progress) = b.construction.as_ref().map(|c| c.total.saturating_sub(1)) {
                b.set_construction_progress(progress);
            }
        }
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, sx, sy)
            .expect("worker should spawn");
        let handoff = (sx + 96.0, sy);
        {
            let w = entities.get_mut(worker).expect("worker should exist");
            w.set_order(Order::build(EntityKind::Depot, 4, 4));
            w.mark_build_phase(BuildPhase::Constructing { site });
            w.set_target_id(Some(site));
            w.append_queued_order(OrderIntent::move_to(handoff.0, handoff.1));
        }
        let mut players = vec![player_state(1)];
        let mut events = HashMap::new();

        let fog = Fog::new(map.size);
        let mut active_sites = BTreeSet::new();
        construction_system(
            &map,
            &mut entities,
            &mut players,
            &mut events,
            &fog,
            &mut active_sites,
        );
        assert!(
            active_sites.contains(&site),
            "completed scaffold should still be reported as actively advanced this tick"
        );

        let w = entities.get(worker).expect("worker should survive");
        assert!(
            matches!(w.order(), Order::Idle),
            "completed build should drop the active order to idle"
        );
        assert_eq!(
            w.queued_orders().len(),
            1,
            "build completion must leave queued handoff orders intact for promotion"
        );
    }

    #[test]
    fn build_completion_clears_all_workers_constructing_same_site() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let (sx, sy) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let site = entities
            .spawn_building(1, EntityKind::Depot, sx, sy, false)
            .expect("scaffold should spawn");
        if let Some(b) = entities.get_mut(site) {
            if let Some(progress) = b.construction.as_ref().map(|c| c.total.saturating_sub(1)) {
                b.set_construction_progress(progress);
            }
        }
        let finishing_worker = entities
            .spawn_unit(1, EntityKind::Worker, sx, sy)
            .expect("worker should spawn");
        let helper_worker = entities
            .spawn_unit(1, EntityKind::Worker, sx + 8.0, sy)
            .expect("helper worker should spawn");
        for worker in [finishing_worker, helper_worker] {
            let w = entities.get_mut(worker).expect("worker should exist");
            w.set_order(Order::build(EntityKind::Depot, 4, 4));
            w.mark_build_phase(BuildPhase::Constructing { site });
            w.set_target_id(Some(site));
        }
        let mut players = vec![player_state(1)];
        let mut events = HashMap::new();

        let fog = Fog::new(map.size);
        let mut active_sites = BTreeSet::new();
        construction_system(
            &map,
            &mut entities,
            &mut players,
            &mut events,
            &fog,
            &mut active_sites,
        );

        assert!(
            entities
                .get(site)
                .is_some_and(|entity| !entity.under_construction()),
            "scaffold should complete"
        );
        for worker in [finishing_worker, helper_worker] {
            assert!(
                matches!(
                    entities.get(worker).expect("worker should survive").order(),
                    Order::Idle
                ),
                "all workers that targeted the completed site should clear Build orders"
            );
        }
        let occ = Occupancy::build(&map, &entities);
        for worker in [finishing_worker, helper_worker] {
            let worker = entities.get(worker).expect("worker should survive");
            assert!(
                standability::unit_static_standable(
                    &map,
                    &occ,
                    worker.kind,
                    worker.pos_x,
                    worker.pos_y,
                ),
                "released builders must be ejected from completed building footprint overlap"
            );
        }
    }

    #[test]
    fn build_site_destruction_clears_active_order_but_preserves_queue() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let (sx, sy) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let site = entities
            .spawn_building(1, EntityKind::Depot, sx, sy, false)
            .expect("scaffold should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, sx, sy)
            .expect("worker should spawn");
        let handoff = (sx + 96.0, sy);
        {
            let w = entities.get_mut(worker).expect("worker should exist");
            w.set_order(Order::build(EntityKind::Depot, 4, 4));
            w.mark_build_phase(BuildPhase::Constructing { site });
            w.set_target_id(Some(site));
            w.append_queued_order(OrderIntent::move_to(handoff.0, handoff.1));
        }
        // Simulate the scaffold being destroyed mid-construction.
        entities.remove(site);
        let mut players = vec![player_state(1)];
        let mut events = HashMap::new();

        let fog = Fog::new(map.size);
        let mut active_sites = BTreeSet::new();
        construction_system(
            &map,
            &mut entities,
            &mut players,
            &mut events,
            &fog,
            &mut active_sites,
        );

        let w = entities.get(worker).expect("worker should survive");
        assert!(
            matches!(w.order(), Order::Idle),
            "lost scaffold should drop the worker's active order"
        );
        assert_eq!(
            w.queued_orders().len(),
            1,
            "queued handoff orders must persist after a scaffold is destroyed"
        );
    }

    #[test]
    fn deconstruction_refunds_worker_owner_and_marks_trap_dead_at_half_build_time() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let (tx, ty) = footprint_center(&map, EntityKind::TankTrap, 4, 4);
        let trap = entities
            .spawn_building(2, EntityKind::TankTrap, tx, ty, true)
            .expect("tank trap should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, tx + config::TILE_SIZE as f32, ty)
            .expect("worker should spawn");
        {
            let w = entities.get_mut(worker).expect("worker should exist");
            w.set_order(Order::deconstruct(trap));
            w.mark_deconstruct_phase(DeconstructPhase::Deconstructing);
            w.set_target_id(Some(trap));
        }
        let mut players = vec![player_state(1), player_state(2)];
        let steel_before = players[0].steel;
        let enemy_steel_before = players[1].steel;
        let build_ticks = config::building_stats(EntityKind::TankTrap)
            .unwrap()
            .build_ticks;
        let required_ticks = tank_trap_deconstruction_ticks();
        assert_eq!(required_ticks * 2, build_ticks);

        for _ in 1..required_ticks {
            deconstruction_system(&mut entities, &mut players);
        }
        assert!(entities.get(trap).expect("trap should exist").hp > 0);

        deconstruction_system(&mut entities, &mut players);

        assert_eq!(
            players[0].steel,
            steel_before + rules::economy::resource_cost(EntityKind::TankTrap).steel,
            "deconstructing player should receive the Tank Trap steel refund"
        );
        assert_eq!(
            players[1].steel, enemy_steel_before,
            "original owner should not receive the refund"
        );
        assert_eq!(
            entities
                .get(trap)
                .expect("trap should exist until death runs")
                .hp,
            0,
            "completed deconstruction should mark the Tank Trap for normal death cleanup"
        );
        assert!(matches!(
            entities.get(worker).expect("worker should survive").order(),
            Order::Idle
        ));
    }

    #[test]
    fn deconstruction_completion_preserves_queued_orders_for_handoff() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let (tx, ty) = footprint_center(&map, EntityKind::TankTrap, 4, 4);
        let trap = entities
            .spawn_building(2, EntityKind::TankTrap, tx, ty, true)
            .expect("tank trap should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, tx + config::TILE_SIZE as f32, ty)
            .expect("worker should spawn");
        let handoff = (tx + 96.0, ty);
        {
            let w = entities.get_mut(worker).expect("worker should exist");
            w.set_order(Order::deconstruct(trap));
            w.mark_deconstruct_phase(DeconstructPhase::Deconstructing);
            w.set_target_id(Some(trap));
            w.append_queued_order(OrderIntent::move_to(handoff.0, handoff.1));
        }
        let mut players = vec![player_state(1), player_state(2)];
        let required_ticks = tank_trap_deconstruction_ticks();

        for _ in 0..required_ticks {
            deconstruction_system(&mut entities, &mut players);
        }

        let w = entities.get(worker).expect("worker should survive");
        assert!(matches!(w.order(), Order::Idle));
        assert_eq!(
            w.queued_orders().len(),
            1,
            "deconstruction completion must leave queued handoff orders intact for promotion"
        );
    }

    pub(super) fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![],
            base_sites: vec![],
        }
    }

    pub(super) fn player_state(id: u32) -> PlayerState {
        PlayerState {
            id,
            team_id: id,
            faction_id: "kriegsia".to_string(),
            name: format!("Player {id}"),
            color: "#fff".to_string(),
            start_tile: (0, 0),
            steel: 1_000,
            oil: 1_000,
            supply_used: 0,
            is_ai: false,
            score: ScoreState::default(),
            upgrades: Default::default(),
            ability_cooldowns: Default::default(),
        }
    }
}

#[cfg(test)]
use tests::{flat_map as test_flat_map, player_state as test_player_state};

#[cfg(test)]
#[path = "construction/build_wait_tests.rs"]
mod build_wait_tests;
