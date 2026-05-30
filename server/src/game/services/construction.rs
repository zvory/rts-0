use std::collections::HashMap;

use crate::config;
use crate::game::entity::{BuildPhase, EntityStore};
use crate::game::map::{Map, MobilityClass};
use crate::game::pathfinding::Passability;
use crate::game::services::occupancy::{building_footprint, Occupancy};
use crate::game::services::{dist2, interact_range};
use crate::protocol::Event;

/// Advance construction for buildings that have a worker actively building them. A worker on a
/// `Build` order that has arrived at the site contributes one tick of progress per tick. On
/// completion the building leaves CONSTRUCT, the worker is freed (idle), and a `Build` event
/// fires to the owner.
pub(crate) fn construction_system(
    map: &Map,
    entities: &mut EntityStore,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    // Collect (worker_id, site_id) build assignments where the worker has reached the site.
    let mut working: Vec<(u32, u32)> = Vec::new();
    for e in entities.iter() {
        if e.is_unit() {
            if let Some(site) = e.order().build_site() {
                if let Some(b) = entities.get(site) {
                    let arrive =
                        interact_range(entities, site).unwrap_or(config::TILE_SIZE as f32 * 2.0);
                    if b.under_construction()
                        && dist2(e.pos_x, e.pos_y, b.pos_x, b.pos_y).sqrt() <= arrive
                    {
                        working.push((e.id, site));
                    }
                }
            }
        }
    }

    for (worker, site) in working {
        // Stop the worker moving while it builds.
        if let Some(w) = entities.get_mut(worker) {
            w.clear_path();
            w.mark_build_phase(BuildPhase::Constructing);
        }
        let completed = {
            let b = match entities.get_mut(site) {
                Some(b) if b.under_construction() => b,
                _ => continue,
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
                .unwrap_or((0, crate::game::entity::EntityKind::Worker));
            events.entry(owner).or_default().push(Event::Build {
                id: site,
                kind: kind.to_protocol_str().to_string(),
            });
            // Move the worker out of the footprint so it doesn't get trapped.
            eject_worker_if_inside(map, entities, worker, site);
            // Free the worker.
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
    for r in 1..=4 {
        for dy in -r..=r {
            for dx in -r..=r {
                if (dx as i32).abs().max((dy as i32).abs()) != r {
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
                if !map.is_passable_for(MobilityClass::Infantry, tx as i32, ty as i32) {
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
