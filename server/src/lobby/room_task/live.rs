use std::collections::HashSet;
use std::time::Instant as StdInstant;

use tokio::time::Instant as TokioInstant;

use super::super::connection::{send_or_log, CommandLifecycleSample, ConnectionSink};
use super::super::dev_replay::match_seed;
use super::super::launch::{LaunchPrediction, LaunchRecipient, StartPayloadBuilder};
use super::super::live_tick::{LiveTickDriver, LiveTickResult};
use super::super::projection::RecipientRole;
use super::super::session_policy::{RoomTimeSource, SessionPhase};
use super::super::{normalize_start_team_id, CommandLifecycleTiming, PlayerInit};
use super::helpers::{late_spectator_notice_name, live_ai_controllers, LIVE_PAUSE_LIMIT};
use super::types::{PendingClientCommandAck, Phase, RoomPlayer};
use super::RoomTask;
use crate::protocol::{
    Event, LivePauseState, NoticeSeverity, PlayerScore, RoomTimeState, ServerMessage, TeamId,
};
use crate::structured_log::{self, MatchStartedLog};
use rts_sim::game::command::SimCommand;
use rts_sim::game::map::Map;
use rts_sim::game::Game;

impl RoomTask {
    pub(super) fn on_join_live_spectator(
        &mut self,
        player_id: u32,
        name: String,
        spectator: bool,
        msg_tx: ConnectionSink,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        if !spectator {
            send_or_log(
                &self.room,
                player_id,
                &msg_tx,
                ServerMessage::Error {
                    msg: "Match already in progress in this room — join as a spectator or try another room."
                        .to_string(),
                },
            );
            crate::log_debug!(room = %self.room, player_id, "rejecting active join; match in progress");
            let _ = ack.send(false);
            return;
        }

        let mut payload = match &self.phase {
            Phase::InGame(game) => game.start_payload(),
            _ => {
                send_or_log(
                    &self.room,
                    player_id,
                    &msg_tx,
                    ServerMessage::Error {
                        msg: "Match already in progress in this room — try another room."
                            .to_string(),
                    },
                );
                crate::log_debug!(room = %self.room, player_id, "rejecting spectator join; no live match payload");
                let _ = ack.send(false);
                return;
            }
        };
        payload.match_run_id = self.match_run_id.clone();

        let notice_recipients = self.late_spectator_notice_recipient_ids();
        let notice_name = late_spectator_notice_name(&name);

        self.insert_human_spectator(player_id, name, msg_tx);
        crate::log_debug!(room = %self.room, player_id, "joined live match as spectator");
        let _ = ack.send(true);
        self.send_current_shutdown_warning_to(player_id);
        self.enqueue_late_spectator_join_notice(notice_recipients, notice_name);

        let projection_policy = self.projection_policy_for_phase(SessionPhase::LiveMatch);
        let start_policy = self.live_session_policy();
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        let builder = StartPayloadBuilder::simulation(start_policy, &payload);
        super::super::launch::send_start_payloads(
            &self.room,
            &builder,
            [LaunchRecipient {
                connection_id: player_id,
                payload_player_id: player_id,
                prediction: LaunchPrediction::Disabled,
                role: RecipientRole::Spectator,
                diagnostics: projection_policy
                    .diagnostic_capabilities_for(RecipientRole::Spectator),
                clear_pending_snapshot: true,
                lab: None,
                msg_tx: player.msg_tx.clone(),
            }],
        );
        if self.live_pause_controls_available() {
            self.send_live_pause_state_to(player_id);
        }
        self.send_live_ai_room_time_state_to(player_id);
    }

    fn insert_human_spectator(&mut self, player_id: u32, name: String, msg_tx: ConnectionSink) {
        self.order.push(player_id);
        self.players.insert(
            player_id,
            RoomPlayer {
                name,
                color: "#6f8fa8".to_string(),
                ready: true,
                spectator: true,
                msg_tx,
                head_of_line_count: 0,
                last_received_client_seq: 0,
                last_sim_consumed_client_seq: 0,
                last_sim_consumed_client_tick: None,
            },
        );
        self.reassign_host_if_needed();
    }

    fn late_spectator_notice_recipient_ids(&self) -> Vec<u32> {
        self.order
            .iter()
            .copied()
            .filter(|id| self.players.contains_key(id))
            .collect()
    }

    fn enqueue_late_spectator_join_notice(&mut self, recipients: Vec<u32>, spectator_name: String) {
        if recipients.is_empty() {
            return;
        }
        let notice = Event::Notice {
            msg: format!("{spectator_name} has joined the match as a spectator"),
            severity: NoticeSeverity::Info,
            x: None,
            y: None,
        };
        for id in recipients {
            self.pending_recipient_notices
                .entry(id)
                .or_default()
                .push(notice.clone());
        }
    }

