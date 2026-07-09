use std::time::Instant as StdInstant;

use tokio::time::Instant as TokioInstant;

use super::super::connection::send_or_log;
use super::super::replay_session::ReplaySession;
use super::super::session_policy::RoomTimeSource;
use super::super::tick_control::ScheduledTickAction;
use super::super::{current_unix_ms, DrainNotice, RoomMode, ShutdownFinalizeResult};
use super::helpers::{match_countdown_duration, server_build_sha, MATCH_COUNTDOWN_WORDS};
use super::match_history::MatchHistoryRecordInput;
use super::types::Phase;
use super::RoomTask;
use crate::protocol::{PlayerScore, ServerMessage};
use crate::structured_log::{self, MatchEndedLog};
use rts_sim::game::replay::ReplayArtifactV1;
use rts_sim::game::Game;

impl RoomTask {
    pub(super) fn new_live_session_blocked_by_drain(&self) -> bool {
        self.drain.is_draining()
            && !self
                .live_session_policy()
                .allows_new_session_while_draining()
    }

    pub(super) fn on_drain_started(&mut self, notice: DrainNotice) {
        self.broadcast_shutdown_warning(notice);
        if matches!(self.phase, Phase::Lobby) {
            self.broadcast_lobby();
        }
    }

    pub(super) fn send_current_shutdown_warning_to(&self, player_id: u32) {
        let Some(notice) = self.drain.notice() else {
            return;
        };
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        send_or_log(
            &self.room,
            player_id,
            &player.msg_tx,
            ServerMessage::ShutdownWarning {
                deadline_unix_ms: notice.deadline_unix_ms,
                seconds_remaining: notice.seconds_remaining,
            },
        );
    }

    fn broadcast_shutdown_warning(&self, notice: DrainNotice) {
        let msg = ServerMessage::ShutdownWarning {
            deadline_unix_ms: notice.deadline_unix_ms,
            seconds_remaining: notice.seconds_remaining,
        };
        self.broadcast(&msg);
    }

    /// One simulation step. No-op in the `Lobby` phase (the ticker keeps running so a room is
    /// always live and ready to start).
    pub(super) fn on_tick(&mut self, scheduled: TokioInstant) {
        match self.tick_control().scheduled_action() {
            ScheduledTickAction::Noop => {}
            ScheduledTickAction::Countdown => {
                self.finish_match_countdown_if_due();
            }
            ScheduledTickAction::RoomControlled(RoomTimeSource::ReplayPlayback) => {
                self.on_tick_replay_viewer(scheduled);
            }
            ScheduledTickAction::RoomControlled(RoomTimeSource::DevScenario) => {
                self.on_tick_dev_watch(scheduled);
            }
            ScheduledTickAction::RoomControlled(RoomTimeSource::Lab)
            | ScheduledTickAction::RoomControlled(RoomTimeSource::LiveGame)
            | ScheduledTickAction::LiveMatch => {
                self.on_tick_live_game(scheduled);
            }
        }
    }

    pub(super) fn start_match_countdown(&mut self) {
        let duration = match_countdown_duration();
        self.match_countdown_deadline = Some(TokioInstant::now() + duration);
        if matches!(self.phase, Phase::BranchStaging(_)) {
            self.broadcast_branch_staging();
        } else {
            self.broadcast_lobby();
        }
        let msg = ServerMessage::MatchCountdown {
            duration_ms: duration.as_millis() as u32,
            words: MATCH_COUNTDOWN_WORDS
                .iter()
                .map(|word| (*word).to_string())
                .collect(),
        };
        self.broadcast(&msg);
        crate::log_info!(room = %self.room, "match countdown started");
    }

    pub(super) fn finish_match_countdown_if_due(&mut self) -> bool {
        let Some(deadline) = self.match_countdown_deadline else {
            return false;
        };
        if TokioInstant::now() < deadline {
            return true;
        }
        self.match_countdown_deadline = None;
        if self.can_start_now() {
            if matches!(self.phase, Phase::BranchStaging(_)) {
                self.start_branch_live();
            } else {
                self.start_match();
            }
        } else {
            crate::log_debug!(room = %self.room, "match countdown aborted; start preconditions changed");
            if matches!(self.phase, Phase::BranchStaging(_)) {
                self.broadcast_branch_staging();
            } else {
                self.broadcast_lobby();
            }
        }
        true
    }

