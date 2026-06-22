use super::connection::send_or_log;
use super::lab_timeline::LabTimeline;
use super::projection::ProjectionPolicy;
use super::session_policy::{RoomTimeSource, SessionPhase, SessionPolicy};
use super::tick_control::{RoomTimeClock, ScheduledTickAction, TickControl};
use super::*;
#[cfg(test)]
use crate::protocol::{
    Command, LabClientOp, LabResult, LabStartRole, LabVisionMode, MovementPathDiagnosticScope,
    RoomTimeState, SnapshotNetStatus, StartPayload, PREDICTION_PROTOCOL_VERSION,
};
use crate::structured_log::{self, MatchEndedLog};
use rts_ai::AiController;
#[cfg(test)]
use rts_sim::game::entity::EntityKind;
#[cfg(test)]
use rts_sim::game::lab::{LabOp, LabSetPlayerResources};
use rts_sim::game::replay::ReplayArtifactV1;
use std::time::Instant as StdInstant;
use tokio::time::Instant as TokioInstant;

mod branch;
mod dev;
mod helpers;
mod lab;
mod live;
mod lobby;
mod replay;
mod types;

use dev::DevDriver;
#[cfg(test)]
use helpers::match_countdown_duration;
use helpers::server_build_sha;
pub(super) use helpers::{
    is_automated_match_history_room, match_history_participants_are_automated,
};
#[cfg(test)]
use helpers::{LAB_PLAYER_ONE_ID, LAB_PLAYER_TWO_ID};
use lab::LabSession;
use types::{AiSlot, Phase};
pub(super) use types::{
    DevScenarioConfig, DevScenarioId, LabRoomConfig, LabScenarioPreset, PendingClientCommandAck,
    RoomMode, RoomPlayer,
};

pub(super) struct RoomTask {
    room: String,
    mode: RoomMode,
    /// Connected players in join order (join order drives lobby display and host fallback).
    order: Vec<u32>,
    /// Wall-clock creation/reset time for the public lobby browser age column.
    created_at_unix_ms: u64,
    pub(super) players: HashMap<u32, RoomPlayer>,
    /// Computer opponents the host has added, in add order. Persist across rematches; cleared
    /// only when the room empties of humans.
    ai_players: Vec<AiSlot>,
    /// Team ids are freeform host-managed slots in the range `1..=MAX_LOBBY_TEAMS`.
    /// Per-human active-seat team assignment. Spectators are omitted and broadcast as team 0.
    human_team_assignments: HashMap<u32, TeamId>,
    /// Per-human active-seat faction selection. Spectators are omitted.
    human_faction_assignments: HashMap<u32, String>,
    /// Name of the map the host has selected (display name from JSON `name` field).
    selected_map: String,
    /// Current host (first joiner; reassigned to the next in `order` when the host leaves).
    host_id: Option<u32>,
    phase: Phase,
    /// Number of players (humans + AI) the in-progress match started with. Used so a lone-player
    /// sandbox never ends while a 2+ player match (including human-vs-AI) resolves to a winner.
    /// `0` outside a match.
    match_player_count: usize,
    /// Number of human (non-AI) players the in-progress match started with. `0` outside a match.
    match_human_count: usize,
    /// Connected human players who already received a terminal score screen for the active match.
    outcome_sent: HashSet<u32>,
    /// In replay branch live matches, connected ids differ from original replay player ids.
    branch_live_seat_by_connection: HashMap<u32, u32>,
    /// Live-match pause is room-owned control-plane state, separate from replay/dev room-time.
    live_paused: bool,
    live_paused_by: Option<u32>,
    live_pause_counts: HashMap<u32, u8>,
    lab_session: Option<LabSession>,
    lab_timeline: Option<LabTimeline>,
    dev_driver: Option<DevDriver>,
    dev_view_player_id: Option<u32>,
    ai_controllers: Vec<AiController>,
    /// Room-time speed multiplier; 1.0 = real-time, 2.0 = 2x faster, etc.
    room_time_speed: f32,
    /// Room-time pause flag. Kept separate from room_time_speed so interval creation never divides
    /// by zero and resume can restore the previous non-zero multiplier.
    room_time_paused: bool,
    lab_room_time_controller_id: Option<u32>,
    slow_tick_count: u32,
    pending_client_command_acks: Vec<PendingClientCommandAck>,
    /// Recipient-specific room-owned notices appended to the next live snapshot for each
    /// connection id. Used when the notice is about room membership rather than sim events.
    pending_recipient_notices: HashMap<u32, Vec<Event>>,
    /// Optional persistence sink for resolved matches. `None` disables match-history writes.
    db: Option<Arc<Db>>,
    /// When true, rows written by this room are hidden from non-localhost match-history reads.
    match_history_local_only: bool,
    /// Wall-clock start time of the currently-running match. `None` outside `Phase::InGame`.
    match_started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Correlates every high-signal log line for one live match attempt.
    match_run_id: Option<String>,
    /// Map name the active match was started on. Empty outside `Phase::InGame`.
    match_map_name: String,
    /// Display names of every participant (humans + AI) in seat order, for match-history rows.
    match_participants: Vec<String>,
    /// Pre-match countdown deadline. While set, lobby membership/settings are frozen and the
    /// match starts on the first room tick at or after this instant.
    match_countdown_deadline: Option<TokioInstant>,
    drain: DrainHandle,
    match_tracked_for_drain: bool,
    lifecycle: Option<RoomLifecycle>,
}

