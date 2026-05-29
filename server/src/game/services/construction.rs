use std::collections::HashMap;

use crate::config;
use crate::game::entity::{EntityStore, Order};
use crate::game::services::{dist2, interact_range};
use crate::protocol::Event;

/// Advance construction for buildings that have a worker actively building them. A worker on a
/// `Build` order that has arrived at the site contributes one tick of progress per tick. On
/// completion the building leaves CONSTRUCT, the worker is freed (idle), and a `Build` event
/// fires to the owner.
pub(crate) fn construction_system(
    entities: &mut EntityStore,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    // Collect (worker_id, site_id) build assignments where the worker has reached the site.
    let mut working: Vec<(u32, u32)> = Vec::new();
    for e in entities.iter() {
        if e.is_unit() {
            if let Order::Build { site } = e.order {
                if let Some(b) = entities.get(site) {
                    let arrive =
                        interact_range(entities, site).unwrap_or(config::TILE_SIZE as f32 * 2.0);
                    if b.under_construction
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
            w.path.clear();
        }
        let completed = {
            let b = match entities.get_mut(site) {
                Some(b) if b.under_construction => b,
                _ => continue,
            };
            b.build_progress += 1;
            if b.build_progress >= b.build_total {
                b.under_construction = false;
                b.build_progress = b.build_total;
                true
            } else {
                false
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
            // Free the worker.
            if let Some(w) = entities.get_mut(worker) {
                w.clear_orders();
            }
        }
    }
}
