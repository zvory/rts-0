use super::connection::send_or_log;
use super::lab_timeline::LabTimeline;
use super::projection::ProjectionPolicy;
use super::session_policy::{RoomTimeSource, SessionPhase, SessionPolicy, SessionPolicyContext};
use super::tick_control::{RoomTimeClock, TickControl};
use super::*;
use crate::lab_scenario_submission::LabScenarioSubmissionService;
#[cfg(test)]
use crate::protocol::{
    Command, LabClientOp, LabResult, LabStartRole, LabVisionMode, MovementPathDiagnosticScope,
    RoomTimeState, SnapshotNetStatus, StartPayload, PREDICTION_PROTOCOL_VERSION,
};
use rts_ai::AiController;
#[cfg(test)]
use rts_sim::game::entity::EntityKind;
#[cfg(test)]
use rts_sim::game::lab::{LabOp, LabSetPlayerResources};
use rts_sim::game::replay::ReplayStartComposition;
use tokio::time::Instant as TokioInstant;

mod branch;
mod dev;
mod helpers;
mod lab;
mod lifecycle;
mod live;
mod lobby;
mod match_history;
mod replay;
mod types;

#[cfg(test)]
use super::replay_session::ReplaySession;
use dev::DevDriver;
#[cfg(test)]
use helpers::match_countdown_duration;
#[cfg(test)]
use helpers::server_build_sha;
pub(super) use helpers::{
    is_automated_match_history_room, match_history_participants_are_automated,
};
#[cfg(test)]
use helpers::{LAB_PLAYER_ONE_ID, LAB_PLAYER_TWO_ID};
use lab::LabSession;
pub(in crate::lobby) use types::AiSlot;
use types::Phase;
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
    lab_scenario_submission: LabScenarioSubmissionService,
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
    match_history_writer: Option<match_history_writes::SharedMatchHistoryWriter>,
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
    /// Tick-zero checkpoint-backed replay start captured at match launch and consumed when any
    /// replay artifact is finalized. This stays outside `Game` so final state cannot be mistaken
    /// for replay start state.
    replay_start: Option<ReplayStartComposition>,
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
        match_history_writer: Option<match_history_writes::SharedMatchHistoryWriter>,
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
            lab_scenario_submission: LabScenarioSubmissionService::disabled(),
            dev_driver: None,
            dev_view_player_id: None,
            ai_controllers: Vec::new(),
            room_time_speed: 1.0,
            room_time_paused: false,
            lab_room_time_controller_id: None,
            slow_tick_count: 0,
            pending_client_command_acks: Vec::new(),
            pending_recipient_notices: HashMap::new(),
            match_history_writer,
            match_history_local_only,
            match_started_at: None,
            match_run_id: None,
            match_map_name: String::new(),
            match_participants: Vec::new(),
            replay_start: None,
            match_countdown_deadline: None,
            drain,
            match_tracked_for_drain: false,
            lifecycle: None,
        }
    }

    pub(super) fn new_with_lifecycle(
        room: String,
        mode: RoomMode,
        match_history_writer: Option<match_history_writes::SharedMatchHistoryWriter>,
        match_history_local_only: bool,
        drain: DrainHandle,
        lifecycle: RoomLifecycle,
    ) -> Self {
        let mut task = Self::new(
            room,
            mode,
            match_history_writer,
            match_history_local_only,
            drain,
        );
        task.lifecycle = Some(lifecycle);
        task
    }

    pub(super) fn with_lab_scenario_submission(
        mut self,
        service: LabScenarioSubmissionService,
    ) -> Self {
        self.lab_scenario_submission = service;
        self
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
            (_, Some(RoomTimeSource::LiveGame)) => Some(RoomTimeClock {
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
        self.session_policy_for_phase(self.session_phase())
    }

    fn session_policy_for_phase(&self, phase: SessionPhase) -> SessionPolicy {
        SessionPolicy::for_room_with_context(
            &self.mode,
            phase,
            self.session_policy_context_for_phase(phase),
        )
    }

    fn session_policy_context_for_phase(&self, phase: SessionPhase) -> SessionPolicyContext {
        SessionPolicyContext {
            ai_only_live_match: phase == SessionPhase::LiveMatch
                && matches!(self.mode, RoomMode::Normal)
                && self.match_player_count > 0
                && self.match_human_count == 0,
        }
    }

    fn projection_policy(&self) -> ProjectionPolicy {
        self.projection_policy_for_phase(self.session_phase())
    }

    fn projection_policy_for_phase(&self, phase: SessionPhase) -> ProjectionPolicy {
        let policy = self.session_policy_for_phase(phase);
        ProjectionPolicy::new(policy.visibility, policy.diagnostics)
    }

    fn is_dev_watch(&self) -> bool {
        self.session_policy().is_dev_watch()
    }

    fn live_session_policy(&self) -> SessionPolicy {
        self.session_policy_for_phase(SessionPhase::LiveMatch)
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
                lifecycle,
            } => self.on_command_with_lifecycle(player_id, client_seq, cmd, lifecycle),
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
            RoomEvent::SetVisionSelection {
                player_id,
                selection,
            } => self.on_set_vision_selection(player_id, selection),
            RoomEvent::Lab {
                player_id,
                request_id,
                op,
            } => self.on_lab_request(player_id, request_id, op),
            RoomEvent::RequestBranchFromTick { player_id, reply } => {
                let _ = reply.send(self.on_request_branch_from_tick(player_id));
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
            RoomEvent::AnnounceBranchFromTick {
                branch_room,
                source_tick,
                seats,
            } => self.on_announce_branch_from_tick(branch_room, source_tick, seats),
            RoomEvent::SelectMap { player_id, map } => self.on_select_map(player_id, map),
            RoomEvent::ReportDisposableIfEmpty => self.report_disposable_if_empty(),
            RoomEvent::DrainStarted(notice) => self.on_drain_started(notice),
            RoomEvent::FinalizeForShutdown { ack } => {
                let _ = ack.send(self.finalize_for_shutdown());
            }
        }
    }

    // -- Sending helpers -----------------------------------------------------

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
