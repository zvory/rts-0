use std::collections::{BTreeSet, HashMap};

use crate::config;
use crate::game::entity::{BuildPhase, DeconstructPhase, EntityKind, EntityStore};
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

/// Advance build orders. Workers in `ToSite` that have walked into arrival range of their
/// intended footprint re-validate placement and affordability, spawn the building, deduct
/// cost, and transition to `Constructing`. Workers in `Constructing` accumulate one tick
/// of progress per tick; on completion the building leaves CONSTRUCT, the worker is
/// freed, and a `Build` event fires to the owner.
pub(crate) fn construction_system(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    active_construction_sites: &mut BTreeSet<u32>,
) {
    let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
    // ----- Arrival pass: ToSite workers that have reached their target -----
    let arrivals: Vec<(u32, EntityKind, u32, u32)> = entities
        .iter()
        .filter_map(|e| {
            if e.hp == 0 || !e.is_unit() {
                return None;
            }
            if e.build_phase() != Some(BuildPhase::ToSite) {
                return None;
            }
            let (kind, tx, ty) = e.order().build_intent_tile()?;
            let (cx, cy) = footprint_center(map, kind, tx, ty);
            let arrive = interact_range_for_kind(kind);
            if dist2(e.pos_x, e.pos_y, cx, cy).sqrt() <= arrive {
                Some((e.id, kind, tx, ty))
            } else {
                None
            }
        })
        .collect();

    for (worker, kind, tx, ty) in arrivals {
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
            continue;
        }

        // Re-validate placement against the live entity set.
        let placeable =
            standability::building_site_clear_for_build_intent(map, entities, kind, tx, ty, worker);
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
        let cost = rules::economy::resource_cost(kind);
        let resource_notice = match players.iter().find(|p| p.id == owner) {
            Some(p) => {
                if p.can_afford(cost.steel, cost.oil) {
                    None
                } else {
                    Some(rules::economy::resource_shortage_notice_for_cost(
                        p.steel, p.oil, cost,
                    ))
                }
            }
            None => Some("Not enough resources"),
        };

        if !placeable {
            events.entry(owner).or_default().push(Event::Notice {
                msg: "Cannot build there".to_string(),
                x: None,
                y: None,
                severity: NoticeSeverity::Info,
            });
            if let Some(w) = entities.get_mut(worker) {
                w.clear_active_order();
            }
            continue;
        }
        if let Some(msg) = resource_notice {
            events.entry(owner).or_default().push(Event::Notice {
                msg: msg.to_string(),
                x: None,
                y: None,
                severity: NoticeSeverity::Info,
            });
            if let Some(w) = entities.get_mut(worker) {
                w.clear_active_order();
            }
            continue;
        }

        let ps = match players.iter_mut().find(|p| p.id == owner) {
            Some(p) => p,
            None => continue,
        };
        if !ps.spend_cost(cost) {
            continue;
        }

        let (cx, cy) = footprint_center(map, kind, tx, ty);
        let site = match entities.spawn_building(owner, kind, cx, cy, false) {
            Some(id) => id,
            None => continue,
        };
        if let Some(player) = players.iter_mut().find(|p| p.id == owner) {
            player.record_entity_created(kind);
        }
        if let Some(w) = entities.get_mut(worker) {
            w.clear_path();
            w.set_target_id(Some(site));
            w.mark_build_phase(BuildPhase::Constructing { site });
        }
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
            defensively_eject_worker_from_static_overlap(map, entities, worker);
            if let Some(w) = entities.get_mut(worker) {
                w.clear_active_order();
            }
        }
    }
}