impl RoomTask {
    pub(super) fn new(
        room: String,
        mode: RoomMode,
        db: Option<Arc<Db>>,
        match_history_local_only: bool,
        drain: DrainHandle,
    ) -> Self {
        RoomTask {
            room,
            mode,
            order: Vec::new(),
            created_at_unix_ms: current_unix_ms(),
            players: HashMap::new(),
            ai_players: Vec::new(),
            human_team_assignments: HashMap::new(),
            human_faction_assignments: HashMap::new(),
            selected_map: "Default".to_string(),
            host_id: None,
            phase: Phase::Lobby,
            match_player_count: 0,
            match_human_count: 0,
            outcome_sent: HashSet::new(),
            branch_live_seat_by_connection: HashMap::new(),
            live_paused: false,
            live_paused_by: None,
            live_pause_counts: HashMap::new(),
            lab_session: None,
            lab_timeline: None,
            dev_driver: None,
            dev_view_player_id: None,
            ai_controllers: Vec::new(),
            room_time_speed: 1.0,
            room_time_paused: false,
            lab_room_time_controller_id: None,
            slow_tick_count: 0,
            pending_client_command_acks: Vec::new(),
            pending_recipient_notices: HashMap::new(),
            db,
            match_history_local_only,
            match_started_at: None,
            match_run_id: None,
            match_map_name: String::new(),
            match_participants: Vec::new(),
            match_countdown_deadline: None,
            drain,
            match_tracked_for_drain: false,
            lifecycle: None,
        }
    }

    pub(super) fn new_with_lifecycle(
        room: String,
        mode: RoomMode,
        db: Option<Arc<Db>>,
        match_history_local_only: bool,
        drain: DrainHandle,
        lifecycle: RoomLifecycle,
    ) -> Self {
        let mut task = Self::new(room, mode, db, match_history_local_only, drain);
        task.lifecycle = Some(lifecycle);
        task
    }

    /// Main loop: multiplex the fixed-rate tick against the inbound event stream. Returns (and
    /// the task ends) when the event channel closes or the registry explicitly disposes this room.
    pub(super) async fn run(
        &mut self,
        mut event_rx: mpsc::Receiver<RoomEvent>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        let mut ticker = self.make_ticker();

        loop {
            if *shutdown_rx.borrow_and_update() {
                return;
            }
            let mut speed_changed = false;
            tokio::select! {
                // Bias is irrelevant for correctness: events are timestamped only by arrival
                // order, and a tick handles whatever has been applied so far.
                scheduled = ticker.tick() => {
                    self.on_tick(scheduled);
                }
                changed = shutdown_rx.changed() => {
                    match changed {
                        Ok(()) if *shutdown_rx.borrow_and_update() => return,
                        Ok(()) => {}
                        Err(_) => return,
                    }
                }
                maybe_event = event_rx.recv() => {
                    match maybe_event {
                        Some(event) => {
                            let old_speed = self.current_speed_multiplier();
                            self.handle_event(event);
                            speed_changed = self.current_speed_multiplier() != old_speed;
                        }
                        None => return, // registry dropped; shut the room down.
                    }
                }
            }
            if speed_changed {
                ticker = self.make_ticker();
            }
        }
    }

    fn make_ticker(&self) -> tokio::time::Interval {
        let dur = self.current_tick_interval();
        let mut t = interval(dur);
        // If the loop ever falls behind (e.g. a long GC pause), skip missed ticks rather than
        // bursting to catch up — the simulation stays close to real time.
        t.set_missed_tick_behavior(MissedTickBehavior::Skip);
        t
    }

    pub(super) fn current_tick_interval(&self) -> Duration {
        let base =
            test_tick_interval_override().unwrap_or_else(|| Duration::from_millis(config::TICK_MS));
        self.tick_control().tick_interval(base)
    }

