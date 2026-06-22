use super::connection::send_or_log;
use super::dev_replay::match_seed;
use super::lab_timeline::LabTimeline;
use super::launch::{LaunchPrediction, LaunchRecipient, StartPayloadBuilder};
use super::live_tick::{LiveTickDriver, LiveTickResult};
use super::projection::{ProjectionPolicy, RecipientRole};
use super::replay_branch::{BranchLaunchError, BranchStagingState};
use super::session_policy::{RoomTimeSource, SessionPhase, SessionPolicy};
use super::tick_control::{RoomTimeClock, ScheduledTickAction, TickControl};
use super::*;
#[cfg(test)]
use crate::protocol::{
    Command, LabClientOp, LabResult, LabStartRole, LabVisionMode, MovementPathDiagnosticScope,
    RoomTimeState, SnapshotNetStatus, StartPayload, PREDICTION_PROTOCOL_VERSION,
};
use crate::protocol::{LivePauseState, NoticeSeverity};
use crate::structured_log::{self, MatchEndedLog, MatchStartedLog};
use rts_ai::AiController;
#[cfg(test)]
use rts_sim::game::entity::EntityKind;
#[cfg(test)]
use rts_sim::game::lab::{LabOp, LabSetPlayerResources};
use rts_sim::game::map::Map;
use rts_sim::game::replay::ReplayArtifactV1;
use std::time::Instant as StdInstant;
use tokio::time::Instant as TokioInstant;

mod dev;
mod helpers;
mod lab;
mod lobby;
mod replay;
mod types;