/// Advance Tank Trap deconstruction orders. A worker must first reach the target trap, then spends
/// the trap's normal build time dismantling it. Completion refunds the trap cost to the worker's
/// owner and leaves removal/event fanout to the ordinary death system.
pub(crate) fn deconstruction_system(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
) {
    let arrivals: Vec<(u32, Option<u32>)> = entities
        .iter()
        .filter_map(|e| {
            if e.hp == 0 || !e.is_unit() || e.deconstruct_phase() != Some(DeconstructPhase::ToTarget)
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
    let required_ticks = config::building_stats(EntityKind::TankTrap)
        .map(|stats| stats.build_ticks)
        .unwrap_or(config::TICK_HZ * 10);

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

/// Ensure a worker that just finished construction is in a body-standable position before
/// dropping its ghost phase. While the worker was ghost-anchored, its circular body was free
/// to overlap building footprints (this is how a builder can stand inside its own scaffold).
/// Once the worker leaves the ghost phase its body is checked by the static invariants, so it
/// must not still be clipping any building rect. This is most easily violated when neighbours
/// are packed tightly around the builder's arrival position; the builder's arrival check only
/// enforces footprint clearance for the new building, not body clearance for adjacent ones.
///
/// We spiral outward from the worker's current tile and snap it to the first tile center where
/// its body is fully static-standable (terrain passable, outside every building footprint).
fn defensively_eject_worker_from_static_overlap(
    map: &Map,
    entities: &mut EntityStore,
    worker: u32,
) {
    let (wx, wy, wkind) = match entities.get(worker) {
        Some(w) => (w.pos_x, w.pos_y, w.kind),
        None => return,
    };

    let occ = Occupancy::build(map, entities);
    if standability::unit_static_standable(map, &occ, wkind, wx, wy) {
        return;
    }

    let (wtx, wty) = map.tile_of(wx, wy);
    let ts = config::TILE_SIZE as f32;
    for r in 1i32..=8 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dy.abs()) != r {
                    continue;
                }
                let tx = wtx as i32 + dx;
                let ty = wty as i32 + dy;
                if !map.in_bounds(tx, ty) {
                    continue;
                }
                let cx = tx as f32 * ts + ts * 0.5;
                let cy = ty as f32 * ts + ts * 0.5;
                if standability::unit_static_standable(map, &occ, wkind, cx, cy) {
                    if let Some(w) = entities.get_mut(worker) {
                        w.pos_x = cx;
                        w.pos_y = cy;
                    }
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityStore, Order, OrderIntent};
    use crate::game::services::geometry::building_rect_for_footprint;
    use crate::game::services::occupancy::footprint_center;
    use crate::game::ScoreState;
    use crate::protocol::terrain;

    #[test]
    fn construction_allows_builder_body_inside_footprint() {
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
        assert!(
            events.get(&1).is_none_or(Vec::is_empty),
            "accepted build-over-self placement should not notify the owner"
        );
    }

    #[test]
    fn construction_rejects_other_unit_body_intersecting_footprint() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
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
            "another living unit body intersecting the footprint must prevent scaffold creation"
        );
        assert!(
            matches!(
                entities.get(worker).expect("worker should survive").order(),
                Order::Idle
            ),
            "blocked final placement should clear the worker order"
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
        let rect_a =
            building_rect_for_footprint(EntityKind::Factory, 10, 10).expect("factory A rect");
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

        defensively_eject_worker_from_static_overlap(&map, &mut entities, worker);

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
    fn deconstruction_refunds_worker_owner_and_marks_trap_dead_after_ten_seconds() {
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
        let required_ticks = config::building_stats(EntityKind::TankTrap)
            .expect("tank trap stats")
            .build_ticks;

        for _ in 0..required_ticks {
            deconstruction_system(&mut entities, &mut players);
        }

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
            entities.get(trap).expect("trap should exist until death runs").hp,
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
        let required_ticks = config::building_stats(EntityKind::TankTrap)
            .expect("tank trap stats")
            .build_ticks;

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
            team_id: id,
            faction_id: "kriegsia".to_string(),
            name: format!("Player {id}"),
            color: "#fff".to_string(),
            start_tile: (0, 0),
            steel: 1_000,
            oil: 1_000,
            supply_used: 0,
            supply_cap: 20,
            is_ai: false,
            score: ScoreState::default(),
            upgrades: Default::default(),
        }
    }
}
