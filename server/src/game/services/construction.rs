use std::collections::HashMap;

use crate::config;
use crate::game::entity::{BuildPhase, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::pathfinding::Passability;
use crate::game::services::occupancy::{
    building_footprint, footprint_center, footprint_placeable, Occupancy,
};
use crate::game::services::spatial::SpatialIndex;
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
    spatial: &SpatialIndex,
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
        let placeable = footprint_placeable(map, entities, spatial, kind, tx, ty);
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
            eject_worker_if_inside(map, entities, worker, site);
            if let Some(w) = entities.get_mut(worker) {
                w.clear_orders();
            }
        }
    }
}

/// If `worker` is standing on a tile inside `site`'s footprint, teleport it to the nearest
/// passable tile outside the footprint. This prevents workers from getting permanently stuck
/// when a building is placed on top of them.
fn eject_worker_if_inside(map: &Map, entities: &mut EntityStore, worker: u32, site: u32) {
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