    pub(super) fn on_command_with_lifecycle(
        &mut self,
        player_id: u32,
        client_seq: u32,
        cmd: SimCommand,
        lifecycle: CommandLifecycleTiming,
    ) {
        let room_handle_started_at = StdInstant::now();
        if self.is_dev_watch() {
            return;
        }
        if client_seq == 0 {
            crate::log_debug!(room = %self.room, player_id, "ignoring command with reserved clientSeq 0");
            self.send_command_receipt(player_id, client_seq, 0, false, Some("invalidSeq"));
            self.record_command_lifecycle_sample(
                player_id,
                client_seq,
                lifecycle,
                room_handle_started_at,
                false,
            );
            return;
        }
        let issuer = self.command_issuer_for_connection(player_id);
        // Commands are ignored unless in-game and the sender is actually in this room. The
        // simulation itself validates ownership/affordability when it applies the command.
        let receipt = if let Phase::InGame(game) = &mut self.phase {
            let server_tick = game.current_tick();
            if let Some(issuer) = issuer {
                if let Some(player) = self.players.get_mut(&player_id) {
                    if client_seq <= player.last_received_client_seq {
                        crate::log_debug!(
                            room = %self.room,
                            player_id,
                            client_seq,
                            last_received = player.last_received_client_seq,
                            "ignoring stale or wrapped command sequence"
                        );
                        (server_tick, false, Some("staleSeq"))
                    } else {
                        player.last_received_client_seq = client_seq;
                        let accepted_at = StdInstant::now();
                        game.enqueue(issuer.seat_id, cmd);
                        self.pending_client_command_acks
                            .push(PendingClientCommandAck {
                                connection_id: issuer.connection_id,
                                client_seq,
                                received_unix_ms: lifecycle.received_unix_ms,
                                family: lifecycle.family.as_str(),
                                accepted_at,
                            });
                        (server_tick, true, None)
                    }
                } else {
                    (server_tick, false, Some("notJoined"))
                }
            } else {
                (server_tick, false, Some("notPlayer"))
            }
        } else {
            (0, false, Some("notInGame"))
        };
        let (server_tick, accepted, reason) = receipt;
        self.send_command_receipt(player_id, client_seq, server_tick, accepted, reason);
        self.record_command_lifecycle_sample(
            player_id,
            client_seq,
            lifecycle,
            room_handle_started_at,
            accepted,
        );
    }

    fn record_command_lifecycle_sample(
        &self,
        player_id: u32,
        client_seq: u32,
        lifecycle: CommandLifecycleTiming,
        room_handle_started_at: StdInstant,
        accepted: bool,
    ) {
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        let sample = CommandLifecycleSample::from_timing(
            client_seq,
            lifecycle,
            room_handle_started_at,
            accepted,
        );
        player.msg_tx.record_command_lifecycle(sample);
    }

    fn send_command_receipt(
        &self,
        player_id: u32,
        client_seq: u32,
        server_tick: u32,
        accepted: bool,
        reason: Option<&str>,
    ) {
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        send_or_log(
            &self.room,
            player_id,
            &player.msg_tx,
            ServerMessage::CommandReceipt {
                client_seq,
                server_tick,
                accepted,
                reason: reason.map(str::to_string),
            },
        );
    }

    fn live_pause_controls_available(&self) -> bool {
        self.session_policy()
            .start_capabilities(true)
            .match_controls
            .pause
    }

    fn live_pause_actor_for_connection(&self, connection_id: u32) -> Option<u32> {
        if !matches!(self.phase, Phase::InGame(_)) || !self.live_pause_controls_available() {
            return None;
        }
        if self.outcome_sent.contains(&connection_id) {
            return None;
        }
        let player = self.players.get(&connection_id)?;
        if player.spectator {
            return Some(connection_id);
        }
        self.live_connection_is_player(connection_id).then(|| {
            self.live_seat_id_for_connection(connection_id)
                .unwrap_or(connection_id)
        })
    }

    fn live_pause_state_for(&self, connection_id: u32) -> LivePauseState {
        let actor_id = self.live_pause_actor_for_connection(connection_id);
        let pauses_remaining = actor_id.map(|actor_id| {
            LIVE_PAUSE_LIMIT
                .saturating_sub(self.live_pause_counts.get(&actor_id).copied().unwrap_or(0))
        });
        let can_pause = pauses_remaining
            .map(|remaining| !self.live_paused && remaining > 0)
            .unwrap_or(false);
        LivePauseState {
            paused: self.live_paused,
            paused_by: self.live_paused_by,
            pauses_remaining,
            pause_limit: LIVE_PAUSE_LIMIT,
            can_pause,
            can_unpause: self.live_paused && actor_id.is_some(),
        }
    }

