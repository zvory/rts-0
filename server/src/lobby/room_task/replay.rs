use std::collections::HashMap;
use std::time::{Duration, Instant as StdInstant};

use tokio::time::Instant as TokioInstant;

use super::super::connection::{send_or_log, ConnectionSink};
use super::super::crash_replay::{dump_crash_replay_artifact, panic_reason};
use super::super::dev_replay::load_replay_artifact;
use super::super::launch::{LaunchPrediction, StartPayloadBuilder, StartPayloadRecipient};
use super::super::participants::replay_viewer;
use super::super::projection::{
    observer_view_from_selection, scope_observer_analysis, ObserverAnalysisAudience,
    ProjectionPolicy, RecipientRole,
};
use super::super::replay_session::{
    validate_vision_selection_request, ReplaySeekPlan, ReplaySession,
};
use super::super::session_policy::{RoomTimeOperation, RoomTimeSource, SessionPhase};
use super::super::tick_control::{RoomTimeSpeed, TickControl};
use super::super::ReplayBranchSeed;
use super::helpers::DRAINING_NEW_MATCHES_DISABLED_MSG;
use super::types::{LabSeekTarget, Phase, ReplayStartPayloadStamp, ReplayTickContext, RoomMode};
use super::RoomTask;
use crate::protocol::{RoomTimeState, ServerMessage, StartPayload, VisionSelectionRequest};
use rts_sim::game::Game;

impl RoomTask {
    pub(super) fn prompt_for_replay_join(
        &self,
        player_id: u32,
        msg_tx: &ConnectionSink,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        send_or_log(
            &self.room,
            player_id,
            msg_tx,
            ServerMessage::JoinReplayPrompt {
                room: self.room.clone(),
            },
        );
        let _ = ack.send(false);
    }

    pub(super) fn on_join_replay_viewer(
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
        self.order.push(player_id);
        self.players.insert(player_id, replay_viewer(name, msg_tx));
        let _ = ack.send(true);
        self.send_active_replay_state_to(player_id);
    }

    pub(super) fn on_join_replay_lobby(
        &mut self,
        player_id: u32,
        name: String,
        msg_tx: ConnectionSink,
        ack: tokio::sync::oneshot::Sender<bool>,
    ) {
        if self.players.contains_key(&player_id) || !self.is_replay_staging_lobby() {
            let _ = ack.send(false);
            return;
        }
        self.order.push(player_id);
        self.players.insert(player_id, replay_viewer(name, msg_tx));
        self.reassign_host_if_needed();
        let _ = ack.send(true);
        self.send_current_shutdown_warning_to(player_id);
        self.broadcast_lobby();
    }

    pub(super) fn on_join_replay_room(
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
        self.order.push(player_id);
        self.players.insert(player_id, replay_viewer(name, msg_tx));
        let _ = ack.send(true);

        match &self.phase {
            Phase::ReplayViewer(_) => {
                self.send_active_replay_state_to(player_id);
            }
            Phase::Lobby => match self.replay_session_for_mode() {
                Ok(session) => self.transition_to_replay_viewer(session),
                Err(err) => {
                    crate::log_warn!(room = %self.room, error = %err, "replay setup failed");
                    if let Some(player) = self.players.get(&player_id) {
                        send_or_log(
                            &self.room,
                            player_id,
                            &player.msg_tx,
                            ServerMessage::Error { msg: err },
                        );
                    }
                }
            },
            Phase::InGame(_) => {}
            Phase::BranchStaging(_) => {}
        }
    }

    pub(super) fn on_start_replay_lobby_request(&mut self, player_id: u32) {
        if !self.is_replay_staging_lobby() {
            return;
        }
        if self.host_id != Some(player_id) {
            crate::log_debug!(room = %self.room, player_id, "ignoring replay start from non-host");
            return;
        }
        if self.drain.is_draining() {
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
            crate::log_debug!(room = %self.room, player_id, "ignoring replay start while server is draining");
            self.broadcast_lobby();
            return;
        }
        if self.players.is_empty() {
            crate::log_debug!(room = %self.room, "ignoring replay start; no spectators present");
            return;
        }
        match self.replay_session_for_mode() {
            Ok(session) => self.transition_to_replay_viewer(session),
            Err(err) => {
                crate::log_warn!(room = %self.room, error = %err, "replay setup failed");
                if let Some(player) = self.players.get(&player_id) {
                    send_or_log(
                        &self.room,
                        player_id,
                        &player.msg_tx,
                        ServerMessage::Error { msg: err },
                    );
                }
            }
        }
    }

