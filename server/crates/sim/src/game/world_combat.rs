use std::collections::HashMap;

use crate::config;
use crate::protocol::Event;

const SIGNAL_QUANTUM_TICKS: u32 = 15;
const SIGNAL_HOLD_TICKS: u32 = 60;
const POSITION_GRID_TILES: f32 = 32.0;

pub(super) fn record_activity(
    events: &HashMap<u32, Vec<Event>>,
    tick: u32,
    world_size: f32,
    last_activity_tick: &mut Option<u32>,
    last_activity_position: &mut Option<[f32; 2]>,
    active_through_tick: &mut Option<u32>,
    published_position: &mut Option<[f32; 2]>,
) {
    if events.values().flatten().any(is_hostile_weapon_activity) {
        *last_activity_tick = Some(tick);
    }
    if let Some(position) = activity_centroid(events) {
        *last_activity_position = Some(quantize_position(position, world_size));
    }

    // Publish only on fixed boundaries. Keeping the published deadline separate
    // from exact activity prevents new combat inside a bucket from suppressing
    // an active signal or exposing live weapon cadence through direction updates.
    if tick.is_multiple_of(SIGNAL_QUANTUM_TICKS) {
        if let Some(last) = *last_activity_tick {
            *active_through_tick = Some(signal_deadline(last));
        }
        if let Some(position) = *last_activity_position {
            *published_position = Some(position);
        }
    }
}

pub(super) fn signal_position(
    tick: u32,
    active_through_tick: Option<u32>,
    published_position: Option<[f32; 2]>,
) -> Option<[f32; 2]> {
    let boundary = tick - tick % SIGNAL_QUANTUM_TICKS;
    active_through_tick
        .is_some_and(|deadline| boundary <= deadline)
        .then_some(published_position)
        .flatten()
}

