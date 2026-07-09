use std::collections::HashMap;

use rts_ai::DEFAULT_LIVE_PROFILE_REQUEST_ID;
use rts_sim::game::map::Map;

use super::super::connection::{send_or_log, ConnectionSink};
use super::super::faction_validation::{
    default_faction_id_for, validate_faction_request, FactionRequestContext, FactionValidation,
};
use super::super::participants::{CommandIssuer, Participants};
use super::super::{
    map_catalog, next_player_id, LobbyJoinState, LobbySummary, LobbySummaryPhase, MAX_PLAYERS,
    PLAYER_PALETTE,
};
use super::helpers::DRAINING_NEW_MATCHES_DISABLED_MSG;
use super::types::{AiSlot, Phase, RoomPlayer, MAX_LOBBY_TEAMS};
use super::RoomTask;
use crate::protocol::{LobbyKind, LobbyPlayer, ServerMessage, TeamId};

impl RoomTask {
    pub(super) fn is_replay_staging_lobby(&self) -> bool {
        matches!(self.mode, super::RoomMode::Replay { .. }) && matches!(self.phase, Phase::Lobby)
    }

    fn lobby_kind(&self) -> LobbyKind {
        if matches!(self.mode, super::RoomMode::Replay { .. }) {
            LobbyKind::Replay
        } else {
            LobbyKind::Normal
        }
    }

