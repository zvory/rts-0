use std::collections::HashMap;

use crate::protocol::Event;

const SIGNAL_QUANTUM_TICKS: u32 = 15;
const SIGNAL_HOLD_TICKS: u32 = 60;

pub(super) fn activity_tick(events: &HashMap<u32, Vec<Event>>, tick: u32) -> Option<u32> {
    events
        .values()
        .flatten()
        .any(is_hostile_weapon_activity)
        .then_some(tick)
}

pub(super) fn signal_active(tick: u32, last_activity_tick: Option<u32>) -> bool {
    let boundary = tick - tick % SIGNAL_QUANTUM_TICKS;
    last_activity_tick
        .is_some_and(|last| last <= boundary && boundary.saturating_sub(last) <= SIGNAL_HOLD_TICKS)
}

fn is_hostile_weapon_activity(event: &Event) -> bool {
    matches!(event, Event::Attack { from, to, .. } if from != to)
        || matches!(
            event,
            Event::MortarLaunch { .. }
                | Event::MortarImpact { .. }
                | Event::ArtilleryTarget { .. }
                | Event::ArtilleryImpact { .. }
                | Event::PanzerfaustLaunch { .. }
                | Event::PanzerfaustImpact { .. }
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_changes_only_at_quantized_boundaries_and_holds_activity() {
        assert!(!signal_active(29, Some(17)));
        assert!(signal_active(30, Some(17)));
        assert!(signal_active(89, Some(17)));
        assert!(!signal_active(90, Some(17)));
    }

    #[test]
    fn artillery_self_reveal_is_not_weapon_activity() {
        let event = Event::Attack {
            from: 7,
            to: 7,
            reveal: None,
            to_pos: None,
            weapon_kind: None,
        };
        assert!(!is_hostile_weapon_activity(&event));
    }

    #[test]
    fn one_projected_attack_activates_the_global_signal_without_recipient_detail() {
        let attack = Event::Attack {
            from: 7,
            to: 9,
            reveal: None,
            to_pos: None,
            weapon_kind: None,
        };
        let events = HashMap::from([(1, vec![attack])]);
        let last = activity_tick(&events, 17);
        assert_eq!(last, Some(17));
        assert!(signal_active(30, last));
    }
}