pub(super) fn valid_checkpoint_signal_state(
    last_activity_tick: Option<u32>,
    last_activity_position: Option<[f32; 2]>,
    active_through_tick: Option<u32>,
    published_position: Option<[f32; 2]>,
    world_size: f32,
) -> bool {
    let deadline_valid = match (last_activity_tick, active_through_tick) {
        (_, None) => true,
        (Some(last), Some(deadline)) => {
            deadline % SIGNAL_QUANTUM_TICKS == 0 && deadline <= signal_deadline(last)
        }
        (None, Some(_)) => false,
    };
    deadline_valid
        && last_activity_position.is_none_or(|position| {
            last_activity_tick.is_some() && position_in_world(position, world_size)
        })
        && published_position.is_none_or(|position| {
            active_through_tick.is_some() && position_in_world(position, world_size)
        })
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

fn activity_centroid(events: &HashMap<u32, Vec<Event>>) -> Option<[f32; 2]> {
    let mut positions = events
        .values()
        .flatten()
        .filter_map(activity_position)
        .filter(|position| position[0].is_finite() && position[1].is_finite())
        .collect::<Vec<_>>();
    positions.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    positions.dedup_by(|a, b| a == b);
    if positions.is_empty() {
        return None;
    }
    let count = positions.len() as f32;
    Some([
        positions.iter().map(|position| position[0]).sum::<f32>() / count,
        positions.iter().map(|position| position[1]).sum::<f32>() / count,
    ])
}

fn activity_position(event: &Event) -> Option<[f32; 2]> {
    match event {
        Event::Attack {
            from,
            to,
            reveal,
            to_pos,
            ..
        } if from != to => reveal
            .as_ref()
            .map(|source| [source.x, source.y])
            .or(*to_pos),
        Event::MortarLaunch { from_x, from_y, .. }
        | Event::PanzerfaustLaunch { from_x, from_y, .. } => Some([*from_x, *from_y]),
        Event::MortarImpact { x, y, .. }
        | Event::ArtilleryTarget { x, y, .. }
        | Event::ArtilleryImpact { x, y, .. }
        | Event::PanzerfaustImpact { x, y } => Some([*x, *y]),
        _ => None,
    }
}

fn quantize_position(position: [f32; 2], world_size: f32) -> [f32; 2] {
    let grid = config::TILE_SIZE as f32 * POSITION_GRID_TILES;
    let max = (world_size - 1.0).max(0.0);
    [
        ((position[0] / grid).round() * grid).clamp(0.0, max),
        ((position[1] / grid).round() * grid).clamp(0.0, max),
    ]
}

fn position_in_world(position: [f32; 2], world_size: f32) -> bool {
    world_size.is_finite()
        && world_size > 0.0
        && position[0].is_finite()
        && position[1].is_finite()
        && position[0] >= 0.0
        && position[1] >= 0.0
        && position[0] < world_size
        && position[1] < world_size
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attack_events() -> HashMap<u32, Vec<Event>> {
        attack_events_at([1900.0, 1300.0])
    }

    fn attack_events_at(position: [f32; 2]) -> HashMap<u32, Vec<Event>> {
        HashMap::from([(
            1,
            vec![Event::Attack {
                from: 7,
                to: 9,
                reveal: None,
                to_pos: Some(position),
                weapon_kind: None,
            }],
        )])
    }

    #[test]
    fn signal_changes_only_at_quantized_boundaries_and_holds_activity() {
        let mut last = None;
        let mut last_position = None;
        let mut deadline = None;
        let mut position = None;
        record_activity(
            &attack_events(),
            17,
            4096.0,
            &mut last,
            &mut last_position,
            &mut deadline,
            &mut position,
        );
        assert_eq!(signal_position(29, deadline, position), None);
        record_activity(
            &HashMap::new(),
            30,
            4096.0,
            &mut last,
            &mut last_position,
            &mut deadline,
            &mut position,
        );
        assert_eq!(
            signal_position(30, deadline, position),
            Some([2048.0, 1024.0])
        );
        assert!(signal_position(89, deadline, position).is_some());
        assert_eq!(signal_position(90, deadline, position), None);
    }

    #[test]
    fn activity_between_boundaries_does_not_suppress_an_active_signal() {
        let mut last = None;
        let mut last_position = None;
        let mut deadline = None;
        let mut position = None;
        record_activity(
            &attack_events(),
            17,
            4096.0,
            &mut last,
            &mut last_position,
            &mut deadline,
            &mut position,
        );
        record_activity(
            &HashMap::new(),
            30,
            4096.0,
            &mut last,
            &mut last_position,
            &mut deadline,
            &mut position,
        );
        assert!(signal_position(30, deadline, position).is_some());

        record_activity(
            &attack_events_at([3072.0, 1024.0]),
            31,
            4096.0,
            &mut last,
            &mut last_position,
            &mut deadline,
            &mut position,
        );
        assert_eq!(
            signal_position(31, deadline, position),
            Some([2048.0, 1024.0])
        );
        assert_eq!(
            signal_position(44, deadline, position),
            Some([2048.0, 1024.0])
        );

        record_activity(
            &HashMap::new(),
            45,
            4096.0,
            &mut last,
            &mut last_position,
            &mut deadline,
            &mut position,
        );
        assert_eq!(
            signal_position(104, deadline, position),
            Some([3072.0, 1024.0])
        );
        assert_eq!(signal_position(105, deadline, position), None);
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
        let mut last_position = None;
        let mut deadline = None;
        let mut position = None;
        record_activity(
            &attack_events(),
            17,
            4096.0,
            &mut last,
            &mut last_position,
            &mut deadline,
            &mut position,
        );
        assert_eq!(last, Some(17));
        assert_eq!(last_position, Some([2048.0, 1024.0]));
        assert_eq!(deadline, None);
        record_activity(
            &HashMap::new(),
            30,
            4096.0,
            &mut last,
            &mut last_position,
            &mut deadline,
            &mut position,
        );
        assert_eq!(signal_position(30, deadline, position), last_position);
    }

    #[test]
    fn duplicate_projection_events_do_not_bias_the_coarse_combat_centroid() {
        let attack = Event::Attack {
            from: 7,
            to: 9,
            reveal: None,
            to_pos: Some([100.0, 100.0]),
            weapon_kind: None,
        };
        let events = HashMap::from([
            (
                1,
                vec![
                    attack.clone(),
                    Event::PanzerfaustImpact {
                        x: 3100.0,
                        y: 100.0,
                    },
                ],
            ),
            (2, vec![attack]),
        ]);
        let mut last = None;
        let mut last_position = None;
        let mut deadline = None;
        let mut position = None;
        record_activity(
            &events,
            15,
            4096.0,
            &mut last,
            &mut last_position,
            &mut deadline,
            &mut position,
        );
        assert_eq!(position, Some([2048.0, 0.0]));
    }

    #[test]
    fn checkpoint_signal_state_rejects_unbounded_or_orphaned_deadlines() {
        let point = Some([2048.0, 1024.0]);
        assert!(valid_checkpoint_signal_state(
            Some(17),
            point,
            Some(75),
            point,
            4096.0
        ));
        assert!(valid_checkpoint_signal_state(
            Some(31),
            point,
            Some(75),
            point,
            4096.0
        ));
        assert!(!valid_checkpoint_signal_state(
            Some(17),
            point,
            Some(76),
            point,
            4096.0
        ));
        assert!(!valid_checkpoint_signal_state(
            Some(17),
            point,
            Some(90),
            point,
            4096.0
        ));
        assert!(!valid_checkpoint_signal_state(
            None,
            point,
            Some(75),
            point,
            4096.0
        ));
        assert!(!valid_checkpoint_signal_state(
            Some(17),
            Some([f32::NAN, 0.0]),
            Some(75),
            point,
            4096.0
        ));
        assert!(!valid_checkpoint_signal_state(
            Some(17),
            point,
            Some(75),
            Some([4096.0, 0.0]),
            4096.0
        ));
    }
}