    fn current_speed_multiplier(&self) -> f32 {
        self.tick_control().speed_multiplier()
    }

    fn tick_control(&self) -> TickControl {
        let policy = self.session_policy();
        let room_time = match (&self.phase, policy.clock.room_time_source()) {
            (Phase::ReplayViewer(session), Some(RoomTimeSource::ReplayPlayback)) => {
                Some(RoomTimeClock {
                    speed: session.speed(),
                    paused: session.is_paused(),
                })
            }
            (_, Some(RoomTimeSource::DevScenario)) => Some(RoomTimeClock {
                speed: self.room_time_speed,
                paused: self.room_time_paused,
            }),
            (_, Some(RoomTimeSource::Lab)) => Some(RoomTimeClock {
                speed: self.room_time_speed,
                paused: self.room_time_paused,
            }),
            _ => None,
        };
        TickControl::new(
            policy.clock,
            room_time,
            self.room_time_speed,
            self.match_countdown_deadline.is_some(),
        )
    }

    fn session_phase(&self) -> SessionPhase {
        match &self.phase {
            Phase::Lobby => SessionPhase::Lobby,
            Phase::InGame(_) => SessionPhase::LiveMatch,
            Phase::ReplayViewer(_) => SessionPhase::ReplayViewer,
            Phase::BranchStaging(_) => SessionPhase::BranchStaging,
        }
    }

    fn session_policy(&self) -> SessionPolicy {
        SessionPolicy::for_room(&self.mode, self.session_phase())
    }

    fn projection_policy(&self) -> ProjectionPolicy {
        self.projection_policy_for_phase(self.session_phase())
    }

    fn projection_policy_for_phase(&self, phase: SessionPhase) -> ProjectionPolicy {
        let policy = self.session_policy();
        let policy = if policy.phase == phase {
            policy
        } else {
            SessionPolicy::for_room(&self.mode, phase)
        };
        ProjectionPolicy::new(policy.visibility, policy.diagnostics)
    }

    fn is_dev_watch(&self) -> bool {
        self.session_policy().is_dev_watch()
    }

    fn live_session_policy(&self) -> SessionPolicy {
        SessionPolicy::for_room(&self.mode, SessionPhase::LiveMatch)
    }

    fn new_live_session_blocked_by_drain(&self) -> bool {
        self.drain.is_draining()
            && !self
                .live_session_policy()
                .allows_new_session_while_draining()
    }

    fn should_persist_match_history(&self) -> bool {
        let match_policy = self.live_session_policy();
        self.match_player_count >= 1
            && match_policy.has_authoritative_mutation()
            && match_policy.allows_match_history()
            && !is_automated_match_history_room(&self.room)
            && !match_history_participants_are_automated(&self.match_participants)
    }

    fn should_capture_post_match_replay(&self) -> bool {
        let match_policy = self.live_session_policy();
        match_policy.captures_post_match_replay()
    }

    fn should_attach_match_history_replay_artifact(&self) -> bool {
        let match_policy = self.live_session_policy();
        match_policy.attaches_match_history_replay_artifact()
    }

    // -- Event handling ------------------------------------------------------

