use std::collections::{HashMap, HashSet};

use super::room_task::RoomPlayer;

pub(super) struct Participants<'a> {
    order: &'a [u32],
    players: &'a HashMap<u32, RoomPlayer>,
    branch_live_seat_by_connection: &'a HashMap<u32, u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CommandIssuer {
    pub(super) connection_id: u32,
    pub(super) seat_id: u32,
}

impl<'a> Participants<'a> {
    pub(super) fn new(
        order: &'a [u32],
        players: &'a HashMap<u32, RoomPlayer>,
        branch_live_seat_by_connection: &'a HashMap<u32, u32>,
    ) -> Self {
        Self {
            order,
            players,
            branch_live_seat_by_connection,
        }
    }

    pub(super) fn host_with_fallback(&self, current_host: Option<u32>) -> Option<u32> {
        current_host
            .filter(|id| self.players.contains_key(id))
            .or_else(|| self.order.first().copied())
    }

    pub(super) fn active_human_count(&self) -> usize {
        self.active_human_ids().len()
    }

    pub(super) fn active_human_ids(&self) -> Vec<u32> {
        self.order
            .iter()
            .copied()
            .filter(|id| self.is_active_human(*id))
            .collect()
    }

    pub(super) fn active_seat_ids(&self, ai_ids: impl IntoIterator<Item = u32>) -> Vec<u32> {
        let mut ids = self.active_human_ids();
        ids.extend(ai_ids);
        ids
    }

    pub(super) fn spectator_visible_player_ids(
        &self,
        ai_ids: impl IntoIterator<Item = u32>,
    ) -> Vec<u32> {
        if !self.branch_live_seat_by_connection.is_empty() {
            return self
                .branch_live_seat_by_connection
                .values()
                .copied()
                .collect();
        }
        self.active_seat_ids(ai_ids)
    }

    pub(super) fn live_seat_id_for_connection(&self, connection_id: u32) -> Option<u32> {
        self.branch_live_seat_by_connection
            .get(&connection_id)
            .copied()
            .or_else(|| {
                self.players
                    .contains_key(&connection_id)
                    .then_some(connection_id)
            })
    }

    pub(super) fn live_connection_is_player(&self, connection_id: u32) -> bool {
        self.is_active_human(connection_id)
            && (self.branch_live_seat_by_connection.is_empty()
                || self
                    .branch_live_seat_by_connection
                    .contains_key(&connection_id))
    }

    pub(super) fn command_issuer_for_connection(
        &self,
        connection_id: u32,
        outcome_sent: &HashSet<u32>,
    ) -> Option<CommandIssuer> {
        if outcome_sent.contains(&connection_id) || !self.live_connection_is_player(connection_id) {
            return None;
        }
        Some(CommandIssuer {
            connection_id,
            seat_id: self.live_seat_id_for_connection(connection_id)?,
        })
    }

    fn is_active_human(&self, connection_id: u32) -> bool {
        self.players
            .get(&connection_id)
            .map(|player| !player.spectator)
            .unwrap_or(false)
    }
}