    fn lobby_map_name(&self) -> String {
        match &self.mode {
            super::RoomMode::Replay { artifact } if matches!(self.phase, Phase::Lobby) => {
                artifact.map_name.clone()
            }
            _ => self.selected_map.clone(),
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
                        || self.total_player_count() >= map_catalog::active_slot_cap(&map)
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
                Phase::ReplayViewer(_) | Phase::BranchStaging(_) => return None,
            }
        };
        let max_slots = if kind == LobbyKind::Replay {
            0
        } else {
            map_catalog::active_slot_cap(&map)
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
            spectator_count: self
                .players
                .values()
                .filter(|player| player.spectator)
                .count(),
            phase,
            join_state,
        })
    }

    pub(in crate::lobby) fn on_join(
        &mut self,
        player_id: u32,
        name: String,
        spectator: bool,
        replay_ok: bool,
        msg_tx: ConnectionSink,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        let policy = self.session_policy();
        if policy.is_dev_watch() {
            self.on_join_dev_watch(player_id, name, msg_tx, ack);
            return;
        }
        if policy.uses_replay_lobby_join() {
            self.on_join_replay_lobby(player_id, name, msg_tx, ack);
            return;
        }
        if policy.uses_replay_room_join() {
            if !replay_ok {
                self.prompt_for_replay_join(player_id, &msg_tx, ack);
                return;
            }
            self.on_join_replay_room(player_id, name, msg_tx, ack);
            return;
        }
        if policy.uses_branch_staging_join() {
            self.on_join_branch_staging(player_id, name, msg_tx, ack);
            return;
        }
        if policy.uses_branch_live_attach() {
            self.on_join_branch_live(player_id, name, msg_tx, ack);
            return;
        }
        if policy.uses_lab_room_join() {
            self.on_join_lab(player_id, name, msg_tx, ack);
            return;
        }
        if matches!(self.phase, Phase::ReplayViewer(_)) {
            if !replay_ok {
                self.prompt_for_replay_join(player_id, &msg_tx, ack);
                return;
            }
            self.on_join_replay_viewer(player_id, name, msg_tx, ack);
            return;
        }
        if self.match_countdown_deadline.is_some() {
            send_or_log(
                &self.room,
                player_id,
                &msg_tx,
                ServerMessage::Error {
                    msg: "Match is starting in this room — try another room.".to_string(),
                },
            );
            crate::log_debug!(room = %self.room, player_id, "rejecting join; match countdown active");
            let _ = ack.send(false);
            return;
        }
        if self.players.contains_key(&player_id) {
            // Defensive: a connection should only ever join once.
            let _ = ack.send(false);
            return;
        }
        if !matches!(self.phase, Phase::Lobby) {
            if policy.allows_live_spectator_attach() {
                self.on_join_live_spectator(player_id, name, spectator, msg_tx, ack);
                return;
            }
            send_or_log(
                &self.room,
                player_id,
                &msg_tx,
                ServerMessage::Error {
                    msg: "Match already in progress in this room — try another room.".to_string(),
                },
            );
            crate::log_debug!(room = %self.room, player_id, "rejecting join; match in progress");
            let _ = ack.send(false);
            return;
        }
        if !spectator
            && self.total_player_count() >= map_catalog::active_slot_cap(&self.selected_map)
        {
            send_or_log(
                &self.room,
                player_id,
                &msg_tx,
                ServerMessage::Error {
                    msg: "Lobby is full — try another room.".to_string(),
                },
            );
            crate::log_debug!(room = %self.room, player_id, "rejecting join; lobby full");
            let _ = ack.send(false);
            return;
        }
        let color = if spectator {
            "#6f8fa8".to_string()
        } else {
            self.next_human_color()
        };
        self.order.push(player_id);
        self.players.insert(
            player_id,
            RoomPlayer {
                name,
                color,
                ready: false,
                spectator,
                msg_tx,
                head_of_line_count: 0,
                last_received_client_seq: 0,
                last_sim_consumed_client_seq: 0,
                last_sim_consumed_client_tick: None,
            },
        );
        self.reassign_host_if_needed();
        if !spectator {
            self.assign_missing_team_for(player_id);
            self.assign_missing_faction_for(player_id);
        }
        crate::log_debug!(room = %self.room, player_id, "joined");
        // The player is now in the room; tell the connection it may mark itself joined.
        let _ = ack.send(true);
        self.send_current_shutdown_warning_to(player_id);
        if matches!(self.phase, Phase::Lobby) {
            self.broadcast_lobby();
        }
    }

    pub(in crate::lobby) fn on_leave(&mut self, player_id: u32) {
        let Some(removed) = self.players.remove(&player_id) else {
            return;
        };
        let was_spectator = removed.spectator;
        self.order.retain(|&id| id != player_id);
        self.human_team_assignments.remove(&player_id);
        self.human_faction_assignments.remove(&player_id);
        self.pending_recipient_notices.remove(&player_id);
        if let Some(session) = &mut self.lab_session {
            session.remove_viewer(player_id);
        }
        self.outcome_sent.remove(&player_id);
        self.reassign_host_if_needed();
        crate::log_debug!(room = %self.room, player_id, "left");

        // If the room emptied out, fully reset it so teardown bookkeeping is complete before any
        // disposable registry handle is dropped.
        if self.players.is_empty() {
            let dispose_empty_room = self.should_dispose_when_empty();
            self.mark_match_finished_for_drain();
            self.reset_empty_room_state();
            if dispose_empty_room {
                self.report_disposable_if_empty();
            }
            crate::log_debug!(room = %self.room, "room emptied; reset to lobby");
            return;
        }

        let mut broadcast_branch_staging = false;
        let removed_live_seat_id = (!was_spectator).then(|| {
            self.live_seat_id_for_connection(player_id)
                .unwrap_or(player_id)
        });
        match &mut self.phase {
            Phase::Lobby => self.broadcast_lobby(),
            Phase::InGame(game) => {
                // Remove their army so the match can still resolve to a winner.
                if let Some(seat_id) = removed_live_seat_id {
                    game.eliminate(seat_id);
                }
                self.branch_live_seat_by_connection.remove(&player_id);
            }
            Phase::ReplayViewer(session) => {
                session.remove_viewer(player_id);
            }
            Phase::BranchStaging(staging) => {
                staging.release_occupant(player_id);
                broadcast_branch_staging = true;
            }
        }
        if broadcast_branch_staging {
            self.broadcast_branch_staging();
        }
    }

    pub(in crate::lobby) fn on_ready(&mut self, player_id: u32, ready: bool) {
        if self.is_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if let Phase::Lobby = self.phase {
            if let Some(player) = self.players.get_mut(&player_id) {
                if player.spectator {
                    return;
                }
                player.ready = ready;
                self.broadcast_lobby();
            }
        }
    }

    pub(in crate::lobby) fn on_start_request(&mut self, player_id: u32) {
        if self.is_dev_watch() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if self.is_replay_staging_lobby() {
            self.on_start_replay_lobby_request(player_id);
            return;
        }
        if self.new_live_session_blocked_by_drain() {
            if let Some(player) = self.players.get(&player_id) {
                send_or_log(
                    &self.room,
                    player_id,
                    &player.msg_tx,
                    ServerMessage::Error {
                        msg: DRAINING_NEW_MATCHES_DISABLED_MSG.to_string(),
                    },
                );
            }
            crate::log_debug!(room = %self.room, player_id, "ignoring start while server is draining");
            return;
        }
        if self.host_id != Some(player_id) {
            crate::log_debug!(room = %self.room, player_id, "ignoring start from non-host");
            return;
        }
        if !self.can_start() {
            crate::log_debug!(room = %self.room, "ignoring start; not all players ready");
            return;
        }
        if self.should_skip_match_countdown() {
            self.start_match();
            return;
        }
        self.start_match_countdown();
    }

    pub(super) fn on_set_team_preset(&mut self, player_id: u32, preset: String) {
        if self.is_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        crate::log_debug!(room = %self.room, preset = %preset, "ignoring deprecated team preset command");
    }

    pub(super) fn on_set_team(&mut self, player_id: u32, target: u32, team_id: TeamId) {
        if self.is_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        if self.is_replay_staging_lobby() {
            return;
        }
        if team_id == 0 {
            crate::log_debug!(room = %self.room, target, "ignoring zero team id");
            return;
        }
        if self
            .players
            .get(&target)
            .map(|player| player.spectator)
            .unwrap_or(false)
        {
            crate::log_debug!(room = %self.room, target, "ignoring spectator team assignment");
            return;
        }
        let known_target = self.human_team_assignments.contains_key(&target)
            || self.ai_players.iter().any(|ai| ai.id == target);
        if !known_target || !self.team_move_allowed(target, team_id) {
            crate::log_debug!(room = %self.room, target, team_id, "ignoring invalid team assignment");
            return;
        }
        if let Some(ai) = self.ai_players.iter_mut().find(|ai| ai.id == target) {
            ai.team_id = team_id;
        } else if self.players.contains_key(&target) {
            self.human_team_assignments.insert(target, team_id);
        }
        self.broadcast_lobby();
    }

    /// Active humans can select their own playable faction in the lobby. The server validates and
    /// ignores unknown, fixture, spectator, countdown, and in-game requests.
    pub(super) fn on_set_faction(&mut self, player_id: u32, faction_id: String) {
        if self.is_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) {
            return;
        }
        if self
            .players
            .get(&player_id)
            .map(|player| player.spectator)
            .unwrap_or(true)
        {
            crate::log_debug!(room = %self.room, player_id, "ignoring spectator faction selection");
            return;
        }
        let accepted =
            match validate_faction_request(FactionRequestContext::NormalLobby, Some(&faction_id)) {
                FactionValidation::AcceptedPlayable { faction_id }
                | FactionValidation::Defaulted { faction_id } => faction_id,
                FactionValidation::AcceptedFixture { .. } => return,
                FactionValidation::Rejected { requested, reason } => {
                    crate::log_debug!(
                        room = %self.room,
                        player_id,
                        faction_id = ?requested,
                        reason = ?reason,
                        "ignoring invalid faction selection"
                    );
                    return;
                }
            };
        if self.human_faction_for(player_id) == accepted {
            return;
        }
        self.human_faction_assignments.insert(player_id, accepted);
        self.broadcast_lobby();
    }

    /// Host-only: seat a computer opponent. Ignored outside the lobby, from non-hosts, or once
    /// the selected map's active seat cap is full.
    pub(super) fn on_add_ai(
        &mut self,
        player_id: u32,
        requested_team_id: Option<TeamId>,
        requested_profile_id: Option<String>,
    ) {
        if self.is_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        if self.is_replay_staging_lobby() {
            return;
        }
        if self.total_player_count() >= map_catalog::active_slot_cap(&self.selected_map) {
            crate::log_debug!(room = %self.room, "ignoring add-ai; room full");
            return;
        }
        let id = next_player_id();
        let profile_request_id = requested_profile_id
            .as_deref()
            .and_then(rts_ai::canonical_live_profile_request_id)
            .unwrap_or(DEFAULT_LIVE_PROFILE_REQUEST_ID);
        let team_id = if let Some(team_id) = requested_team_id {
            if !self.team_move_allowed(id, team_id) {
                crate::log_debug!(room = %self.room, team_id, "ignoring invalid AI team assignment");
                return;
            }
            team_id
        } else {
            self.next_default_team_for_new_seat(id)
        };
        self.ai_players.push(AiSlot {
            id,
            team_id,
            faction_id: default_faction_id_for(FactionRequestContext::AiSeat),
            profile_request_id,
        });
        crate::log_debug!(room = %self.room, ai_id = id, "AI opponent added");
        self.broadcast_lobby();
    }

    /// Host-only: select which supported live AI profile or suite request an AI opponent will use
    /// next match.
    pub(super) fn on_set_ai_profile(
        &mut self,
        player_id: u32,
        target: u32,
        requested_profile_id: String,
    ) {
        if self.is_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        if self.is_replay_staging_lobby() {
            return;
        }
        let Some(profile_request_id) =
            rts_ai::canonical_live_profile_request_id(&requested_profile_id)
        else {
            crate::log_debug!(
                room = %self.room,
                target,
                ai_profile_id = %requested_profile_id,
                "ignoring invalid AI profile selection"
            );
            return;
        };
        let Some(ai) = self.ai_players.iter_mut().find(|ai| ai.id == target) else {
            return;
        };
        if ai.profile_request_id == profile_request_id {
            return;
        }
        ai.profile_request_id = profile_request_id;
        crate::log_debug!(
            room = %self.room,
            ai_id = target,
            ai_profile_id = %profile_request_id,
            "AI profile selected"
        );
        self.broadcast_lobby();
    }

    /// Host-only: remove a previously-added AI opponent by id. Ignored outside the lobby, from
    /// non-hosts, or for an unknown id.
    pub(super) fn on_remove_ai(&mut self, player_id: u32, target: u32) {
        if self.is_dev_watch() {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if !matches!(self.phase, Phase::Lobby) || self.host_id != Some(player_id) {
            return;
        }
        if self.is_replay_staging_lobby() {
            return;
        }
        let before = self.ai_players.len();
        self.ai_players.retain(|a| a.id != target);
        if self.ai_players.len() != before {
            crate::log_debug!(room = %self.room, ai_id = target, "AI opponent removed");
            self.broadcast_lobby();
        }
    }

    pub(super) fn ai_slot_display_names(&self) -> Vec<String> {
        let mut profile_counts: HashMap<&'static str, usize> = HashMap::new();
        for ai in &self.ai_players {
            let label = rts_ai::live_profile_label(ai.profile_request_id);
            *profile_counts.entry(label).or_default() += 1;
        }

        let mut profile_seen: HashMap<&'static str, usize> = HashMap::new();
        self.ai_players
            .iter()
            .map(|ai| {
                let label = rts_ai::live_profile_label(ai.profile_request_id);
                if profile_counts.get(label).copied().unwrap_or(0) > 1 {
                    let seen = profile_seen.entry(label).or_default();
                    *seen += 1;
                    format!("{label} {seen}")
                } else {
                    label.to_string()
                }
            })
            .collect()
    }

    pub(super) fn on_select_map(&mut self, player_id: u32, map: String) {
        if self.is_dev_watch()
            || self.match_countdown_deadline.is_some()
            || !matches!(self.phase, Phase::Lobby)
            || self.host_id != Some(player_id)
            || self.is_replay_staging_lobby()
        {
            return;
        }
        let Some((map, cap)) = map_catalog::selectable_map(&map) else {
            crate::log_debug!(room = %self.room, map = %map, "ignoring unknown map selection");
            return;
        };
        if self.selected_map == map {
            return;
        }
        self.selected_map = map;
        self.trim_active_slots_to_cap(cap);
        crate::log_debug!(room = %self.room, map = %self.selected_map, "map selected");
        self.broadcast_lobby();
    }

    pub(super) fn on_set_spectator(&mut self, player_id: u32, target: u32, spectator: bool) {
        if self.is_dev_watch()
            || self.match_countdown_deadline.is_some()
            || !matches!(self.phase, Phase::Lobby)
            || self.is_replay_staging_lobby()
        {
            return;
        }
        if target != player_id && self.host_id != Some(player_id) {
            crate::log_debug!(
                room = %self.room,
                player_id,
                target,
                "ignoring non-host spectator assignment"
            );
            return;
        }
        let current = self.players.get(&target).map(|p| p.spectator);
        if current == Some(spectator) || current.is_none() {
            return;
        }
        if spectator {
            self.demote_human_to_spectator(target);
        } else {
            if self.total_player_count() >= map_catalog::active_slot_cap(&self.selected_map) {
                crate::log_debug!(room = %self.room, player_id, target, "ignoring player role switch; room full");
                return;
            }
            let color = self.next_human_color();
            if let Some(player) = self.players.get_mut(&target) {
                player.spectator = false;
                player.ready = false;
                player.color = color;
            }
            self.assign_missing_team_for(target);
            self.assign_missing_faction_for(target);
        }
        self.broadcast_lobby();
    }

    fn demote_human_to_spectator(&mut self, target: u32) {
        if let Some(player) = self.players.get_mut(&target) {
            player.spectator = true;
            player.ready = false;
            player.color = "#6f8fa8".to_string();
        }
        self.human_team_assignments.remove(&target);
        self.human_faction_assignments.remove(&target);
    }

    fn trim_active_slots_to_cap(&mut self, cap: usize) {
        let mut active_humans: Vec<u32> = self.active_human_ids().collect();
        while active_humans.len() + self.ai_players.len() > cap {
            if self.ai_players.pop().is_some() {
                continue;
            }
            let Some(index) = active_humans
                .iter()
                .rposition(|id| Some(*id) != self.host_id)
            else {
                return;
            };
            self.demote_human_to_spectator(active_humans.remove(index));
        }
    }

    pub(super) fn total_player_count(&self) -> usize {
        self.active_human_count() + self.ai_players.len()
    }

    pub(super) fn active_human_count(&self) -> usize {
        self.participants().active_human_count()
    }

    pub(super) fn active_human_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.participants().active_human_ids().into_iter()
    }

    fn active_seat_ids(&self) -> Vec<u32> {
        self.participants()
            .active_seat_ids(self.ai_players.iter().map(|ai| ai.id))
    }

    pub(super) fn team_id_for_active_seat(&self, id: u32) -> TeamId {
        if let Some(team_id) = self.human_team_assignments.get(&id) {
            return *team_id;
        }
        if let Some(ai) = self.ai_players.iter().find(|ai| ai.id == id) {
            return ai.team_id;
        }
        id
    }

    pub(super) fn human_faction_for(&self, id: u32) -> String {
        self.human_faction_assignments
            .get(&id)
            .cloned()
            .unwrap_or_else(|| default_faction_id_for(FactionRequestContext::NormalLobby))
    }

    fn team_counts_except(&self, except_id: Option<u32>) -> HashMap<TeamId, usize> {
        let mut counts = HashMap::new();
        for id in self.active_seat_ids() {
            if Some(id) == except_id {
                continue;
            }
            let team_id = self.team_id_for_active_seat(id);
            *counts.entry(team_id).or_insert(0) += 1;
        }
        counts
    }

    fn team_move_allowed(&self, target: u32, team_id: TeamId) -> bool {
        if team_id == 0 {
            return false;
        }
        let mut counts = self.team_counts_except(Some(target));
        let new_count = counts
            .entry(team_id)
            .and_modify(|count| *count += 1)
            .or_insert(1);
        team_id <= MAX_LOBBY_TEAMS && *new_count <= MAX_PLAYERS
    }

    pub(super) fn next_default_team_for_new_seat(&self, new_id: u32) -> TeamId {
        let counts = self.team_counts_except(Some(new_id));
        if let Some(next_after_occupied) = counts
            .keys()
            .copied()
            .filter(|team_id| (1..=MAX_LOBBY_TEAMS).contains(team_id))
            .max()
            .and_then(|team_id| team_id.checked_add(1))
            .filter(|team_id| *team_id <= MAX_LOBBY_TEAMS && !counts.contains_key(team_id))
        {
            return next_after_occupied;
        }
        for team_id in 1..=MAX_LOBBY_TEAMS {
            if counts.get(&team_id).copied().unwrap_or(0) == 0 {
                return team_id;
            }
        }
        new_id.clamp(1, MAX_LOBBY_TEAMS)
    }

    pub(super) fn assign_missing_team_for(&mut self, player_id: u32) {
        if self.human_team_assignments.contains_key(&player_id) {
            return;
        }
        let team_id = self.next_default_team_for_new_seat(player_id);
        self.human_team_assignments.insert(player_id, team_id);
    }

    pub(super) fn assign_missing_faction_for(&mut self, player_id: u32) {
        if self.human_faction_assignments.contains_key(&player_id) {
            return;
        }
        self.human_faction_assignments.insert(
            player_id,
            default_faction_id_for(FactionRequestContext::NormalLobby),
        );
    }

    fn team_composition_valid(&self) -> bool {
        let active_ids = self.active_seat_ids();
        if active_ids.is_empty()
            || active_ids.len() > map_catalog::active_slot_cap(&self.selected_map)
        {
            return false;
        }
        for id in active_ids {
            let team_id = self.team_id_for_active_seat(id);
            if team_id == 0 || team_id > MAX_LOBBY_TEAMS {
                return false;
            }
        }
        true
    }

    pub(super) fn spectator_visible_player_ids(&self) -> Vec<u32> {
        self.participants()
            .spectator_visible_player_ids(self.ai_players.iter().map(|ai| ai.id))
    }

    pub(super) fn live_seat_id_for_connection(&self, connection_id: u32) -> Option<u32> {
        self.participants()
            .live_seat_id_for_connection(connection_id)
    }

    pub(super) fn live_connection_is_player(&self, connection_id: u32) -> bool {
        self.participants().live_connection_is_player(connection_id)
    }

    pub(super) fn command_issuer_for_connection(
        &self,
        connection_id: u32,
    ) -> Option<CommandIssuer> {
        self.participants()
            .command_issuer_for_connection(connection_id, &self.outcome_sent)
    }

    pub(super) fn reassign_host_if_needed(&mut self) {
        self.host_id = self.participants().host_with_fallback(self.host_id);
    }

    fn participants(&self) -> Participants<'_> {
        Participants::new(
            &self.order,
            &self.players,
            &self.branch_live_seat_by_connection,
        )
    }

    /// Pick the first palette color not currently held by a human player. Join order alone is
    /// not enough because earlier seats can leave while later players keep their colors.
    fn next_human_color(&self) -> String {
        PLAYER_PALETTE
            .iter()
            .copied()
            .find(|color| !self.players.values().any(|p| p.color == *color))
            .unwrap_or(PLAYER_PALETTE[self.active_human_count() % PLAYER_PALETTE.len()])
            .to_string()
    }

    /// Color for the `seat`-th AI opponent. AIs use the same accessible order as humans while
    /// skipping colors already held by active humans, so mixed human/AI rooms stay distinct
    /// without bunching every AI into the palette tail.
    pub(super) fn ai_color(&self, seat: usize) -> String {
        PLAYER_PALETTE
            .iter()
            .copied()
            .filter(|color| {
                !self
                    .players
                    .values()
                    .any(|player| !player.spectator && player.color == *color)
            })
            .nth(seat)
            .unwrap_or(PLAYER_PALETTE[(self.active_human_count() + seat) % PLAYER_PALETTE.len()])
            .to_string()
    }

    /// A match may start with at least one active participant and every active human ready.
    /// Spectators can host and watch from the lobby, but they do not block readiness.
    fn can_start(&self) -> bool {
        self.match_countdown_deadline.is_none() && self.can_start_now()
    }

    pub(super) fn can_start_now(&self) -> bool {
        if self.is_replay_staging_lobby() {
            return !self.drain.is_draining() && self.host_id.is_some() && !self.players.is_empty();
        }
        if let Phase::BranchStaging(staging) = &self.phase {
            return !self.new_live_session_blocked_by_drain() && staging.can_start();
        }
        !self.new_live_session_blocked_by_drain()
            && self.total_player_count() > 0
            && self.team_composition_valid()
            && self
                .players
                .values()
                .filter(|p| !p.spectator)
                .all(|p| p.ready)
    }

    fn should_skip_match_countdown(&self) -> bool {
        !self.session_policy().countdown_eligible || self.active_human_count() <= 1
    }

    /// Build and broadcast the current `lobby` message to everyone in the room.
    pub(super) fn broadcast_lobby(&mut self) {
        let host_id = self.host_id.unwrap_or(0);
        let kind = self.lobby_kind();
        // Humans first (in join order), then AI opponents. AIs always read as ready.
        let mut players: Vec<LobbyPlayer> = self
            .order
            .iter()
            .filter_map(|id| {
                self.players.get(id).map(|p| LobbyPlayer {
                    id: *id,
                    team_id: if p.spectator {
                        0
                    } else {
                        self.team_id_for_active_seat(*id)
                    },
                    faction_id: if p.spectator {
                        default_faction_id_for(FactionRequestContext::NormalLobby)
                    } else {
                        self.human_faction_for(*id)
                    },
                    name: p.name.clone(),
                    ready: p.ready,
                    color: p.color.clone(),
                    is_ai: false,
                    ai_profile_id: None,
                    is_spectator: p.spectator,
                })
            })
            .collect();
        let ai_names = self.ai_slot_display_names();
        for ((seat, ai), name) in self.ai_players.iter().enumerate().zip(ai_names) {
            players.push(LobbyPlayer {
                id: ai.id,
                team_id: ai.team_id,
                faction_id: ai.faction_id.clone(),
                name,
                ready: true,
                color: self.ai_color(seat),
                is_ai: true,
                ai_profile_id: Some(ai.profile_request_id.to_string()),
                is_spectator: false,
            });
        }
        let msg = ServerMessage::Lobby {
            room: self.room.clone(),
            kind,
            host_id,
            players,
            can_start: self.can_start(),
            team_preset: "custom".to_string(),
            map: self.lobby_map_name(),
            maps: if kind == LobbyKind::Replay {
                Vec::new()
            } else {
                Map::list_available()
            },
        };
        self.broadcast(&msg);
    }
}
