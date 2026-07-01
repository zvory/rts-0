use crate::protocol::{kinds, Event, NoticeSeverity, ResourceDelta, Snapshot};

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
    union_events_matching(event_sets, |_| true)
}

pub(super) fn union_events_without_private_notices<'a>(
    event_sets: impl Iterator<Item = &'a Vec<Event>>,
) -> Vec<Event> {
    union_events_matching(event_sets, |event| {
        !private_notice_for_observer_union(event)
    })
}

fn union_events_matching<'a>(
    event_sets: impl Iterator<Item = &'a Vec<Event>>,
    include_event: impl Fn(&Event) -> bool,
) -> Vec<Event> {
    let mut events = Vec::new();
    for set in event_sets {
        for event in set {
            if !include_event(event) {
                continue;
            }
            if !events.contains(event) {
                events.push(event.clone());
            }
        }
    }
    events
}

fn private_notice_for_observer_union(event: &Event) -> bool {
    matches!(
        event,
        Event::Notice {
            msg,
            x: None,
            y: None,
            severity: NoticeSeverity::Info,
        } if !msg.starts_with("alert:")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info_notice(msg: &str) -> Event {
        Event::Notice {
            msg: msg.to_string(),
            severity: NoticeSeverity::Info,
            x: None,
            y: None,
        }
    }

    #[test]
    fn observer_event_union_filters_only_position_free_private_info_notices() {
        let private_notice = info_notice("Unknown unit");
        let alert_notice = info_notice("alert:under_attack");
        let positioned_notice = Event::Notice {
            msg: "Cannot build there".to_string(),
            severity: NoticeSeverity::Info,
            x: Some(160.0),
            y: Some(192.0),
        };
        let warning_notice = Event::Notice {
            msg: "Low oil".to_string(),
            severity: NoticeSeverity::Warn,
            x: None,
            y: None,
        };
        let duplicate_attack = Event::Attack {
            from: 1,
            to: 2,
            reveal: None,
            to_pos: None,
            weapon_kind: None,
        };
        let p1_events = vec![
            private_notice.clone(),
            alert_notice.clone(),
            positioned_notice.clone(),
            duplicate_attack.clone(),
        ];
        let p2_events = vec![warning_notice.clone(), duplicate_attack.clone()];

        let spectator_union =
            union_events_without_private_notices([&p1_events, &p2_events].into_iter());
        assert!(!spectator_union.contains(&private_notice));
        assert!(spectator_union.contains(&alert_notice));
        assert!(spectator_union.contains(&positioned_notice));
        assert!(spectator_union.contains(&warning_notice));
        assert_eq!(
            spectator_union
                .iter()
                .filter(|event| **event == duplicate_attack)
                .count(),
            1,
            "event unions should still dedupe shared events"
        );

        let selected_perspective_union = union_events([&p1_events, &p2_events].into_iter());
        assert!(
            selected_perspective_union.contains(&private_notice),
            "selected replay/lab perspectives keep private notices for the selected real players"
        );
    }
}
