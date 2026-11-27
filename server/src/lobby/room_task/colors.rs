use super::{Phase, RoomTask};
use crate::lobby::PLAYER_PALETTE;

impl RoomTask {
    pub(super) fn on_set_preferred_color(&mut self, player_id: u32, color: Option<String>) {
        if !matches!(
            self.phase,
            Phase::Lobby | Phase::InGame(_) | Phase::ReplayViewer(_)
        ) || !matches!(self.mode, super::RoomMode::Normal)
            || self.is_dev_watch()
            || self.is_replay_staging_lobby()
            || !self.players.contains_key(&player_id)
        {
            return;
        }
        if color
            .as_deref()
            .is_some_and(|value| !PLAYER_PALETTE.contains(&value))
        {
            return;
        }
        // Keep the lottery stable across broadcasts and preference changes: resending a
        // preference cannot reroll a conflict. Priorities are independent of join order.
        let entry = self
            .color_preferences
            .entry(player_id)
            .or_insert_with(|| (None, rand::random()));
        if entry.0 == color {
            return;
        }
        entry.0 = color;
        // Countdown and replay colors are frozen; retain edits for the next lobby.
        if matches!(self.phase, Phase::Lobby) && self.match_countdown_deadline.is_none() {
            self.broadcast_lobby();
        }
    }

    pub(super) fn resolve_preferred_colors(&mut self) {
        if !matches!(self.phase, Phase::Lobby) || self.match_countdown_deadline.is_some() {
            return;
        }
        let ids: Vec<u32> = self.active_human_ids().collect();
        let mut assigned = std::collections::HashMap::new();
        // Reserve every uncontested preference before assigning any fallback colors.
        for color in PLAYER_PALETTE {
            let winner = ids
                .iter()
                .filter_map(|id| {
                    let (preference, priority) = self.color_preferences.get(id)?;
                    (preference.as_deref() == Some(color)).then_some((*id, *priority))
                })
                .max_by_key(|(id, priority)| (*priority, *id));
            if let Some((id, _)) = winner {
                assigned.insert(id, color);
            }
        }
        // With no active preference, preserve the existing free-color/rejoin behavior.
        if assigned.is_empty() {
            return;
        }
        for id in ids {
            if !assigned.contains_key(&id) {
                if let Some(color) = PLAYER_PALETTE
                    .iter()
                    .copied()
                    .find(|color| !assigned.values().any(|used| used == color))
                {
                    assigned.insert(id, color);
                }
            }
        }
        for (id, color) in assigned {
            if let Some(player) = self.players.get_mut(&id) {
                player.color = color.to_string();
            }
        }
    }
}
