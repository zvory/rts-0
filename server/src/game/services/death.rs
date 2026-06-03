use std::collections::HashMap;

use crate::game::entity::{EntityKind, EntityStore, Order};
use crate::game::fog::Fog;
use crate::game::PlayerState;
use crate::protocol::Event;
use crate::rules::projection;

/// Remove entities whose hp has hit zero, emitting a fog-respecting `Death` event: a player
/// gets the poof only if they owned the entity or its death position is currently visible to
/// them (events are best-effort flavor). `death_system` runs before the fog recompute, so the
/// current fog still reflects who could see the unit while it was alive — exactly the players
/// who should see it die. A dead building drops its queue implicitly by being removed. Workers
/// building a since-removed site are reset elsewhere.
pub(crate) fn death_system(
    entities: &mut EntityStore,
    fog: &Fog,
    players: &mut [PlayerState],
    events: &mut HashMap<u32, Vec<Event>>,
) {
    let dead: Vec<(u32, u32, f32, f32, EntityKind, Option<u32>)> = entities
        .iter()
        .filter(|e| e.is_targetable() && e.hp == 0)
        .map(|e| {
            (
                e.id,
                e.owner,
                e.pos_x,
                e.pos_y,
                e.kind,
                e.last_damage_owner(),
            )
        })
        .collect();

    for (id, owner, x, y, kind, killer) in dead {
        entities.release_miner(id);
        entities.remove(id);
        record_score_death(players, owner, kind, killer);
        // Deliver the death only to players who owned the entity or could see where it died,
        // so a death poof never reveals an entity hidden in a player's fog.
        let pids: Vec<u32> = events.keys().copied().collect();
        for pid in pids {
            if !projection::event_visible_to(pid, x, y, owner, fog) {
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
            if !projection::event_visible_to(pid, x, y, 0, fog) {
                continue;
            }
            events.entry(pid).or_default().push(Event::Death {
                id,
                x,
                y,
                kind: kind.to_protocol_str().to_string(),
            });
        }
        entities.remove(id);
    }

    // Clear stale node reservations through the authoritative slot predicate.
    entities.clear_stale_miner_slots();

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