    fn handle_event(&mut self, event: RoomEvent) {
        match event {
            RoomEvent::Summary { reply } => {
                let _ = reply.send(self.lobby_summary());
            }
            RoomEvent::Join {
                player_id,
                name,
                spectator,
                replay_ok,
                msg_tx,
                ack,
            } => self.on_join(player_id, name, spectator, replay_ok, msg_tx, ack),
            RoomEvent::Leave { player_id } => self.on_leave(player_id),
            RoomEvent::Ready { player_id, ready } => self.on_ready(player_id, ready),
            RoomEvent::StartRequest { player_id } => self.on_start_request(player_id),
            RoomEvent::SetTeamPreset { player_id, preset } => {
                self.on_set_team_preset(player_id, preset)
            }
            RoomEvent::SetTeam {
                player_id,
                target,
                team_id,
            } => self.on_set_team(player_id, target, team_id),
            RoomEvent::SetFaction {
                player_id,
                faction_id,
            } => self.on_set_faction(player_id, faction_id),
            RoomEvent::AddAi {
                player_id,
                team_id,
                ai_profile_id,
            } => self.on_add_ai(player_id, team_id, ai_profile_id),
            RoomEvent::SetAiProfile {
                player_id,
                target,
                ai_profile_id,
            } => self.on_set_ai_profile(player_id, target, ai_profile_id),
            RoomEvent::RemoveAi { player_id, target } => self.on_remove_ai(player_id, target),
            RoomEvent::SetSpectator {
                player_id,
                target,
                spectator,
            } => self.on_set_spectator(player_id, target, spectator),
            RoomEvent::Command {
                player_id,
                client_seq,
                cmd,
            } => self.on_command(player_id, client_seq, cmd),
            RoomEvent::GiveUp { player_id } => self.on_give_up(player_id),
            RoomEvent::PauseGame { player_id } => self.on_pause_game(player_id),
            RoomEvent::UnpauseGame { player_id } => self.on_unpause_game(player_id),
            RoomEvent::ReturnToLobby { player_id } => self.on_return_to_lobby(player_id),
            RoomEvent::SetRoomTimeSpeed { player_id, speed } => {
                self.on_set_room_time_speed(player_id, speed)
            }
            RoomEvent::StepRoomTime { player_id } => self.on_step_room_time(player_id),
            RoomEvent::SeekRoomTime {
                player_id,
                ticks_back,
            } => self.on_seek_room_time(player_id, ticks_back),
            RoomEvent::SeekRoomTimeTo { player_id, tick } => {
                self.on_seek_room_time_to(player_id, tick)
            }
            RoomEvent::SetReplayVision { player_id, vision } => {
                self.on_set_replay_vision(player_id, vision)
            }
            RoomEvent::Lab {
                player_id,
                request_id,
                op,
            } => self.on_lab_request(player_id, request_id, op),
            RoomEvent::RequestReplayBranch { player_id, reply } => {
                let _ = reply.send(self.on_request_replay_branch(player_id));
            }
            RoomEvent::ClaimBranchSeat {
                player_id,
                seat_player_id,
            } => self.on_claim_branch_seat(player_id, seat_player_id),
            RoomEvent::ReleaseBranchSeat {
                player_id,
                seat_player_id,
            } => self.on_release_branch_seat(player_id, seat_player_id),
            RoomEvent::StartBranch { player_id } => self.on_start_branch(player_id),
            RoomEvent::AnnounceReplayBranch {
                branch_room,
                source_tick,
                seats,
            } => self.on_announce_replay_branch(branch_room, source_tick, seats),
            RoomEvent::SelectMap { player_id, map } => self.on_select_map(player_id, map),
            RoomEvent::ReportDisposableIfEmpty => self.report_disposable_if_empty(),
            RoomEvent::DrainStarted(notice) => self.on_drain_started(notice),
        }
    }

    fn on_drain_started(&mut self, notice: DrainNotice) {
        self.broadcast_shutdown_warning(notice);
        if matches!(self.phase, Phase::Lobby) {
            self.broadcast_lobby();
        }
    }

    fn send_current_shutdown_warning_to(&self, player_id: u32) {
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

    // -- In-game phase -------------------------------------------------------

    /// One simulation step. No-op in the `Lobby` phase (the ticker keeps running so a room is
    /// always live and ready to start).
    fn on_tick(&mut self, scheduled: TokioInstant) {
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
            | ScheduledTickAction::LiveMatch => {
                self.on_tick_live_game(scheduled);
            }
        }
    }