    fn send_live_pause_state_to(&self, connection_id: u32) {
        let Some(player) = self.players.get(&connection_id) else {
            return;
        };
        send_or_log(
            &self.room,
            connection_id,
            &player.msg_tx,
            ServerMessage::LivePauseState(self.live_pause_state_for(connection_id)),
        );
    }

    pub(super) fn broadcast_live_pause_state(&self) {
        if !matches!(self.phase, Phase::InGame(_)) || !self.live_pause_controls_available() {
            return;
        }
        for &connection_id in &self.order {
            self.send_live_pause_state_to(connection_id);
        }
    }

    pub(super) fn on_pause_game(&mut self, player_id: u32) {
        let Some(actor_id) = self.live_pause_actor_for_connection(player_id) else {
            self.send_live_pause_state_to(player_id);
            return;
        };
        if self.live_paused {
            self.send_live_pause_state_to(player_id);
            return;
        }
        let used = self.live_pause_counts.get(&actor_id).copied().unwrap_or(0);
        if used >= LIVE_PAUSE_LIMIT {
            self.send_live_pause_state_to(player_id);
            return;
        }
        self.live_pause_counts
            .insert(actor_id, used.saturating_add(1));
        self.live_paused = true;
        self.live_paused_by = Some(actor_id);
        crate::log_info!(room = %self.room, player_id, pause_actor_id = actor_id, "live match paused");
        self.broadcast_live_pause_state();
    }

    pub(super) fn on_unpause_game(&mut self, player_id: u32) {
        if self.live_pause_actor_for_connection(player_id).is_none() {
            self.send_live_pause_state_to(player_id);
            return;
        }
        if !self.live_paused {
            self.send_live_pause_state_to(player_id);
            return;
        }
        self.live_paused = false;
        self.live_paused_by = None;
        crate::log_info!(room = %self.room, player_id, "live match unpaused");
        self.broadcast_live_pause_state();
    }

    pub(super) fn on_give_up(&mut self, player_id: u32) {
        if self.is_dev_watch() {
            return;
        }
        if !self.live_connection_is_player(player_id) || self.outcome_sent.contains(&player_id) {
            return;
        }
        let seat_id = self
            .live_seat_id_for_connection(player_id)
            .unwrap_or(player_id);

        let mut game = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::Lobby => {
                self.phase = Phase::Lobby;
                return;
            }
            Phase::InGame(game) => game,
            Phase::ReplayViewer(session) => {
                self.phase = Phase::ReplayViewer(session);
                return;
            }
            Phase::BranchStaging(staging) => {
                self.phase = Phase::BranchStaging(staging);
                return;
            }
        };

        crate::log_debug!(room = %self.room, player_id, "player gave up");
        game.eliminate(seat_id);
        let alive = game.alive_players();
        let alive_teams = game.alive_team_ids();
        let scores = game.scores();

        if self.match_player_count >= 2 && alive_teams.len() <= 1 {
            let winner_id = alive_teams
                .first()
                .and_then(|team_id| game.first_alive_player_on_team(*team_id));
            self.end_match(winner_id, scores, Some(&game));
            return;
        }

        if self.match_player_count >= 2 {
            self.send_new_defeats(&game, &alive);
        }

