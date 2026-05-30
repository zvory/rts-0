use std::collections::HashMap;

use crate::game::entity::{EntityKind, EntityStore, Order};
use crate::game::fog::Fog;
use crate::protocol::Event;

/// Remove entities whose hp has hit zero, emitting a fog-respecting `Death` event: a player
/// gets the poof only if they owned the entity or its death position is currently visible to
/// them (events are best-effort flavor). `death_system` runs before the fog recompute, so the
/// current fog still reflects who could see the unit while it was alive — exactly the players
/// who should see it die. A dead building drops its queue implicitly by being removed. Workers
/// building a since-removed site are reset elsewhere.
pub(crate) fn death_system(
    entities: &mut EntityStore,
    fog: &Fog,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    let dead: Vec<(u32, u32, f32, f32, EntityKind)> = entities
        .iter()
        .filter(|e| e.is_targetable() && e.hp == 0)
        .map(|e| (e.id, e.owner, e.pos_x, e.pos_y, e.kind))
        .collect();

    for (id, owner, x, y, kind) in dead {
        entities.remove(id);
        // Deliver the death only to players who owned the entity or could see where it died,
        // so a death poof never reveals an entity hidden in a player's fog.
        let pids: Vec<u32> = events.keys().copied().collect();
        for pid in pids {
            if pid != owner && !fog.is_visible_world(pid, x, y) {
                continue;
            }
            events.entry(pid).or_default().push(Event::Death {
                id,
                x,
                y,
                kind: kind.to_protocol_str().to_string(),
            });
        }
    }

    // Clear any node miner reservations pointing at dead workers.
    let nodes_to_clear: Vec<u32> = entities
        .iter()
        .filter(|e| e.is_node())
        .filter_map(|e| {
            e.miner().and_then(|m| {
                if !entities.contains(m) {
                    Some(e.id)
                } else {
                    None
                }
            })
        })
        .collect();
    for nid in nodes_to_clear {
        if let Some(n) = entities.get_mut(nid) {
            if let Some(node) = n.resource_node.as_mut() {
                node.miner = None;
            }
        }
    }

    // Clean up dangling orders that reference removed entities (build sites, attack targets)
    // so units don't chase ghosts. Gather orders self-heal via `retarget_or_idle`.
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
                _ => false,
            }
        };
        if stale {
            if let Some(e) = entities.get_mut(id) {
                e.clear_orders();
            }
        }
    }
}
