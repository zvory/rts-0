use std::time::{Duration, Instant as StdInstant};

use super::types::Phase;
use super::RoomTask;
use crate::lobby::replay_session::MAX_CHAT_LOG_ENTRIES;
use crate::protocol::chat::{MAX_CHAT_CHARS, MAX_CHAT_INPUT_CHARS};
use crate::protocol::{ChatChannel, ChatScope, ServerMessage};
use rts_sim::game::replay::ChatLogEntry;

// Chat shares the larger Lab-capable WebSocket frame allowance. Bound the amount of player text
// inspected on the room task as well as the delivered result, so a whitespace-heavy frame cannot
// monopolize the authoritative room loop.
const CHAT_RATE_WINDOW: Duration = Duration::from_secs(10);
const CHAT_RATE_BURST: usize = 5;

pub(super) fn sanitize_chat_text(text: String) -> String {
    let mut sanitized = String::with_capacity(text.len().min(MAX_CHAT_CHARS));
    let mut char_count = 0;
    let mut pending_space = false;

    for ch in text.chars().take(MAX_CHAT_INPUT_CHARS) {
        if ch.is_whitespace() {
            pending_space = !sanitized.is_empty();
            continue;
        }
        if pending_space {
            // Do not spend the final slot on whitespace: the bounded result stays trimmed even
            // when truncation lands between words.
            if char_count + 1 >= MAX_CHAT_CHARS {
                break;
            }
            sanitized.push(' ');
            char_count += 1;
            pending_space = false;
        }
        sanitized.push(ch);
        char_count += 1;
        if char_count == MAX_CHAT_CHARS {
            break;
        }
    }

    sanitized
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
                if self.match_chat_log.len() < MAX_CHAT_LOG_ENTRIES {
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
        assert_eq!(
            sanitize_chat_text(format!("{} y", "x".repeat(MAX_CHAT_CHARS - 1))),
            "x".repeat(MAX_CHAT_CHARS - 1)
        );
        assert_eq!(
            sanitize_chat_text(format!("ok {}ignored", " \n\t".repeat(100_000))),
            "ok"
        );
        assert!(sanitize_chat_text(" \n\t ".to_string()).is_empty());
    }
}
