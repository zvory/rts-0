use std::collections::{HashMap, HashSet};

use super::{connection::send_or_log, room_task::RoomPlayer};
use crate::protocol::{ChatChannel, ChatScope, ServerMessage};
use rts_sim::game::{entity::EntityKind, Game, PlayerCriticalEventState};

const WARNING_COOLDOWN_TICKS: u32 = 30 * rts_rules::balance::TICK_HZ;

#[derive(Default)]
pub(super) struct CriticalEventTracker {
    last_warning: HashMap<u32, u32>,
    previously_completed: HashSet<(u32, EntityKind)>,
    seeded: bool,
}

enum Notification {
    Private { recipient: u32, text: String },
}

impl CriticalEventTracker {
    pub(super) fn seed_initial_buildings(&mut self, game: &Game) {
        if self.seeded {
            return;
        }
        // A live replay branch can start with completed buildings under attack.
        for state in game.critical_event_state() {
            self.remember_completed(state.player_id, state.completed_buildings);
        }
        self.seeded = true;
    }

    fn remember_completed(&mut self, player: u32, buildings: Vec<EntityKind>) {
        self.previously_completed
            .extend(buildings.into_iter().map(|kind| (player, kind)));
    }

    fn warning_allowed(&mut self, player: u32, tick: u32) -> bool {
        let last = self.last_warning.entry(player);
        match last {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if tick.saturating_sub(*entry.get()) < WARNING_COOLDOWN_TICKS {
                    return false;
                }
                entry.insert(tick);
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(tick);
            }
        }
        true
    }

    fn observe(&mut self, tick: u32, states: Vec<PlayerCriticalEventState>) -> Vec<Notification> {
        let mut notifications = Vec::new();
        for mut state in states {
            self.remember_completed(state.player_id, state.completed_buildings);
            state
                .blocked_production
                .retain(|&(_, kind)| self.previously_completed.contains(&(state.player_id, kind)));
            if !state.blocked_production.is_empty() && self.warning_allowed(state.player_id, tick) {
                let mut prerequisites: Vec<_> = state
                    .blocked_production
                    .into_iter()
                    .map(|(_, kind)| prerequisite_name(kind))
                    .collect();
                prerequisites.sort_unstable();
                prerequisites.dedup();
                notifications.push(Notification::Private {
                    recipient: state.player_id,
                    text: format!("Rebuild {}!", prerequisites.join(", ")),
                });
            }
        }
        notifications
    }
}

