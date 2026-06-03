use std::collections::HashMap;

use crate::config;
use crate::game::entity::{BuildPhase, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::pathfinding::Passability;
use crate::game::services::occupancy::{building_footprint, footprint_center, Occupancy};
use crate::game::services::standability;
use crate::game::services::{dist2, interact_range_for_kind};
use crate::game::PlayerState;
use crate::protocol::Event;
use crate::rules;

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
) {
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

        // Re-validate placement against the live entity set.
        let placeable = standability::building_site_clear(map, entities, kind, tx, ty);
        if config::building_stats(kind).is_none() {
            if let Some(w) = entities.get_mut(worker) {
                w.clear_orders();
            }
            continue;
        }
        let (cost_steel, cost_oil) = rules::economy::cost(kind);
        let affordable = players
            .iter()
            .find(|p| p.id == owner)
            .map(|p| p.steel >= cost_steel && p.oil >= cost_oil)
            .unwrap_or(false);

        if !placeable {
            events.entry(owner).or_default().push(Event::Notice {
                msg: "Cannot build there".to_string(),
            });
            if let Some(w) = entities.get_mut(worker) {
                w.clear_orders();
            }
            continue;
        }
        if !affordable {
            events.entry(owner).or_default().push(Event::Notice {
                msg: "Not enough resources".to_string(),
            });
            if let Some(w) = entities.get_mut(worker) {
                w.clear_orders();
            }
            continue;
        }

        let ps = match players.iter_mut().find(|p| p.id == owner) {
            Some(p) => p,
            None => continue,
        };
        ps.steel -= cost_steel;
        ps.oil -= cost_oil;

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
                BuildPhase::ToSite => None,
            }
        })
        .collect();

    for (worker, site) in working {
        let completed = {
            let b = match entities.get_mut(site) {
                Some(b) if b.hp > 0 && b.under_construction() => b,
                _ => {
                    if let Some(w) = entities.get_mut(worker) {
                        w.clear_orders();
                    }
                    continue;
                }
            };
            let Some(c) = b.construction.as_mut() else {
                continue;
            };
            c.progress += 1;
            if c.progress < c.total {
                false
            } else {
                c.progress = c.total;
                b.construction = None;
                true
            }
        };
        if completed {
            let (owner, kind) = entities
                .get(site)
                .map(|b| (b.owner, b.kind))
                .unwrap_or((0, EntityKind::Worker));
            events.entry(owner).or_default().push(Event::Build {
                id: site,
                kind: kind.to_protocol_str().to_string(),
            });
            defensively_eject_worker_from_legacy_overlap(map, entities, worker, site);
            if let Some(w) = entities.get_mut(worker) {
                w.clear_orders();
            }
        }
    }
}

/// Defensive fallback for malformed legacy states only.
///
/// Normal construction should never need this: scaffold creation is body-aware and rejects every
/// living unit body, including the chosen builder after the build-intent staging exception has
/// done its job. If an old replay or manual test fixture already has a constructing worker inside
/// its site, move it before clearing the ghost construction phase so invariants report the source
/// bug instead of leaving the worker permanently embedded.
fn defensively_eject_worker_from_legacy_overlap(
    map: &Map,
    entities: &mut EntityStore,
    worker: u32,
    site: u32,
) {
    let b = match entities.get(site) {
        Some(b) => b,
        None => return,
    };
    let footprint = building_footprint(map, b);
    let w = match entities.get(worker) {
        Some(w) => w,
        None => return,
    };
    let (wtx, wty) = map.tile_of(w.pos_x, w.pos_y);
    if !footprint.contains(&(wtx, wty)) {
        return;
    }

    // Recompute occupancy so we respect other finished buildings.
    let occ = Occupancy::build(map, entities);
    let ts = config::TILE_SIZE as f32;
    for r in 1i32..=4 {
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
                let tx = tx as u32;
                let ty = ty as u32;
                if footprint.contains(&(tx, ty)) {
                    continue;
                }
                if !map.is_passable(tx as i32, ty as i32) {
                    continue;
                }
                if !occ.passable(tx as i32, ty as i32) {
                    continue;
                }
                if let Some(w) = entities.get_mut(worker) {
                    w.pos_x = tx as f32 * ts + ts * 0.5;
                    w.pos_y = ty as f32 * ts + ts * 0.5;
                }
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityStore, Order};
    use crate::game::services::geometry::building_rect_for_footprint;
    use crate::game::services::occupancy::footprint_center;
    use crate::game::ScoreState;
    use crate::protocol::terrain;

    #[test]
    fn construction_revalidates_worker_body_outside_footprint() {
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

        construction_system(&map, &mut entities, &mut players, &mut events);

        assert!(
            entities
                .iter()
                .all(|entity| entity.kind != EntityKind::Depot),
            "worker body inside the footprint must prevent scaffold creation"
        );
        assert!(
            matches!(
                entities.get(worker).expect("worker should survive").order(),
                Order::Idle
            ),
            "failed final placement should clear the worker order"
        );
        assert!(
            events
                .get(&1)
                .is_some_and(|events| matches!(events.as_slice(), [Event::Notice { msg }] if msg == "Cannot build there")),
            "failed final placement should notify the owner"
        );
    }

    #[test]
    fn construction_rejects_other_unit_body_intersecting_footprint() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let rect = building_rect_for_footprint(EntityKind::Depot, 4, 4).expect("depot rect");
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
            .spawn_unit(1, EntityKind::Tank, rect.max_x + 19.0, rect.min_y + 32.0)
            .expect("tank should spawn");
        let mut players = vec![player_state(1)];
        let mut events = HashMap::new();

        construction_system(&map, &mut entities, &mut players, &mut events);

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
