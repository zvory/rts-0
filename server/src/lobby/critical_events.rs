use std::collections::{HashMap, HashSet};

use super::{connection::send_or_log, replay_session::MAX_CHAT_LOG_ENTRIES, room_task::RoomPlayer};
use crate::protocol::{ChatChannel, ChatScope, ServerMessage};
use rts_sim::game::{entity::EntityKind, replay::ChatLogEntry, Game, PlayerCriticalEventState};

const WARNING_COOLDOWN_TICKS: u32 = 30 * rts_rules::balance::TICK_HZ;

#[derive(Default)]
pub(super) struct CriticalEventTracker {
    last_warning: HashMap<(u32, String), u32>,
    upgrading_depots: HashSet<u32>,
}

enum Notification {
    Private { recipient: u32, text: String },
    DepotUpgrade { player_id: u32 },
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
        let mut upgrading = HashSet::new();
        for state in states {
            if state.supply_cap > 0
                && state.supply_used >= state.supply_cap
                && self.warning_allowed(state.player_id, "supply".into(), tick)
            {
                let advice = if state.supply_cap < rts_rules::balance::PLAYER_SUPPLY_CAP {
                    " Upgrade resource depot to increase supply cap."
                } else {
                    ""
                };
                notifications.push(Notification::Private {
                    recipient: state.player_id,
                    text: format!(
                        "You have reached {} out of {} supply.{advice}",
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
            for depot in state.upgrading_depots {
                upgrading.insert(depot);
                if !self.upgrading_depots.contains(&depot) {
                    notifications.push(Notification::DepotUpgrade {
                        player_id: state.player_id,
                    });
                }
            }
        }
        self.upgrading_depots = upgrading;
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

fn color_name(color: &str) -> &str {
    match color {
        "#0072b2" => "Blue",
        "#d55e00" => "Orange",
        "#009e73" => "Green",
        "#cc79a7" => "Pink",
        "#56b4e9" => "Sky blue",
        "#e69f00" => "Amber",
        "#f0e442" => "Yellow",
        "#7e57c2" => "Purple",
        _ => color,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn deliver(
    tracker: &mut CriticalEventTracker,
    game: &Game,
    room: &str,
    players: &HashMap<u32, RoomPlayer>,
    order: &[u32],
    seats: &HashMap<u32, u32>,
    chat_log: &mut Vec<ChatLogEntry>,
) {
    let tick = game.tick_count();
    let notifications = tracker.observe(tick, game.critical_event_state());
    if notifications.is_empty() {
        return;
    }
    let identities = game.player_inits();
    deliver_notifications(
        notifications,
        tick,
        room,
        players,
        order,
        seats,
        chat_log,
        &identities,
    );
}

#[allow(clippy::too_many_arguments)]
fn deliver_notifications(
    notifications: Vec<Notification>,
    tick: u32,
    room: &str,
    players: &HashMap<u32, RoomPlayer>,
    order: &[u32],
    seats: &HashMap<u32, u32>,
    chat_log: &mut Vec<ChatLogEntry>,
    identities: &[rts_sim::game::PlayerInit],
) {
    for notification in notifications {
        let (recipient, text) = match notification {
            Notification::Private { recipient, text } => (Some(recipient), text),
            Notification::DepotUpgrade { player_id } => {
                let Some(player) = identities.iter().find(|player| player.id == player_id) else {
                    continue;
                };
                (
                    None,
                    format!(
                        "{} ({}) has upgraded a resource depot to an Industrial Center.",
                        color_name(&player.color),
                        player.name
                    ),
                )
            }
        };
        let message = ServerMessage::Chat {
            scope: ChatScope::Game,
            channel: ChatChannel::All,
            tick: Some(tick),
            sender_id: 0,
            sender_name: "Game".into(),
            text: text.clone(),
        };
        for &id in order {
            let Some(player) = players.get(&id) else {
                continue;
            };
            if recipient.is_some_and(|recipient| {
                player.spectator || seats.get(&id).copied().unwrap_or(id) != recipient
            }) {
                continue;
            }
            send_or_log(room, id, &player.msg_tx, message.clone());
        }
        // Only public announcements are retained; private warnings never enter observer/replay chat.
        if recipient.is_none() && chat_log.len() < MAX_CHAT_LOG_ENTRIES {
            chat_log.push(ChatLogEntry {
                tick,
                sender_id: 0,
                sender_name: "Game".into(),
                channel: ChatChannel::All,
                text,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> PlayerCriticalEventState {
        PlayerCriticalEventState {
            player_id: 1,
            supply_used: 100,
            supply_cap: 100,
            blocked_production: vec![(EntityKind::AntiTankGun, EntityKind::EngineeringComplex)],
            upgrading_depots: vec![42],
        }
    }

    #[test]
    fn critical_events_cooldowns_are_independent_and_exact() {
        let mut tracker = CriticalEventTracker::default();
        let initial = tracker.observe(1, vec![state()]);
        assert_eq!(initial.len(), 3);
        assert!(
            matches!(&initial[0], Notification::Private { recipient: 1, text }
            if text == "You have reached 100 out of 100 supply. Upgrade resource depot to increase supply cap.")
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
        other.upgrading_depots.clear();
        assert_eq!(
            tracker
                .observe(WARNING_COOLDOWN_TICKS + 2, vec![state(), other])
                .len(),
            2
        );
    }

    #[test]
    fn critical_events_depot_announces_each_start_including_restart() {
        let mut tracker = CriticalEventTracker::default();
        let mut current = state();
        current.supply_used = 1;
        current.blocked_production.clear();
        assert_eq!(tracker.observe(1, vec![current.clone()]).len(), 1);
        assert!(tracker.observe(2, vec![current.clone()]).is_empty());
        current.upgrading_depots.push(43);
        assert_eq!(tracker.observe(3, vec![current.clone()]).len(), 1);
        current.upgrading_depots.clear();
        assert!(tracker.observe(4, vec![current.clone()]).is_empty());
        current.upgrading_depots.push(42);
        assert_eq!(tracker.observe(5, vec![current]).len(), 1);
    }

    #[test]
    fn critical_events_recovery_and_max_supply_do_not_give_false_advice() {
        let mut tracker = CriticalEventTracker::default();
        let mut current = state();
        current.upgrading_depots.clear();
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
    fn critical_events_private_routing_and_public_replay_retention() {
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
        let identities = vec![rts_sim::game::PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".into(),
            name: "Alice".into(),
            color: "#0072b2".into(),
            is_ai: false,
        }];
        let seats = HashMap::from([(11, 1)]);
        let mut log = Vec::new();
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
            &mut log,
            &identities,
        );
        assert!(
            matches!(writers[0].reliable_rx.try_recv(), Ok(ServerMessage::Chat { sender_id: 0, sender_name, text, .. })
            if sender_name == "Game" && text == "Private warning")
        );
        assert!(writers[1].reliable_rx.try_recv().is_err());
        assert!(writers[2].reliable_rx.try_recv().is_err());
        assert!(log.is_empty());
        deliver_notifications(
            vec![Notification::DepotUpgrade { player_id: 1 }],
            8,
            "test",
            &players,
            &[11, 2, 99],
            &seats,
            &mut log,
            &identities,
        );
        for writer in &mut writers {
            assert!(
                matches!(writer.reliable_rx.try_recv(), Ok(ServerMessage::Chat { sender_id: 0, text, .. })
                if text == "Blue (Alice) has upgraded a resource depot to an Industrial Center.")
            );
        }
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].tick, 8);
    }
}