    /// Resolve a finished match: tell everyone who won and start post-match replay playback.
    pub(super) fn end_match(
        &mut self,
        winner_id: Option<u32>,
        scores: Vec<PlayerScore>,
        game: Option<&Game>,
    ) {
        let winner_team_id =
            winner_id.and_then(|id| Self::team_id_for_score_seat(game, &scores, id));
        let ended_at = chrono::Utc::now();
        let duration_ms = self.match_duration_ms_for(ended_at);
        let duration_ticks = game.map(Game::tick_count);
        let max_head_of_line_count = self
            .players
            .values()
            .map(|player| player.head_of_line_count)
            .max()
            .unwrap_or(0);
        let replay_artifact = game
            .filter(|_| self.should_capture_post_match_replay())
            .and_then(|game| self.finalize_replay_artifact(game, winner_id, scores.clone()));
        let winner_name =
            winner_id.and_then(|wid| scores.iter().find(|s| s.id == wid).map(|s| s.name.clone()));
        let outcome = crate::db::MatchOutcome::from_winner_name(winner_name.as_deref());
        let will_record_history = self.will_record_match_history();
        structured_log::log_match_ended(MatchEndedLog {
            room: &self.room,
            match_run_id: self.match_run_id.as_deref(),
            map: &self.match_map_name,
            participants: &self.match_participants,
            winner_id,
            winner_team_id,
            duration_ms,
            duration_ticks,
            slow_tick_count: self.slow_tick_count,
            max_head_of_line_count,
            score_count: scores.len(),
            replay_captured: replay_artifact.is_some(),
            will_record_history,
            outcome: outcome.as_str(),
        });

        // Persist replay-backed history for deploy-recorded matches. The Recent Matches endpoint
        // filters debug, solo, and AI-only rows; persistence keeps their replay artifacts
        // available for follow-up diagnostics without exposing them on the lobby front page.
        if let (true, Some(started_at), Some(duration_ms)) =
            (will_record_history, self.match_started_at, duration_ms)
        {
            let rec = self.build_match_history_record(MatchHistoryRecordInput {
                started_at,
                ended_at,
                duration_ms: duration_ms as i32,
                scores: &scores,
                replay_artifact: replay_artifact.as_ref(),
                outcome,
                winner_name,
            });
            self.queue_match_history_write(rec);
        }
        self.clear_finished_match_identity();

        let recipients: Vec<u32> = self.order.clone();
        for id in &recipients {
            if self.outcome_sent.contains(id) {
                continue;
            }
            let Some(player) = self.players.get(id) else {
                continue;
            };
            let you = if player.spectator {
                "draw"
            } else {
                let seat_id = self.live_seat_id_for_connection(*id).unwrap_or(*id);
                let seat_team_id = Self::team_id_for_score_seat(game, &scores, seat_id);
                match (winner_team_id, winner_id) {
                    (Some(winner_team_id), _) if seat_team_id == Some(winner_team_id) => "won",
                    (Some(_), _) => "lost",
                    (None, Some(winner_id)) if winner_id == seat_id => "won",
                    (None, Some(_)) => "lost",
                    (None, None) => "draw",
                }
            }
            .to_string();
            send_or_log(
                &self.room,
                *id,
                &player.msg_tx,
                ServerMessage::GameOver {
                    winner_id,
                    winner_team_id,
                    you,
                    scores: scores.clone(),
                },
            );
            self.outcome_sent.insert(*id);
        }

        self.mark_match_finished_for_drain();
        if let Some(artifact) = replay_artifact {
            match ReplaySession::new(artifact) {
                Ok(session) => {
                    self.transition_to_replay_viewer(session);
                    return;
                }
                Err(err) => {
                    crate::log_warn!(room = %self.room, error = %err, "post-match replay setup failed");
                    self.broadcast(&ServerMessage::Error {
                        msg: "Post-match replay could not be started.".to_string(),
                    });
                }
            }
        }
        self.return_to_lobby();
    }

    pub(super) fn finalize_for_shutdown(&mut self) -> ShutdownFinalizeResult {
        let game = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::InGame(game) => game,
            Phase::Lobby => {
                self.phase = Phase::Lobby;
                return ShutdownFinalizeResult::default();
            }
            Phase::ReplayViewer(session) => {
                self.phase = Phase::ReplayViewer(session);
                return ShutdownFinalizeResult::default();
            }
            Phase::BranchStaging(staging) => {
                self.phase = Phase::BranchStaging(staging);
                return ShutdownFinalizeResult::default();
            }
        };

