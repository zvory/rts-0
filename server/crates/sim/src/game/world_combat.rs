use std::collections::HashMap;

use crate::protocol::Event;

const SIGNAL_QUANTUM_TICKS: u32 = 15;
const SIGNAL_HOLD_TICKS: u32 = 60;

pub(super) fn record_activity(
    events: &HashMap<u32, Vec<Event>>,
    tick: u32,
    last_activity_tick: &mut Option<u32>,
    active_through_tick: &mut Option<u32>,
) {
    if events.values().flatten().any(is_hostile_weapon_activity) {
        *last_activity_tick = Some(tick);
    }

    // Publish only on fixed boundaries. Keeping the published deadline separate
    // from the exact latest activity prevents new activity inside a bucket from
    // suppressing a signal that was already active.
    if tick.is_multiple_of(SIGNAL_QUANTUM_TICKS) {
        if let Some(last) = *last_activity_tick {
            *active_through_tick = Some(signal_deadline(last));
        }
    }
}

pub(super) fn signal_active(tick: u32, active_through_tick: Option<u32>) -> bool {
    let boundary = tick - tick % SIGNAL_QUANTUM_TICKS;
    active_through_tick.is_some_and(|deadline| boundary <= deadline)
}

pub(super) fn valid_checkpoint_signal_state(
    last_activity_tick: Option<u32>,
    active_through_tick: Option<u32>,
) -> bool {
    match (last_activity_tick, active_through_tick) {
        (_, None) => true,
        (Some(last), Some(deadline)) => {
            deadline % SIGNAL_QUANTUM_TICKS == 0 && deadline <= signal_deadline(last)
        }
        (None, Some(_)) => false,
    }
}

fn signal_deadline(last_activity_tick: u32) -> u32 {
    let deadline = last_activity_tick.saturating_add(SIGNAL_HOLD_TICKS);
    deadline - deadline % SIGNAL_QUANTUM_TICKS
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

    fn attack_events() -> HashMap<u32, Vec<Event>> {
        HashMap::from([(
            1,
            vec![Event::Attack {
                from: 7,
                to: 9,
                reveal: None,
                to_pos: None,
                weapon_kind: None,
            }],
        )])
    }

    #[test]
    fn signal_changes_only_at_quantized_boundaries_and_holds_activity() {
        let mut last = None;
        let mut deadline = None;
        record_activity(&attack_events(), 17, &mut last, &mut deadline);
        assert!(!signal_active(29, deadline));
        record_activity(&HashMap::new(), 30, &mut last, &mut deadline);
        assert!(signal_active(30, deadline));
        assert!(signal_active(89, deadline));
        assert!(!signal_active(90, deadline));
    }

    #[test]
    fn activity_between_boundaries_does_not_suppress_an_active_signal() {
        let mut last = None;
        let mut deadline = None;
        record_activity(&attack_events(), 17, &mut last, &mut deadline);
        record_activity(&HashMap::new(), 30, &mut last, &mut deadline);
        assert!(signal_active(30, deadline));

        record_activity(&attack_events(), 31, &mut last, &mut deadline);
        assert!(signal_active(31, deadline));
        assert!(signal_active(44, deadline));

        record_activity(&HashMap::new(), 45, &mut last, &mut deadline);
        assert!(signal_active(104, deadline));
        assert!(!signal_active(105, deadline));
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
        let mut last = None;
        let mut deadline = None;
        record_activity(&attack_events(), 17, &mut last, &mut deadline);
        assert_eq!(last, Some(17));
        assert_eq!(deadline, None);
        record_activity(&HashMap::new(), 30, &mut last, &mut deadline);
        assert!(signal_active(30, deadline));
    }

    #[test]
    fn checkpoint_signal_state_rejects_unbounded_or_orphaned_deadlines() {
        assert!(valid_checkpoint_signal_state(Some(17), Some(75)));
        assert!(valid_checkpoint_signal_state(Some(31), Some(75)));
        assert!(!valid_checkpoint_signal_state(Some(17), Some(76)));
        assert!(!valid_checkpoint_signal_state(Some(17), Some(90)));
        assert!(!valid_checkpoint_signal_state(None, Some(75)));
    }
}