    /// Resolve a finished match: tell everyone who won and start post-match replay playback.
    fn end_match(&mut self, winner_id: Option<u32>, scores: Vec<PlayerScore>, game: Option<&Game>) {
        let winner_team_id =
            winner_id.and_then(|id| Self::team_id_for_score_seat(game, &scores, id));
        let ended_at = chrono::Utc::now();
        let duration_ms = self.match_started_at.map(|started_at| {
            ended_at
                .signed_duration_since(started_at)
                .num_milliseconds()
                .clamp(0, i32::MAX as i64)
        });
        let duration_ticks = game.map(Game::tick_count);
        let max_head_of_line_count = self
            .players
            .values()
            .map(|player| player.head_of_line_count)
            .max()
            .unwrap_or(0);
        let replay_artifact = game
            .filter(|_| self.should_capture_post_match_replay())
            .map(|game| {
                ReplayArtifactV1::capture_from_game(
                    game,
                    server_build_sha(),
                    winner_id,
                    scores.clone(),
                )
            });
        let will_record_history = self.db.is_some()
            && self.match_started_at.is_some()
            && self.should_persist_match_history();
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
        });

        // Persist replay-backed history for deploy-recorded matches. The Recent Matches endpoint
        // filters debug and AI-only rows; persistence keeps their replay artifacts available for
        // follow-up diagnostics without exposing them on the lobby front page.
        if let (Some(db), Some(started_at)) = (self.db.clone(), self.match_started_at) {
            if self.should_persist_match_history() {
                let duration_ms = ended_at
                    .signed_duration_since(started_at)
                    .num_milliseconds()
                    .clamp(0, i32::MAX as i64) as i32;
                let winner_name = winner_id
                    .and_then(|wid| scores.iter().find(|s| s.id == wid).map(|s| s.name.clone()));
                let score_json = serde_json::to_value(&scores).unwrap_or(serde_json::Value::Null);
                let replay = if self.should_attach_match_history_replay_artifact() {
                    replay_artifact.as_ref().and_then(|artifact| {
                        match crate::db::MatchReplayRecord::from_artifact(artifact) {
                            Ok(replay) => Some(replay),
                            Err(err) => {
                                crate::log_warn!(room = %self.room, error = %err, "failed to serialize replay artifact for match history");
                                None
                            }
                        }
                    })
                } else {
                    None
                };
                let rec = crate::db::MatchRecord {
                    started_at,
                    ended_at,
                    duration_ms,
                    map_name: self.match_map_name.clone(),
                    winner_name,
                    participants: self.match_participants.clone(),
                    score_screen: score_json,
                    human_count: i32::try_from(self.match_human_count).unwrap_or(i32::MAX),
                    debug_mode: false,
                    local_only: self.match_history_local_only,
                    replay,
                };
                // Detached: a slow Supabase write must never stall the room transitioning back to
                // lobby. Errors are logged inside `record_match`.
                tokio::spawn(async move { db.record_match(rec).await });
            }
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

    fn return_to_lobby(&mut self) {
        // Reset for the next match: drop the game/replay, clear ready flags, and re-advertise
        // the lobby. AI slots and map selection persist for rematches.
        self.phase = Phase::Lobby;
        self.reset_after_live_match_for_room_phase();
        self.broadcast_lobby();
    }

    fn prepare_live_match_launch(&mut self) {
        self.match_countdown_deadline = None;
        self.reset_match_net_status();
        self.reset_live_pause_state();
        self.reset_room_time_state();
    }

    fn record_live_match_started(
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

    fn reset_after_live_match_for_room_phase(&mut self) {
        self.match_player_count = 0;
        self.match_human_count = 0;
        self.outcome_sent.clear();
        self.branch_live_seat_by_connection.clear();
        self.pending_recipient_notices.clear();
        self.reset_live_pause_state();
        self.reset_room_time_state();
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
    }

    fn reset_empty_room_state(&mut self) {
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

    fn reset_live_pause_state(&mut self) {
        self.live_paused = false;
        self.live_paused_by = None;
        self.live_pause_counts.clear();
    }

    fn reset_room_time_state(&mut self) {
        self.room_time_speed = 1.0;
        self.room_time_paused = false;
        self.lab_room_time_controller_id = None;
    }

    fn mark_match_started_for_drain(&mut self) {
        if !self.match_tracked_for_drain
            && self.live_session_policy().tracks_active_session_for_drain()
        {
            self.match_tracked_for_drain = true;
            self.drain.match_started();
        }
    }

    fn mark_match_finished_for_drain(&mut self) {
        if self.match_tracked_for_drain {
            self.match_tracked_for_drain = false;
            self.drain.match_finished();
        }
    }

    fn report_disposable_if_empty(&self) {
        if self.players.is_empty() {
            if let Some(lifecycle) = &self.lifecycle {
                lifecycle.request_disposal();
            }
        }
    }

    fn should_dispose_when_empty(&self) -> bool {
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

    // -- Sending helpers -----------------------------------------------------

    fn finish_perf_tick(
        &self,
        perf: Option<&rts_sim::perf::TickPerf>,
        game: &Game,
        scheduler_lag: Duration,
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

    /// Send one message to every connected player. Closed sinks are logged and skipped; the
    /// owning connection task is responsible for emitting the eventual `Leave`.
    fn broadcast(&self, msg: &ServerMessage) {
        for &id in &self.order {
            if let Some(player) = self.players.get(&id) {
                send_or_log(&self.room, id, &player.msg_tx, msg.clone());
            }
        }
    }

    fn send_error_to(&self, player_id: u32, msg: &str) {
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        send_or_log(
            &self.room,
            player_id,
            &player.msg_tx,
            ServerMessage::Error {
                msg: msg.to_string(),
            },
        );
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

fn test_tick_interval_override() -> Option<Duration> {
    #[cfg(test)]
    {
        None
    }
    #[cfg(not(test))]
    {
        std::env::var("RTS_TEST_TICK_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|&millis| millis > 0)
            .map(Duration::from_millis)
    }
}

#[cfg(test)]
mod tests;
