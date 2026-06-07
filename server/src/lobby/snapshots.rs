use crate::protocol::{kinds, Event, ResourceDelta, Snapshot};

/// Keep static resource positions in the start payload and send only compact visible remaining
/// updates in snapshots. Internal `Game::snapshot_for` still includes resource entities for
/// self-play/replay paths that consume snapshots directly.
pub fn compact_snapshot_for_wire(snapshot: &mut Snapshot) {
    for event in &snapshot.events {
        let Event::Death { id, kind, .. } = event else {
            continue;
        };
        if kind != kinds::STEEL && kind != kinds::OIL {
            continue;
        }
        if let Some(delta) = snapshot.resource_deltas.iter_mut().find(|d| d.id == *id) {
            delta.remaining = 0;
        } else {
            snapshot.resource_deltas.push(ResourceDelta {
                id: *id,
                remaining: 0,
            });
        }
    }
    snapshot.resource_deltas.sort_by_key(|d| d.id);
    snapshot
        .entities
        .retain(|entity| entity.kind != kinds::STEEL && entity.kind != kinds::OIL);
}

pub(super) fn union_events<'a>(event_sets: impl Iterator<Item = &'a Vec<Event>>) -> Vec<Event> {
    let mut events = Vec::new();
    for set in event_sets {
        for event in set {
            if !events.contains(event) {
                events.push(event.clone());
            }
        }
    }
    events
}
