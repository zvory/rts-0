use super::super::{map_catalog::active_slot_cap, LobbyJoinState, LobbySummary, LobbySummaryPhase};
use super::types::{Phase, RoomMode};
use super::RoomTask;
use crate::protocol::LobbyKind;

impl RoomTask {
    pub(super) fn lobby_kind(&self) -> LobbyKind {
        if matches!(self.phase, Phase::ReplayViewer(_))
            || matches!(self.mode, RoomMode::Replay { .. })
        {
            LobbyKind::Replay
        } else {
            LobbyKind::Normal
        }
    }

    pub(super) fn lobby_map_name(&self) -> String {
        match &self.phase {
            Phase::ReplayViewer(session) => session.start_metadata().map_name,
            Phase::Lobby => match &self.mode {
                RoomMode::Replay { artifact } => artifact.map_name.clone(),
                _ => self.selected_map.clone(),
            },
            Phase::InGame(_) | Phase::BranchStaging(_) => self.selected_map.clone(),
        }
    }

    pub(super) fn lobby_summary(&self) -> Option<LobbySummary> {
        let policy = self.session_policy();
        if !policy.is_public_lobby_browser_room() {
            return None;
        }
        let host_id = self.host_id?;
        let host_name = self
            .players
            .get(&host_id)
            .map(|player| player.name.clone())?;
        let kind = self.lobby_kind();
        let (phase, join_state, map) = if self.match_countdown_deadline.is_some() {
            (
                LobbySummaryPhase::Countdown,
                LobbyJoinState::Starting,
                self.selected_map.clone(),
            )
        } else {
            match &self.phase {
                Phase::Lobby => {
                    let map = self.lobby_map_name();
                    let join_state = if kind == LobbyKind::Replay
                        || self.total_player_count() >= active_slot_cap(&map)
                    {
                        LobbyJoinState::FullSpectatorOnly
                    } else {
                        LobbyJoinState::Open
                    };
                    (LobbySummaryPhase::Lobby, join_state, map)
                }
                Phase::InGame(_) => (
                    LobbySummaryPhase::InGame,
                    LobbyJoinState::InGame,
                    self.match_map_name.clone(),
                ),
                Phase::ReplayViewer(_) => (
                    LobbySummaryPhase::InGame,
                    LobbyJoinState::InGame,
                    self.lobby_map_name(),
                ),
                Phase::BranchStaging(_) => return None,
            }
        };
        let max_slots = if kind == LobbyKind::Replay {
            0
        } else {
            active_slot_cap(&map)
        };
        Some(LobbySummary {
            room: self.room.clone(),
            kind,
            host_name: Some(host_name),
            map,
            created_at_unix_ms: self.created_at_unix_ms,
            occupied_slots: if kind == LobbyKind::Replay {
                0
            } else {
                self.total_player_count()
            },
            max_slots,
            spectator_count: if kind == LobbyKind::Replay {
                self.players.len()
            } else {
                self.players
                    .values()
                    .filter(|player| player.spectator)
                    .count()
            },
            phase,
            join_state,
        })
    }
}