fn prerequisite_name(kind: EntityKind) -> String {
    match kind {
        EntityKind::Steelworks => "Gun Works".into(),
        EntityKind::Factory => "Vehicle Works".into(),
        _ => kind
            .stable_id()
            .split('_')
            .map(|word| {
                let mut chars = word.chars();
                let mut name = String::new();
                if let Some(first) = chars.next() {
                    name.extend(first.to_uppercase());
                }
                name.extend(chars);
                name
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

pub(super) fn deliver(
    tracker: &mut CriticalEventTracker,
    game: &Game,
    room: &str,
    players: &HashMap<u32, RoomPlayer>,
    order: &[u32],
    seats: &HashMap<u32, u32>,
) {
    let tick = game.tick_count();
    let notifications = tracker.observe(tick, game.critical_event_state());
    if notifications.is_empty() {
        return;
    }
    deliver_notifications(notifications, tick, room, players, order, seats);
}

fn deliver_notifications(
    notifications: Vec<Notification>,
    tick: u32,
    room: &str,
    players: &HashMap<u32, RoomPlayer>,
    order: &[u32],
    seats: &HashMap<u32, u32>,
) {
    for notification in notifications {
        let Notification::Private { recipient, text } = notification;
        let message = ServerMessage::Chat {
            scope: ChatScope::Game,
            channel: ChatChannel::All,
            tick: Some(tick),
            sender_id: 0,
            sender_name: "Game".into(),
            text,
        };
        for &id in order {
            let Some(player) = players.get(&id) else {
                continue;
            };
            if player.spectator || seats.get(&id).copied().unwrap_or(id) != recipient {
                continue;
            }
            send_or_log(room, id, &player.msg_tx, message.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> PlayerCriticalEventState {
        PlayerCriticalEventState {
            player_id: 1,
            supply_used: 300,
            supply_cap: 300,
            completed_buildings: vec![],
            blocked_production: vec![
                (EntityKind::AntiTankGun, EntityKind::EngineeringComplex),
                (EntityKind::CommandCar, EntityKind::EngineeringComplex),
            ],
        }
    }

    fn completed(player_id: u32, kinds: &[EntityKind]) -> PlayerCriticalEventState {
        PlayerCriticalEventState {
            player_id,
            completed_buildings: kinds.to_vec(),
            blocked_production: vec![],
            ..state()
        }
    }

    #[test]
    fn critical_events_require_prior_completion_by_the_same_player() {
        let mut tracker = CriticalEventTracker::default();
        let engineering = EntityKind::EngineeringComplex;
        assert!(tracker.observe(1, vec![state()]).is_empty());
        assert!(tracker
            .observe(2, vec![state(), completed(2, &[engineering])])
            .is_empty());
        assert!(tracker.observe(3, vec![state()]).is_empty());
        assert!(tracker
            .observe(4, vec![completed(1, &[engineering])])
            .is_empty());

        let mut current = state();
        current
            .blocked_production
            .push((EntityKind::MachineGunner, EntityKind::TrainingCentre));
        let notifications = tracker.observe(5, vec![current]);
        assert!(matches!(
            notifications.as_slice(),
            [Notification::Private { recipient: 1, text }] if text == "Rebuild Engineering Complex!"
        ));
        assert!(tracker
            .observe(WARNING_COOLDOWN_TICKS + 5, vec![completed(1, &[])])
            .is_empty());
    }

    #[test]
    fn critical_events_cooldowns_are_independent_and_exact() {
        let mut tracker = CriticalEventTracker::default();
        let engineering = EntityKind::EngineeringComplex;
        assert!(tracker
            .observe(
                0,
                vec![completed(1, &[engineering]), completed(2, &[engineering])]
            )
            .is_empty());
        let initial = tracker.observe(1, vec![state()]);
        assert_eq!(initial.len(), 1);
        assert!(
            matches!(&initial[0], Notification::Private { recipient: 1, text }
            if text == "Rebuild Engineering Complex!")
        );
        assert!(tracker
            .observe(WARNING_COOLDOWN_TICKS, vec![state()])
            .is_empty());
        assert_eq!(
            tracker
                .observe(WARNING_COOLDOWN_TICKS + 1, vec![state()])
                .len(),
            1
        );
        let mut other = state();
        other.player_id = 2;
        assert_eq!(
            tracker
                .observe(WARNING_COOLDOWN_TICKS + 2, vec![state(), other])
                .len(),
            1
        );
    }

    #[test]
    fn critical_events_recovery_and_max_supply_do_not_give_false_advice() {
        let mut tracker = CriticalEventTracker::default();
        let mut current = state();
        current.blocked_production.clear();
        current.supply_used = rts_rules::balance::PLAYER_SUPPLY_CAP;
        current.supply_cap = current.supply_used;
        assert!(tracker.observe(1, vec![current.clone()]).is_empty());
        current.supply_used = 1;
        assert!(tracker.observe(1000, vec![current]).is_empty());
    }

    #[test]
    fn critical_events_name_the_actual_missing_prerequisites() {
        for (unit, prerequisite, expected) in [
            (
                EntityKind::MachineGunner,
                EntityKind::TrainingCentre,
                "Rebuild Training Centre!",
            ),
            (
                EntityKind::RocketLauncher,
                EntityKind::Factory,
                "Rebuild Vehicle Works!",
            ),
        ] {
            let mut current = state();
            current.blocked_production = vec![(unit, prerequisite)];
            let mut tracker = CriticalEventTracker::default();
            tracker.observe(0, vec![completed(1, &[prerequisite])]);
            let notifications = tracker.observe(1, vec![current]);
            assert_eq!(notifications.len(), 1);
            assert!(
                matches!(&notifications[0], Notification::Private { text, .. }
                if text == expected)
            );
        }

        let mut current = state();
        current.blocked_production.extend([
            (EntityKind::RocketLauncher, EntityKind::Factory),
            (EntityKind::MachineGunner, EntityKind::TrainingCentre),
        ]);
        let mut tracker = CriticalEventTracker::default();
        tracker.observe(
            0,
            vec![completed(
                1,
                &[
                    EntityKind::EngineeringComplex,
                    EntityKind::Factory,
                    EntityKind::TrainingCentre,
                ],
            )],
        );
        let notifications = tracker.observe(1, vec![current]);
        assert_eq!(notifications.len(), 1);
        assert!(
            matches!(&notifications[0], Notification::Private { text, .. }
            if text == "Rebuild Engineering Complex, Training Centre, Vehicle Works!")
        );
    }
}

#[cfg(test)]
mod delivery_tests {
    use super::super::connection::ConnectionSink;
    use super::*;

    #[test]
    fn critical_events_private_routing_excludes_opponents_and_spectators() {
        let mut players = HashMap::new();
        let mut writers = Vec::new();
        for id in [11, 2, 99] {
            let (msg_tx, writer) = ConnectionSink::new();
            players.insert(
                id,
                RoomPlayer {
                    name: format!("Player {id}"),
                    color: "#0072b2".into(),
                    ready: true,
                    spectator: id == 99,
                    msg_tx,
                    head_of_line_count: 0,
                    last_received_client_seq: 0,
                    last_sim_consumed_client_seq: 0,
                    last_sim_consumed_client_tick: None,
                },
            );
            writers.push(writer);
        }
        let seats = HashMap::from([(11, 1)]);
        deliver_notifications(
            vec![Notification::Private {
                recipient: 1,
                text: "Private warning".into(),
            }],
            7,
            "test",
            &players,
            &[11, 2, 99],
            &seats,
        );
        assert!(
            matches!(writers[0].reliable_rx.try_recv(), Ok(ServerMessage::Chat { sender_id: 0, sender_name, text, .. })
            if sender_name == "Game" && text == "Private warning")
        );
        assert!(writers[1].reliable_rx.try_recv().is_err());
        assert!(writers[2].reliable_rx.try_recv().is_err());
    }
}