    fn replay_session_for_mode(&self) -> Result<ReplaySession, String> {
        let artifact = match &self.mode {
            RoomMode::Replay { artifact } => artifact.clone(),
            RoomMode::ReplayArtifact { artifact } => load_replay_artifact(artifact)?,
            _ => return Err("room is not configured for replay playback".to_string()),
        };
        ReplaySession::new(artifact)
    }

    fn replay_start_payload_stamp(&self) -> ReplayStartPayloadStamp {
        ReplayStartPayloadStamp {
            policy: self.session_policy(),
            diagnostics: self
                .projection_policy()
                .diagnostic_capabilities_for(RecipientRole::Spectator),
        }
    }

    fn replay_start_payload_for(
        &self,
        session: &ReplaySession,
        watcher_id: u32,
        stamp: ReplayStartPayloadStamp,
    ) -> StartPayload {
        let base_payload = session.game().start_payload();
        let builder = StartPayloadBuilder::replay(
            stamp.policy,
            &base_payload,
            session.start_metadata(),
            session.can_create_replay_branch(),
        );
        builder.build(&StartPayloadRecipient {
            payload_player_id: watcher_id,
            role: RecipientRole::Spectator,
            prediction: LaunchPrediction::Disabled,
            diagnostics: stamp.diagnostics,
            lab: None,
            observer_view: Some(self.observer_view_selection_for(watcher_id)),
        })
    }

    pub(super) fn send_replay_start_to(&self, watcher_id: u32) {
        let Phase::ReplayViewer(session) = &self.phase else {
            return;
        };
        let Some(player) = self.players.get(&watcher_id) else {
            return;
        };
        let payload =
            self.replay_start_payload_for(session, watcher_id, self.replay_start_payload_stamp());
        send_or_log(
            &self.room,
            watcher_id,
            &player.msg_tx,
            ServerMessage::Start(payload),
        );
    }

    fn send_active_replay_state_to(&mut self, watcher_id: u32) {
        self.send_replay_start_to(watcher_id);
        self.send_room_time_state_to(watcher_id);
        self.send_current_replay_snapshot_to(watcher_id);
        self.send_observer_analysis_to(watcher_id);
    }

