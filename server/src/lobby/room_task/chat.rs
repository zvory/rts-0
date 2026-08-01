use std::time::{Duration, Instant as StdInstant};

use super::types::Phase;
use super::RoomTask;
use crate::protocol::{ChatChannel, ChatScope, ServerMessage};
use rts_sim::game::replay::ChatLogEntry;

const MAX_CHAT_CHARS: usize = 200;
const MAX_REPLAY_CHAT_ENTRIES: usize = 10_000;
const CHAT_RATE_WINDOW: Duration = Duration::from_secs(10);
const CHAT_RATE_BURST: usize = 5;

pub(super) fn sanitize_chat_text(text: String) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_CHAT_CHARS)
        .collect()
}

impl RoomTask {
    pub(super) fn on_chat_send(
        &mut self,
        player_id: u32,
        requested_channel: ChatChannel,
        text: String,
    ) {
        let Some(sender) = self.players.get(&player_id) else {
            return;
        };
        let sender_name = sender.name.clone();
        let sender_is_spectator = sender.spectator;
        let text = sanitize_chat_text(text);
        if text.is_empty() || !self.chat_rate_limit_allows(player_id) {
            return;
        }

        match &self.phase {
            Phase::Lobby | Phase::BranchStaging(_) => {
                self.broadcast(&ServerMessage::Chat {
                    scope: ChatScope::Lobby,
                    channel: ChatChannel::All,
                    tick: None,
                    sender_id: player_id,
                    sender_name,
                    text,
                });
            }
            Phase::ReplayViewer(_) => {}
            Phase::InGame(game) => {
                if self.match_chat_log.len() >= MAX_REPLAY_CHAT_ENTRIES {
                    return;
                }
                let sender_seat = self
                    .live_seat_id_for_connection(player_id)
                    .unwrap_or(player_id);
                let sender_team = (!sender_is_spectator)
                    .then(|| game.team_of_player(sender_seat))
                    .flatten();
                let channel = if requested_channel == ChatChannel::Team && sender_team.is_some() {
                    ChatChannel::Team
                } else {
                    ChatChannel::All
                };
                let tick = game.tick_count();
                let delivery = ServerMessage::Chat {
                    scope: ChatScope::Game,
                    channel,
                    tick: Some(tick),
                    sender_id: player_id,
                    sender_name: sender_name.clone(),
                    text: text.clone(),
                };
                let recipients = self.chat_recipients(channel, sender_team);
                self.send_chat_to(recipients, &delivery);
                self.match_chat_log.push(ChatLogEntry {
                    tick,
                    sender_id: player_id,
                    sender_name,
                    channel,
                    text,
                });
            }
        }
    }

    fn chat_rate_limit_allows(&mut self, player_id: u32) -> bool {
        let now = StdInstant::now();
        let recent = self.recent_chat_times.entry(player_id).or_default();
        while recent
            .front()
            .is_some_and(|sent_at| now.saturating_duration_since(*sent_at) >= CHAT_RATE_WINDOW)
        {
            recent.pop_front();
        }
        if recent.len() >= CHAT_RATE_BURST {
            return false;
        }
        recent.push_back(now);
        true
    }

    fn chat_recipients(&self, channel: ChatChannel, sender_team: Option<u32>) -> Vec<u32> {
        self.order
            .iter()
            .copied()
            .filter(|connection_id| {
                let Some(player) = self.players.get(connection_id) else {
                    return false;
                };
                if channel == ChatChannel::All {
                    return true;
                }
                if player.spectator {
                    return false;
                }
                let seat = self
                    .live_seat_id_for_connection(*connection_id)
                    .unwrap_or(*connection_id);
                let Phase::InGame(game) = &self.phase else {
                    return false;
                };
                game.team_of_player(seat) == sender_team
            })
            .collect()
    }

    pub(super) fn broadcast_replay_chat(&self, entries: Vec<ChatLogEntry>) {
        for entry in entries {
            self.broadcast(&ServerMessage::Chat {
                scope: ChatScope::Game,
                channel: entry.channel,
                tick: Some(entry.tick),
                sender_id: entry.sender_id,
                sender_name: entry.sender_name,
                text: entry.text,
            });
        }
    }

    fn send_chat_to(&self, recipients: Vec<u32>, message: &ServerMessage) {
        for player in recipients
            .into_iter()
            .filter_map(|id| self.players.get(&id).map(|player| (id, player)))
        {
            super::super::connection::send_or_log(
                &self.room,
                player.0,
                &player.1.msg_tx,
                message.clone(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_text_is_single_line_bounded_and_trimmed() {
        assert_eq!(
            sanitize_chat_text("  hello\n\tteam  ".to_string()),
            "hello team"
        );
        assert_eq!(
            sanitize_chat_text("x".repeat(250)).chars().count(),
            MAX_CHAT_CHARS
        );
        assert!(sanitize_chat_text(" \n\t ".to_string()).is_empty());
    }
}