        let ended_at = chrono::Utc::now();
        let duration_ms = self.match_duration_ms_for(ended_at);
        let duration_ticks = Some(game.tick_count());
        let scores = game.scores();
        let max_head_of_line_count = self
            .players
            .values()
            .map(|player| player.head_of_line_count)
            .max()
            .unwrap_or(0);
        let match_history_allowed = self.should_persist_match_history();
        let will_record_history = self.match_history_writer.is_some()
            && self.match_started_at.is_some()
            && match_history_allowed;
        let replay_artifact = (will_record_history
            && self.should_attach_match_history_replay_artifact())
        .then(|| self.finalize_replay_artifact(&game, None, scores.clone()))
        .flatten();
        structured_log::log_match_ended(MatchEndedLog {
            room: &self.room,
            match_run_id: self.match_run_id.as_deref(),
            map: &self.match_map_name,
            participants: &self.match_participants,
            winner_id: None,
            winner_team_id: None,
            duration_ms,
            duration_ticks,
            slow_tick_count: self.slow_tick_count,
            max_head_of_line_count,
            score_count: scores.len(),
            replay_captured: replay_artifact.is_some(),
            will_record_history,
            outcome: crate::db::MatchOutcome::Aborted.as_str(),
        });

        let record_queued = if let (true, Some(started_at), Some(duration_ms)) =
            (will_record_history, self.match_started_at, duration_ms)
        {
            let rec = self.build_match_history_record(MatchHistoryRecordInput {
                started_at,
                ended_at,
                duration_ms: duration_ms as i32,
                scores: &scores,
                replay_artifact: replay_artifact.as_ref(),
                outcome: crate::db::MatchOutcome::Aborted,
                winner_name: None,
            });
            self.queue_match_history_write(rec)
        } else {
            false
        };

        crate::log_info!(
            room = %self.room,
            match_run_id = self.match_run_id.as_deref().unwrap_or(""),
            map = %self.match_map_name,
            match_history_allowed,
            record_queued,
            replay_captured = replay_artifact.is_some(),
            "shutdown finalized active match as aborted"
        );

        self.clear_finished_match_identity();
        self.reset_after_live_match_for_room_phase();
        self.mark_match_finished_for_drain();

