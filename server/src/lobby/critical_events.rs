use std::collections::HashMap;

use super::{connection::send_or_log, room_task::RoomPlayer};
use crate::protocol::{ChatChannel, ChatScope, ServerMessage};
use rts_sim::game::{entity::EntityKind, Game, PlayerCriticalEventState};

const WARNING_COOLDOWN_TICKS: u32 = 30 * rts_rules::balance::TICK_HZ;

#[derive(Default)]
pub(super) struct CriticalEventTracker {
    last_warning: HashMap<(u32, String), u32>,
}

enum Notification {
    Private { recipient: u32, text: String },
}

impl CriticalEventTracker {
    fn warning_allowed(&mut self, player: u32, key: String, tick: u32) -> bool {
        let last = self.last_warning.entry((player, key));
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
        for state in states {
            if state.supply_cap > 0
                && state.supply_used >= state.supply_cap
                && self.warning_allowed(state.player_id, "supply".into(), tick)
            {
                notifications.push(Notification::Private {
                    recipient: state.player_id,
                    text: format!(
                        "You have reached {} out of {} supply.",
                        state.supply_used, state.supply_cap
                    ),
                });
            }
            for (unit, prerequisite) in state.blocked_production {
                if self.warning_allowed(state.player_id, format!("{unit:?}:{prerequisite:?}"), tick)
                {
                    notifications.push(Notification::Private {
                        recipient: state.player_id,
                        text: format!(
                            "{} lost, {} production blocked.",
                            display_kind(prerequisite),
                            display_kind(unit)
                        ),
                    });
                }
            }
        }
        notifications
    }
}

fn display_kind(kind: EntityKind) -> String {
    match kind {
        EntityKind::AntiTankGun => "AT gun".into(),
        EntityKind::Steelworks => "Gun works".into(),
        EntityKind::Factory => "Vehicle works".into(),
        _ => {
            let name = format!("{kind:?}");
            let mut text = String::new();
            for (index, ch) in name.chars().enumerate() {
                if index > 0 && ch.is_uppercase() {
                    text.push(' ');
                }
                if index == 0 {
                    text.push(ch);
                } else {
                    text.extend(ch.to_lowercase());
                }
            }
            text
        }
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
    deliver_notifications(
        notifications,
        tick,
        room,
        players,
        order,
        seats,
    );
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
            blocked_production: vec![(EntityKind::AntiTankGun, EntityKind::EngineeringComplex)],
        }
    }

    #[test]
    fn critical_events_cooldowns_are_independent_and_exact() {
        let mut tracker = CriticalEventTracker::default();
        let initial = tracker.observe(1, vec![state()]);
        assert_eq!(initial.len(), 2);
        assert!(
            matches!(&initial[0], Notification::Private { recipient: 1, text }
            if text == "You have reached 300 out of 300 supply.")
        );
        assert!(
            matches!(&initial[1], Notification::Private { recipient: 1, text }
            if text == "Engineering complex lost, AT gun production blocked.")
        );
        assert!(tracker
            .observe(WARNING_COOLDOWN_TICKS, vec![state()])
            .is_empty());
        assert_eq!(
            tracker
                .observe(WARNING_COOLDOWN_TICKS + 1, vec![state()])
                .len(),
            2
        );
        let mut other = state();
        other.player_id = 2;
        assert_eq!(
            tracker
                .observe(WARNING_COOLDOWN_TICKS + 2, vec![state(), other])
                .len(),
            2
        );
    }

    #[test]
    fn critical_events_recovery_and_max_supply_do_not_give_false_advice() {
        let mut tracker = CriticalEventTracker::default();
        let mut current = state();
        current.blocked_production.clear();
        current.supply_used = rts_rules::balance::PLAYER_SUPPLY_CAP;
        current.supply_cap = current.supply_used;
        let notifications = tracker.observe(1, vec![current.clone()]);
        assert!(
            matches!(&notifications[0], Notification::Private { text, .. } if !text.contains("Upgrade"))
        );
        current.supply_used = 1;
        assert!(tracker.observe(1000, vec![current]).is_empty());
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