    fn send_current_replay_snapshot_to(&mut self, watcher_id: u32) {
        let context = ReplayTickContext {
            scheduler_lag: Duration::ZERO,
            tick_budget: self.current_tick_interval(),
            tick_start: StdInstant::now(),
            projection_policy: self.projection_policy_for_phase(SessionPhase::ReplayViewer),
        };
        let session = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::ReplayViewer(session) => session,
            other => {
                self.phase = other;
                return;
            }
        };
        self.clear_pending_snapshots_for([watcher_id]);
        self.fanout_replay_snapshots_to(&session, [watcher_id], HashMap::new(), context, None);
        self.phase = Phase::ReplayViewer(session);
    }

    pub(super) fn send_room_time_state_to(&self, watcher_id: u32) {
        let Phase::ReplayViewer(session) = &self.phase else {
            return;
        };
        let Some(player) = self.players.get(&watcher_id) else {
            return;
        };
        send_or_log(
            &self.room,
            watcher_id,
            &player.msg_tx,
            ServerMessage::RoomTimeState(session.state()),
        );
    }

    pub(super) fn send_observer_analysis_to(&self, watcher_id: u32) {
        let Phase::ReplayViewer(session) = &self.phase else {
            return;
        };
        if self.projection_policy().observer_analysis_audience()
            != ObserverAnalysisAudience::AllRecipients
        {
            return;
        }
        self.send_scoped_replay_observer_analysis(session, [watcher_id]);
    }

    fn broadcast_room_time_state_for(&self, session: &ReplaySession) {
        let msg = ServerMessage::RoomTimeState(session.state());
        self.broadcast(&msg);
    }

    fn broadcast_observer_analysis_for(
        &self,
        session: &ReplaySession,
        projection_policy: ProjectionPolicy,
    ) {
        if projection_policy.observer_analysis_audience() != ObserverAnalysisAudience::AllRecipients
        {
            return;
        }
        self.send_scoped_replay_observer_analysis(session, self.order.clone());
    }

    pub(super) fn room_time_state_for_live_game(
        &self,
        game: &Game,
        controller_id: Option<u32>,
    ) -> RoomTimeState {
        RoomTimeState {
            current_tick: game.tick_count(),
            duration_ticks: 0,
            keyframe_ticks: Vec::new(),
            speed: if self.room_time_paused {
                0.0
            } else {
                self.room_time_speed
            },
            paused: self.room_time_paused,
            ended: false,
            controller_id,
        }
    }

    fn clear_pending_snapshots_for(&self, recipients: impl IntoIterator<Item = u32>) {
        for player in recipients
            .into_iter()
            .filter_map(|id| self.players.get(&id))
        {
            player.msg_tx.clear_pending_snapshot();
        }
    }

    pub(super) fn on_tick_replay_viewer(&mut self, scheduled: TokioInstant) {
        let context = ReplayTickContext {
            scheduler_lag: scheduled.elapsed(),
            tick_budget: self.current_tick_interval(),
            tick_start: StdInstant::now(),
            projection_policy: self.projection_policy_for_phase(SessionPhase::ReplayViewer),
        };
        let mut session = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::ReplayViewer(session) => session,
            other => {
                self.phase = other;
                return;
            }
        };
        let mut perf = rts_sim::perf::TickPerf::maybe_new();

        if session.has_remaining_ticks() {
            if let Err(err) = session.enqueue_for_current_tick() {
                crate::log_warn!(room = %self.room, error = %err, "replay command enqueue failed");
                self.send_dev_error(&err);
                self.phase = Phase::ReplayViewer(session);
                return;
            }
            let game_tick_start = StdInstant::now();
            let tick_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                session.tick(perf.as_mut())
            }));
            if let Some(perf) = perf.as_mut() {
                perf.record_phase("game_tick", game_tick_start.elapsed());
            }
            let per_player_events = match tick_result {
                Ok(events) => events,
                Err(payload) => {
                    let reason = panic_reason(&payload);
                    dump_crash_replay_artifact(
                        &self.room,
                        session.game().tick_count(),
                        &session.artifact,
                        &reason,
                    );
                    self.send_dev_error("Replay playback failed");
                    self.phase = Phase::Lobby;
                    return;
                }
            };
            session.record_keyframe_if_due();
            let recipients = self.order.clone();
            self.fanout_replay_snapshots_to(
                &session,
                recipients,
                per_player_events,
                context,
                perf.as_mut(),
            );
            self.broadcast_observer_analysis_for(&session, context.projection_policy);
        } else {
            self.broadcast_room_time_state_for(&session);
            self.broadcast_observer_analysis_for(&session, context.projection_policy);
        }

        self.finish_perf_tick(
            perf.as_ref(),
            session.game(),
            context.scheduler_lag,
            context.tick_start,
        );
        self.phase = Phase::ReplayViewer(session);
    }

    pub(in crate::lobby) fn on_set_room_time_speed(&mut self, player_id: u32, speed: f32) {
        if !self.tick_control().allows_room_time_operation(
            RoomTimeOperation::SetSpeed,
            self.players.contains_key(&player_id),
        ) {
            return;
        }

        match self.session_policy().clock.room_time_source() {
            Some(RoomTimeSource::ReplayPlayback) => {}
            Some(RoomTimeSource::DevScenario) => {
                self.apply_room_time_speed(speed);
                self.broadcast_dev_watch_state();
                return;
            }
            Some(RoomTimeSource::Lab) => {
                if !self.lab_room_time_control_allowed(player_id) {
                    return;
                }
                self.apply_room_time_speed(speed);
                self.lab_room_time_controller_id = Some(player_id);
                self.broadcast_lab_room_time_state();
                return;
            }
            Some(RoomTimeSource::LiveGame) => {
                self.apply_room_time_speed(speed);
                self.broadcast_live_ai_room_time_state();
                return;
            }
            None => return,
        }

        if let Phase::ReplayViewer(session) = &mut self.phase {
            session.set_speed(player_id, speed);
            let state = session.state();
            self.broadcast(&ServerMessage::RoomTimeState(state));
        }
    }

    pub(super) fn on_step_room_time(&mut self, player_id: u32) {
        if !self
            .tick_control()
            .can_step_room_time(self.players.contains_key(&player_id))
        {
            return;
        }
        match self.session_policy().clock.room_time_source() {
            Some(RoomTimeSource::DevScenario) => {
                self.on_tick_dev_watch(TokioInstant::now());
                self.broadcast_dev_watch_state();
            }
            Some(RoomTimeSource::Lab) => {
                if !self.lab_room_time_control_allowed(player_id) {
                    return;
                }
                self.lab_room_time_controller_id = Some(player_id);
                self.on_tick_live_game(TokioInstant::now());
                self.broadcast_lab_room_time_state();
            }
            Some(RoomTimeSource::ReplayPlayback) | Some(RoomTimeSource::LiveGame) | None => {}
        }
    }

    fn apply_room_time_speed(&mut self, speed: f32) {
        match TickControl::room_time_speed(speed) {
            RoomTimeSpeed::Paused => {
                self.room_time_paused = true;
            }
            RoomTimeSpeed::Running(speed) => {
                self.room_time_paused = false;
                self.room_time_speed = speed;
            }
        }
    }

    pub(super) fn on_set_vision_selection(
        &mut self,
        player_id: u32,
        selection: VisionSelectionRequest,
    ) {
        let may_select_vision = match &self.phase {
            Phase::ReplayViewer(_) => self.players.contains_key(&player_id),
            Phase::InGame(_) => self
                .players
                .get(&player_id)
                .is_some_and(|player| player.spectator),
            _ => false,
        };
        if !may_select_vision {
            return;
        }
        if let Phase::InGame(game) = &self.phase {
            let valid_ids: Vec<u32> = game.player_inits().iter().map(|player| player.id).collect();
            if validate_vision_selection_request(&selection, &valid_ids).is_err() {
                self.send_error_to(player_id, "Invalid vision selection");
                return;
            }
            let view = observer_view_from_selection(selection);
            self.observer_views.insert(player_id, view);
            self.clear_pending_snapshots_for([player_id]);
            self.fanout_current_observer_snapshots_to([player_id]);
            return;
        }
        let context = ReplayTickContext {
            scheduler_lag: Duration::ZERO,
            tick_budget: self.current_tick_interval(),
            tick_start: StdInstant::now(),
            projection_policy: self.projection_policy_for_phase(SessionPhase::ReplayViewer),
        };
        let send_analysis = context.projection_policy.observer_analysis_audience()
            == ObserverAnalysisAudience::AllRecipients;
        let session = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::ReplayViewer(session) => session,
            other => {
                self.phase = other;
                return;
            }
        };

        if !self.players.contains_key(&player_id) {
            self.phase = Phase::ReplayViewer(session);
            return;
        }
        let valid_ids = session.active_player_ids();
        if validate_vision_selection_request(&selection, &valid_ids).is_err() {
            self.send_error_to(player_id, "Invalid vision selection");
            self.phase = Phase::ReplayViewer(session);
            return;
        }

        let view = observer_view_from_selection(selection);
        self.observer_views.insert(player_id, view.clone());
        self.clear_pending_snapshots_for([player_id]);
        self.fanout_replay_snapshots_to(&session, [player_id], HashMap::new(), context, None);
        let analysis = send_analysis
            .then(|| scope_observer_analysis(session.game().observer_analysis(), &view));
        if let (Some(analysis), Some(player)) = (analysis, self.players.get(&player_id)) {
            send_or_log(
                &self.room,
                player_id,
                &player.msg_tx,
                ServerMessage::ObserverAnalysis(analysis),
            );
        }
        self.phase = Phase::ReplayViewer(session);
    }

    pub(super) fn on_request_branch_from_tick(
        &self,
        player_id: u32,
    ) -> Result<ReplayBranchSeed, String> {
        if !self.players.contains_key(&player_id) {
            return Err("Cannot branch replay: viewer is not in this room.".to_string());
        }
        let Phase::ReplayViewer(session) = &self.phase else {
            return Err("Cannot branch replay outside replay playback.".to_string());
        };
        session.branch_seed()
    }

    /// Rewind room-controlled time by `ticks_back` ticks. Pass `u32::MAX` to reset to the start.
    /// No-op outside rooms whose clock capability allows relative seek.
    pub(super) fn on_seek_room_time(&mut self, player_id: u32, ticks_back: u32) {
        if !self.tick_control().allows_room_time_operation(
            RoomTimeOperation::SeekRelative,
            self.players.contains_key(&player_id),
        ) {
            return;
        }
        match self.session_policy().clock.room_time_source() {
            Some(RoomTimeSource::Lab) => {
                self.on_seek_lab_room_time(player_id, LabSeekTarget::Relative(ticks_back));
                return;
            }
            Some(RoomTimeSource::ReplayPlayback) => {}
            Some(RoomTimeSource::DevScenario) | Some(RoomTimeSource::LiveGame) | None => return,
        }
        let send_analysis = self.projection_policy().observer_analysis_audience()
            == ObserverAnalysisAudience::AllRecipients;
        self.on_seek_replay_room_time(player_id, send_analysis, |session| {
            session.plan_seek_back(ticks_back)
        });
    }

    /// Seek room-controlled time to an absolute tick. No-op outside rooms whose clock capability
    /// allows absolute seek.
    pub(super) fn on_seek_room_time_to(&mut self, player_id: u32, tick: u32) {
        if !self.tick_control().allows_room_time_operation(
            RoomTimeOperation::SeekAbsolute,
            self.players.contains_key(&player_id),
        ) {
            return;
        }
        match self.session_policy().clock.room_time_source() {
            Some(RoomTimeSource::Lab) => {
                self.on_seek_lab_room_time(player_id, LabSeekTarget::Absolute(tick));
                return;
            }
            Some(RoomTimeSource::ReplayPlayback) => {}
            Some(RoomTimeSource::DevScenario) | Some(RoomTimeSource::LiveGame) | None => return,
        }
        let send_analysis = self.projection_policy().observer_analysis_audience()
            == ObserverAnalysisAudience::AllRecipients;
        self.on_seek_replay_room_time(player_id, send_analysis, |session| {
            session.plan_seek_to(tick)
        });
    }

    fn on_seek_replay_room_time(
        &mut self,
        player_id: u32,
        send_analysis: bool,
        plan_seek: impl FnOnce(&ReplaySession) -> Result<ReplaySeekPlan, String>,
    ) {
        let context = ReplayTickContext {
            scheduler_lag: Duration::ZERO,
            tick_budget: self.current_tick_interval(),
            tick_start: StdInstant::now(),
            projection_policy: self.projection_policy_for_phase(SessionPhase::ReplayViewer),
        };
        let start_stamp = self.replay_start_payload_stamp();
        let mut session = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::ReplayViewer(session) => session,
            other => {
                self.phase = other;
                return;
            }
        };

        let viewer_count = self.players.len();
        let seek_result = plan_seek(&session).and_then(|plan| {
            self.broadcast(&ServerMessage::RoomTimeSeekStarted {
                controller_id: player_id,
                from_tick: plan.from_tick,
                target_tick: plan.target_tick,
            });
            session.apply_seek(&self.room, viewer_count, player_id, plan)
        });
        match seek_result {
            Ok(_) => {
                let starts = self
                    .order
                    .iter()
                    .filter_map(|viewer_id| {
                        self.players.get(viewer_id).map(|player| {
                            let start =
                                self.replay_start_payload_for(&session, *viewer_id, start_stamp);
                            (*viewer_id, player.msg_tx.clone(), start)
                        })
                    })
                    .collect::<Vec<_>>();
                let recipients = starts
                    .iter()
                    .map(|(viewer_id, _, _)| *viewer_id)
                    .collect::<Vec<_>>();
                let state = session.state();

                self.clear_pending_snapshots_for(recipients.iter().copied());
                for (viewer_id, msg_tx, start) in starts {
                    send_or_log(&self.room, viewer_id, &msg_tx, ServerMessage::Start(start));
                }
                self.broadcast(&ServerMessage::RoomTimeState(state));
                self.fanout_replay_snapshots_to(
                    &session,
                    recipients,
                    HashMap::new(),
                    context,
                    None,
                );
                if send_analysis {
                    self.broadcast_observer_analysis_for(&session, context.projection_policy);
                }
            }
            Err(err) => {
                crate::log_warn!(room = %self.room, error = %err, "replay seek failed");
                self.send_dev_error(&err);
            }
        }

        self.phase = Phase::ReplayViewer(session);
    }

    pub(super) fn transition_to_replay_viewer(&mut self, session: ReplaySession) {
        self.phase = Phase::ReplayViewer(Box::new(session));
        self.reset_after_live_match_for_room_phase();
        let recipients = self.order.clone();
        for id in recipients {
            self.send_replay_start_to(id);
            self.send_room_time_state_to(id);
            self.send_observer_analysis_to(id);
        }
        crate::log_info!(
            room = %self.room,
            viewer_count = self.players.len(),
            "replay viewer active"
        );
    }

    pub(super) fn on_return_to_lobby(&mut self, player_id: u32) {
        if self.players.contains_key(&player_id) && matches!(self.phase, Phase::ReplayViewer(_)) {
            self.on_leave(player_id);
        }
    }
}