        ShutdownFinalizeResult {
            had_active_match: true,
            finalized_match: true,
            match_history_allowed,
            record_queued,
            replay_captured: replay_artifact.is_some(),
        }
    }

    pub(super) fn return_to_lobby(&mut self) {
        // Reset for the next match: drop the game/replay, clear ready flags, and re-advertise
        // the lobby. AI slots and map selection persist for rematches.
        self.phase = Phase::Lobby;
        self.reset_after_live_match_for_room_phase();
        self.broadcast_lobby();
    }

    pub(super) fn prepare_live_match_launch(&mut self) {
        self.match_countdown_deadline = None;
        self.reset_match_net_status();
        self.reset_live_pause_state();
        self.reset_room_time_state();
        self.replay_start = None;
    }

    pub(super) fn capture_replay_start_for(&mut self, game: &Game) {
        match rts_sim::game::replay::ReplayStartComposition::capture(game, server_build_sha()) {
            Ok(start) => self.replay_start = Some(start),
            Err(err) => {
                self.replay_start = None;
                crate::log_warn!(
                    room = %self.room,
                    error = %err,
                    "failed to capture launch-time replay start"
                );
            }
        }
    }

    pub(super) fn finalize_replay_artifact(
        &self,
        game: &Game,
        winner_id: Option<u32>,
        scores: Vec<PlayerScore>,
    ) -> Option<ReplayArtifactV1> {
        let Some(start) = &self.replay_start else {
            crate::log_warn!(
                room = %self.room,
                "cannot finalize replay artifact without launch-time start checkpoint"
            );
            return None;
        };
        Some(start.finalize(game, winner_id, scores))
    }

    pub(super) fn record_live_match_started(
        &mut self,
        player_count: usize,
        human_count: usize,
        map_name: String,
        participants: Vec<String>,
    ) {
        self.match_player_count = player_count;
        self.match_human_count = human_count;
        self.match_started_at = Some(chrono::Utc::now());
        self.match_run_id = Some(structured_log::new_match_run_id(&self.room));
        self.match_map_name = map_name;
        self.match_participants = participants;
        self.outcome_sent.clear();
    }

    pub(super) fn reset_after_live_match_for_room_phase(&mut self) {
        self.match_player_count = 0;
        self.match_human_count = 0;
        self.outcome_sent.clear();
        self.branch_live_seat_by_connection.clear();
        self.pending_recipient_notices.clear();
        self.reset_live_pause_state();
        self.reset_room_time_state();
        self.replay_start = None;
        for player in self.players.values_mut() {
            player.ready = false;
            player.msg_tx.clear_pending_snapshot();
        }
    }

    fn clear_finished_match_identity(&mut self) {
        self.match_started_at = None;
        self.match_run_id = None;
        self.match_map_name.clear();
        self.match_participants.clear();
        self.replay_start = None;
    }

    pub(super) fn reset_empty_room_state(&mut self) {
        self.phase = Phase::Lobby;
        self.created_at_unix_ms = current_unix_ms();
        self.match_countdown_deadline = None;
        self.match_player_count = 0;
        self.match_human_count = 0;
        self.outcome_sent.clear();
        self.branch_live_seat_by_connection.clear();
        self.pending_recipient_notices.clear();
        self.reset_live_pause_state();
        self.lab_session = None;
        self.lab_timeline = None;
        self.host_id = None;
        // Drop AI opponents too: with no humans there is nobody to host them, and a fresh
        // joiner under this room name should start from a clean lobby.
        self.ai_players.clear();
        self.human_team_assignments.clear();
        self.human_faction_assignments.clear();
        self.dev_driver = None;
        self.dev_view_player_id = None;
        self.ai_controllers.clear();
        self.pending_client_command_acks.clear();
        self.reset_room_time_state();
        self.clear_finished_match_identity();
    }

    pub(super) fn reset_live_pause_state(&mut self) {
        self.live_paused = false;
        self.live_paused_by = None;
        self.live_pause_counts.clear();
    }

    pub(super) fn reset_room_time_state(&mut self) {
        self.room_time_speed = 1.0;
        self.room_time_paused = false;
        self.lab_room_time_controller_id = None;
    }

    pub(super) fn mark_match_started_for_drain(&mut self) {
        if !self.match_tracked_for_drain
            && self.live_session_policy().tracks_active_session_for_drain()
        {
            self.match_tracked_for_drain = true;
            self.drain.match_started();
        }
    }

    pub(super) fn mark_match_finished_for_drain(&mut self) {
        if self.match_tracked_for_drain {
            self.match_tracked_for_drain = false;
            self.drain.match_finished();
        }
    }

    pub(super) fn report_disposable_if_empty(&self) {
        if self.players.is_empty() {
            if let Some(lifecycle) = &self.lifecycle {
                lifecycle.request_disposal();
            }
        }
    }

    pub(super) fn should_dispose_when_empty(&self) -> bool {
        match self.mode {
            RoomMode::Normal
            | RoomMode::DevScenario(_)
            | RoomMode::Replay { .. }
            | RoomMode::ReplayArtifact { .. }
            | RoomMode::Lab(_) => true,
            // Branch seeds exist only inside the private branch room until the branch launches.
            RoomMode::ReplayBranch { .. } => false,
        }
    }

    pub(super) fn finish_perf_tick(
        &self,
        perf: Option<&rts_sim::perf::TickPerf>,
        game: &Game,
        scheduler_lag: std::time::Duration,
        tick_start: StdInstant,
    ) {
        let Some(perf) = perf else {
            return;
        };
        perf.finish(rts_sim::perf::TickContext {
            room: &self.room,
            match_run_id: self.match_run_id.as_deref().unwrap_or(""),
            tick: game.current_tick(),
            scheduler_lag,
            total: tick_start.elapsed(),
            players: self.players.values().filter(|p| !p.spectator).count(),
            spectators: self.players.values().filter(|p| p.spectator).count(),
            ai_players: self.ai_players.len(),
            counts: game.perf_entity_counts(),
        });
    }

    fn reset_match_net_status(&mut self) {
        self.slow_tick_count = 0;
        self.pending_client_command_acks.clear();
        self.pending_recipient_notices.clear();
        for player in self.players.values_mut() {
            player.head_of_line_count = 0;
            player.last_received_client_seq = 0;
            player.last_sim_consumed_client_seq = 0;
            player.last_sim_consumed_client_tick = None;
        }
    }
}