        if self.match_player_count < 2 {
            self.end_match(None, scores, Some(&game));
        } else {
            self.phase = Phase::InGame(game);
        }
    }

    /// Transition from `Lobby` to `InGame`: create the simulation and send each player their
    /// own `start` payload. Only called from `on_start_request` once preconditions hold.
    pub(super) fn start_match(&mut self) {
        // Defense in depth: internal observer tooling may stage experimental profiles, but no
        // such profile may cross the authoritative launch seam into a human match.
        self.replace_internal_ai_profiles_for_player_match();
        self.prepare_live_match_launch();
        let mut inits: Vec<PlayerInit> = self
            .active_human_ids()
            .filter_map(|id| {
                self.players.get(&id).map(|p| PlayerInit {
                    id,
                    team_id: self.team_id_for_active_seat(id),
                    faction_id: self.human_faction_for(id),
                    name: p.name.clone(),
                    color: p.color.clone(),
                    is_ai: false,
                })
            })
            .collect();
        // Seat AI opponents after the humans so colors match the lobby list and authored start
        // sites are assigned in the same order the lobby displays players.
        let ai_names = self.ai_slot_display_names();
        for ((seat, ai), name) in self.ai_players.iter().enumerate().zip(ai_names) {
            inits.push(PlayerInit {
                id: ai.id,
                team_id: ai.team_id,
                faction_id: ai.faction_id.clone(),
                name,
                color: self.ai_color(seat),
                is_ai: true,
            });
        }

        let seed = match_seed();

        // Load the selected map from disk. On failure, send an error to the host and abort.
        let map_metadata = match Map::metadata_for_name(&self.selected_map) {
            Ok(metadata) => metadata,
            Err(err) => {
                let msg = format!("Cannot load map \"{}\": {err}", self.selected_map);
                crate::log_warn!(room = %self.room, error = %err, "map metadata load failed at start");
                if let Some(host_id) = self.host_id {
                    if let Some(player) = self.players.get(&host_id) {
                        send_or_log(
                            &self.room,
                            host_id,
                            &player.msg_tx,
                            ServerMessage::Error { msg },
                        );
                    }
                }
                return;
            }
        };
        let start_players: Vec<_> = inits
            .iter()
            .map(|player| {
                (
                    player.id,
                    normalize_start_team_id(player.id, player.team_id),
                )
            })
            .collect();
        let map = match Map::load_for_players(&self.selected_map, &start_players, seed) {
            Ok(m) => m,
            Err(err) => {
                let msg = format!("Cannot load map \"{}\": {err}", self.selected_map);
                crate::log_warn!(room = %self.room, error = %err, "map load failed at start");
                if let Some(host_id) = self.host_id {
                    if let Some(player) = self.players.get(&host_id) {
                        send_or_log(
                            &self.room,
                            host_id,
                            &player.msg_tx,
                            ServerMessage::Error { msg },
                        );
                    }
                }
                return;
            }
        };

        let game =
            Game::new_with_random_ai_profiles_and_map_metadata(&inits, seed, map, map_metadata);
        self.capture_replay_start_for(&game);
        let match_player_count = inits.len();
        let match_human_count = inits.iter().filter(|p| !p.is_ai).count();
        let match_map_name = self.selected_map.clone();
        let match_participants = inits.iter().map(|p| p.name.clone()).collect();
        self.record_live_match_started(
            match_player_count,
            match_human_count,
            match_map_name,
            match_participants,
        );
        let mut payload = game.start_payload();
        payload.match_run_id = self.match_run_id.clone();
        self.ai_controllers = live_ai_controllers(&inits, &self.ai_players, seed);

        let projection_policy = self.projection_policy_for_phase(SessionPhase::LiveMatch);
        let start_policy = self.live_session_policy();
        let recipients: Vec<_> = self
            .order
            .iter()
            .filter_map(|&id| {
                let role = self.players.get(&id).map(|player| {
                    if player.spectator {
                        RecipientRole::Spectator
                    } else {
                        RecipientRole::ActivePlayer
                    }
                })?;
                self.players.get(&id).map(|player| LaunchRecipient {
                    connection_id: id,
                    payload_player_id: id,
                    role,
                    prediction: if player.spectator {
                        LaunchPrediction::Disabled
                    } else {
                        LaunchPrediction::Enabled
                    },
                    diagnostics: projection_policy.diagnostic_capabilities_for(role),
                    clear_pending_snapshot: false,
                    lab: None,
                    msg_tx: player.msg_tx.clone(),
                })
            })
            .collect();
        let builder = StartPayloadBuilder::simulation(start_policy, &payload);
        super::super::launch::send_start_payloads(&self.room, &builder, recipients);

        structured_log::log_match_started(MatchStartedLog {
            room: &self.room,
            match_run_id: self.match_run_id.as_deref().unwrap_or(""),
            mode: "live",
            map: &self.match_map_name,
            seed,
            players: self.match_player_count,
            humans: self.match_human_count,
            ai: self.ai_players.len(),
            participants: &self.match_participants,
        });
        self.mark_match_started_for_drain();
        self.phase = Phase::InGame(Box::new(game));
        self.broadcast_live_ai_room_time_state();
        self.broadcast_live_pause_state();
    }

    fn live_ai_room_time_state(&self) -> Option<RoomTimeState> {
        if self.session_policy().clock.room_time_source() != Some(RoomTimeSource::LiveGame) {
            return None;
        }
        let Phase::InGame(game) = &self.phase else {
            return None;
        };
        Some(self.room_time_state_for_live_game(game, None))
    }

    pub(super) fn send_live_ai_room_time_state_to(&self, player_id: u32) {
        let Some(state) = self.live_ai_room_time_state() else {
            return;
        };
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        send_or_log(
            &self.room,
            player_id,
            &player.msg_tx,
            ServerMessage::RoomTimeState(state),
        );
    }

    pub(super) fn broadcast_live_ai_room_time_state(&self) {
        let Some(state) = self.live_ai_room_time_state() else {
            return;
        };
        self.broadcast(&ServerMessage::RoomTimeState(state));
    }

    pub(super) fn on_tick_live_game(&mut self, scheduled: TokioInstant) {
        if self.live_paused && self.live_pause_controls_available() {
            return;
        }
        self.apply_lab_scenario_actions();
        // Take ownership of the game for the duration of the tick so we can both mutate it and
        // freely borrow `self` for sending. Restored (or replaced with `Lobby`) before return.
        let projection_policy = self.projection_policy();
        let game = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::Lobby => {
                // Stay in lobby; nothing to simulate.
                self.phase = Phase::Lobby;
                return;
            }
            Phase::InGame(game) => game,
            Phase::ReplayViewer(session) => {
                self.phase = Phase::ReplayViewer(session);
                return;
            }
            Phase::BranchStaging(staging) => {
                self.phase = Phase::BranchStaging(staging);
                return;
            }
        };
        let tick_budget = self.current_tick_interval();
        let match_run_id = self.match_run_id.as_deref();
        let ai_player_count = self.ai_players.len();
        let spectator_visible_players = self.spectator_visible_player_ids();
        let lab_visible_player_ids_by_recipient = self.lab_visible_player_ids_by_recipient(&game);
        let record_lab_timeline = matches!(self.mode, super::types::RoomMode::Lab(_));
        let result = LiveTickDriver {
            room: &self.room,
            scheduled,
            tick_budget,
            match_run_id,
            match_player_count: self.match_player_count,
            ai_player_count,
            players: &mut self.players,
            order: &self.order,
            outcome_sent: &mut self.outcome_sent,
            branch_live_seat_by_connection: &self.branch_live_seat_by_connection,
            ai_controllers: &mut self.ai_controllers,
            pending_client_command_acks: &mut self.pending_client_command_acks,
            pending_recipient_notices: &mut self.pending_recipient_notices,
            slow_tick_count: &mut self.slow_tick_count,
            spectator_visible_players,
            lab_visible_player_ids_by_recipient,
            projection_policy,
            replay_start: self.replay_start.as_ref(),
        }
        .run(game);

        match result {
            LiveTickResult::Continue(game) => {
                let broadcast_lab_timeline_state = if record_lab_timeline {
                    self.lab_timeline
                        .as_mut()
                        .is_some_and(|timeline| timeline.record_keyframe_if_due(&game))
                } else {
                    false
                };
                self.phase = Phase::InGame(game);
                if broadcast_lab_timeline_state {
                    self.broadcast_lab_room_time_state();
                }
            }
            LiveTickResult::EndMatch {
                game,
                winner_id,
                scores,
            } => {
                self.end_match(winner_id, scores, Some(&game));
            }
            LiveTickResult::PanicEnd { scores } => {
                self.end_match(None, scores, None);
            }
        }
    }

    /// Send a one-time score screen to connected players who have been eliminated while a
    /// multi-player match continues.
    fn send_new_defeats(&mut self, game: &Game, alive: &[u32]) {
        let alive: HashSet<u32> = alive.iter().copied().collect();
        let recipients: Vec<u32> = self
            .order
            .iter()
            .copied()
            .filter(|id| {
                self.live_connection_is_player(*id)
                    && self
                        .live_seat_id_for_connection(*id)
                        .map(|seat_id| {
                            !alive.contains(&seat_id) && !game.team_has_alive_player(seat_id)
                        })
                        .unwrap_or(false)
                    && !self.outcome_sent.contains(id)
            })
            .collect();
        if recipients.is_empty() {
            return;
        }
        let scores = game.scores();
        for id in recipients {
            let Some(player) = self.players.get(&id) else {
                continue;
            };
            send_or_log(
                &self.room,
                id,
                &player.msg_tx,
                ServerMessage::GameOver {
                    winner_id: None,
                    winner_team_id: None,
                    you: "lost".to_string(),
                    scores: scores.clone(),
                },
            );
            self.outcome_sent.insert(id);
        }
    }

    pub(super) fn team_id_for_score_seat(
        game: Option<&Game>,
        scores: &[PlayerScore],
        seat_id: u32,
    ) -> Option<TeamId> {
        game.and_then(|game| game.team_of_player(seat_id))
            .or_else(|| {
                scores
                    .iter()
                    .find(|score| score.id == seat_id)
                    .map(|score| score.team_id)
                    .filter(|team_id| *team_id != 0)
            })
    }
}