use dev::DevDriver;
#[cfg(test)]
use helpers::match_countdown_duration;
pub(super) use helpers::{
    is_automated_match_history_room, match_history_participants_are_automated,
};
use helpers::{
    late_spectator_notice_name, live_ai_controllers, server_build_sha,
    DRAINING_NEW_MATCHES_DISABLED_MSG, LIVE_PAUSE_LIMIT,
};
#[cfg(test)]
use helpers::{LAB_PLAYER_ONE_ID, LAB_PLAYER_TWO_ID};
use lab::LabSession;
use types::{AiSlot, Phase};
pub(super) use types::{
    DevScenarioConfig, DevScenarioId, LabRoomConfig, PendingClientCommandAck, RoomMode, RoomPlayer,
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

    fn on_join_live_spectator(
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
        let start_policy = SessionPolicy::for_room(&self.mode, SessionPhase::LiveMatch);
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        let builder = StartPayloadBuilder::simulation(start_policy, &payload);
        super::launch::send_start_payloads(
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

    fn on_command(&mut self, player_id: u32, client_seq: u32, cmd: SimCommand) {
        if self.is_dev_watch() {
            return;
        }
        if client_seq == 0 {
            crate::log_debug!(room = %self.room, player_id, "ignoring command with reserved clientSeq 0");
            self.send_command_receipt(player_id, client_seq, 0, false, Some("invalidSeq"));
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
                        game.enqueue(issuer.seat_id, cmd);
                        self.pending_client_command_acks
                            .push(PendingClientCommandAck {
                                connection_id: issuer.connection_id,
                                client_seq,
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

    fn live_pause_seat_for_connection(&self, connection_id: u32) -> Option<u32> {
        if !matches!(self.phase, Phase::InGame(_)) || !self.live_pause_controls_available() {
            return None;
        }
        if self.outcome_sent.contains(&connection_id) {
            return None;
        }
        self.live_connection_is_player(connection_id).then(|| {
            self.live_seat_id_for_connection(connection_id)
                .unwrap_or(connection_id)
        })
    }

    fn live_pause_state_for(&self, connection_id: u32) -> LivePauseState {
        let seat_id = self.live_pause_seat_for_connection(connection_id);
        let pauses_remaining = seat_id.map(|seat_id| {
            LIVE_PAUSE_LIMIT
                .saturating_sub(self.live_pause_counts.get(&seat_id).copied().unwrap_or(0))
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
            can_unpause: self.live_paused && seat_id.is_some(),
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

    fn broadcast_live_pause_state(&self) {
        if !matches!(self.phase, Phase::InGame(_)) || !self.live_pause_controls_available() {
            return;
        }
        for &connection_id in &self.order {
            self.send_live_pause_state_to(connection_id);
        }
    }

    fn on_pause_game(&mut self, player_id: u32) {
        let Some(seat_id) = self.live_pause_seat_for_connection(player_id) else {
            self.send_live_pause_state_to(player_id);
            return;
        };
        if self.live_paused {
            self.send_live_pause_state_to(player_id);
            return;
        }
        let used = self.live_pause_counts.get(&seat_id).copied().unwrap_or(0);
        if used >= LIVE_PAUSE_LIMIT {
            self.send_live_pause_state_to(player_id);
            return;
        }
        self.live_pause_counts
            .insert(seat_id, used.saturating_add(1));
        self.live_paused = true;
        self.live_paused_by = Some(seat_id);
        crate::log_info!(room = %self.room, player_id, seat_id, "live match paused");
        self.broadcast_live_pause_state();
    }

    fn on_unpause_game(&mut self, player_id: u32) {
        if self.live_pause_seat_for_connection(player_id).is_none() {
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

    fn on_give_up(&mut self, player_id: u32) {
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

    // -- Lobby phase ---------------------------------------------------------

    fn on_join_branch_staging(
        &mut self,
        player_id: u32,
        name: String,
        msg_tx: ConnectionSink,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        if self.players.contains_key(&player_id) {
            let _ = ack.send(false);
            return;
        }
        if !matches!(self.phase, Phase::BranchStaging(_)) {
            let seed = match &self.mode {
                RoomMode::ReplayBranch { seed } => seed.clone(),
                _ => {
                    let _ = ack.send(false);
                    return;
                }
            };
            self.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));
        }
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
        let _ = ack.send(true);
        self.send_current_shutdown_warning_to(player_id);
        self.broadcast_branch_staging();
    }

    fn on_join_branch_live(
        &mut self,
        player_id: u32,
        name: String,
        msg_tx: ConnectionSink,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        if self.players.contains_key(&player_id) {
            let _ = ack.send(false);
            return;
        }
        self.on_join_live_spectator(player_id, name, true, msg_tx, ack);
    }

    /// Transition from `Lobby` to `InGame`: create the simulation and send each player their
    /// own `start` payload. Only called from `on_start_request` once preconditions hold.
    fn start_match(&mut self) {
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
        for (seat, ai) in self.ai_players.iter().enumerate() {
            inits.push(PlayerInit {
                id: ai.id,
                team_id: ai.team_id,
                faction_id: ai.faction_id.clone(),
                name: ai.name.clone(),
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
        let start_policy = SessionPolicy::for_room(&self.mode, SessionPhase::LiveMatch);
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
        super::launch::send_start_payloads(&self.room, &builder, recipients);

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
        self.broadcast_live_pause_state();
    }

    fn start_branch_live(&mut self) {
        self.prepare_live_match_launch();
        let staging = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::BranchStaging(staging) => staging,
            other => {
                self.phase = other;
                return;
            }
        };
        let launch = match staging
            .prepare_launch(|connection_id| self.players.contains_key(&connection_id))
        {
            Ok(launch) => launch,
            Err(BranchLaunchError::UnsupportedFaction {
                seat_player_id,
                requested,
                reason,
            }) => {
                crate::log_warn!(
                    room = %self.room,
                    seat_player_id,
                    faction_id = ?requested,
                    reason = ?reason,
                    "replay branch seat rejected by faction policy"
                );
                self.phase = Phase::BranchStaging(staging);
                self.broadcast_branch_staging();
                return;
            }
            Err(BranchLaunchError::NotReady | BranchLaunchError::MissingOccupant) => {
                self.phase = Phase::BranchStaging(staging);
                self.broadcast_branch_staging();
                return;
            }
        };

        let game = launch.game;
        self.branch_live_seat_by_connection = launch.seat_by_connection;
        self.record_live_match_started(
            launch.match_player_count,
            launch.match_player_count,
            launch.map_name,
            launch.participants,
        );
        let mut payload = game.start_payload();
        payload.match_run_id = self.match_run_id.clone();
        self.ai_controllers.clear();
        self.dev_driver = None;
        self.dev_view_player_id = None;

        let projection_policy = self.projection_policy_for_phase(SessionPhase::LiveMatch);
        let start_policy = SessionPolicy::for_room(&self.mode, SessionPhase::LiveMatch);
        let mut recipients = Vec::new();
        for &connection_id in &self.order {
            let Some(player) = self.players.get_mut(&connection_id) else {
                continue;
            };
            let mapped_seat = self
                .branch_live_seat_by_connection
                .get(&connection_id)
                .copied();
            let role = if mapped_seat.is_some() {
                RecipientRole::ActivePlayer
            } else {
                RecipientRole::Spectator
            };
            player.spectator = mapped_seat.is_none();
            player.ready = false;
            recipients.push(LaunchRecipient {
                connection_id,
                payload_player_id: mapped_seat.unwrap_or(connection_id),
                role,
                prediction: if mapped_seat.is_some() {
                    LaunchPrediction::Enabled
                } else {
                    LaunchPrediction::Disabled
                },
                diagnostics: projection_policy.diagnostic_capabilities_for(role),
                clear_pending_snapshot: true,
                lab: None,
                msg_tx: player.msg_tx.clone(),
            });
        }
        let builder = StartPayloadBuilder::simulation(start_policy, &payload);
        super::launch::send_start_payloads(&self.room, &builder, recipients);

        structured_log::log_match_started(MatchStartedLog {
            room: &self.room,
            match_run_id: self.match_run_id.as_deref().unwrap_or(""),
            mode: "replay_branch",
            map: &self.match_map_name,
            seed: launch.seed,
            players: self.match_player_count,
            humans: self.match_human_count,
            ai: 0,
            participants: &self.match_participants,
        });
        self.mark_match_started_for_drain();
        self.phase = Phase::InGame(Box::new(game));
        self.broadcast_live_pause_state();
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

    fn on_tick_live_game(&mut self, scheduled: TokioInstant) {
        if self.live_paused && self.live_pause_controls_available() {
            return;
        }
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
        let lab_snapshot_projections = self.lab_snapshot_projections(&game);
        let record_lab_timeline = matches!(self.mode, RoomMode::Lab(_));
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
            lab_snapshot_projections,
            projection_policy,
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

    fn on_claim_branch_seat(&mut self, player_id: u32, seat_player_id: u32) {
        if !self.players.contains_key(&player_id) {
            return;
        }
        let result = match &mut self.phase {
            Phase::BranchStaging(staging) => staging.claim(player_id, seat_player_id),
            _ => return,
        };
        match result {
            Ok(()) => self.broadcast_branch_staging(),
            Err(err) => self.send_error_to(player_id, err),
        }
    }

    fn on_release_branch_seat(&mut self, player_id: u32, seat_player_id: u32) {
        if !self.players.contains_key(&player_id) {
            return;
        }
        let released = match &mut self.phase {
            Phase::BranchStaging(staging) => staging.release(player_id, seat_player_id),
            _ => return,
        };
        if released {
            self.broadcast_branch_staging();
        }
    }

    fn on_start_branch(&mut self, player_id: u32) {
        if self.host_id != Some(player_id) {
            return;
        }
        if self.match_countdown_deadline.is_some() {
            return;
        }
        if self.new_live_session_blocked_by_drain() {
            self.send_error_to(player_id, DRAINING_NEW_MATCHES_DISABLED_MSG);
            return;
        }
        let Some(staging) = self.branch_staging() else {
            return;
        };
        if !staging.can_start() {
            self.send_error_to(
                player_id,
                "All original branch seats must be claimed before launch.",
            );
            return;
        }
        self.start_match_countdown();
    }

    fn on_announce_replay_branch(
        &self,
        branch_room: String,
        source_tick: u32,
        seats: Vec<ReplayBranchSeat>,
    ) {
        if !matches!(self.phase, Phase::ReplayViewer(_)) {
            return;
        }
        self.broadcast(&ServerMessage::ReplayBranchCreated {
            branch_room,
            source_tick,
            seats,
        });
    }

    fn branch_staging(&self) -> Option<&BranchStagingState> {
        match &self.phase {
            Phase::BranchStaging(staging) => Some(staging),
            _ => None,
        }
    }

    fn branch_staging_message(&self, staging: &BranchStagingState) -> ServerMessage {
        let occupants = self
            .order
            .iter()
            .filter_map(|id| {
                self.players.get(id).map(|player| BranchStagingOccupant {
                    id: *id,
                    name: player.name.clone(),
                })
            })
            .collect();
        staging.message(
            self.room.clone(),
            self.host_id.unwrap_or(0),
            occupants,
            self.match_countdown_deadline.is_none() && !self.drain.is_draining(),
        )
    }

    fn broadcast_branch_staging(&self) {
        let Some(staging) = self.branch_staging() else {
            return;
        };
        self.broadcast(&self.branch_staging_message(staging));
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

    fn team_id_for_score_seat(
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
