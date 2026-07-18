use super::super::connection::ConnectionSink;
use super::super::launch::{LaunchPrediction, LaunchRecipient, StartPayloadBuilder};
use super::super::projection::RecipientRole;
use super::super::replay_branch::{BranchLaunchError, BranchStagingState};
use super::super::session_policy::{SessionPhase, SessionPolicy};
use super::helpers::DRAINING_NEW_MATCHES_DISABLED_MSG;
use super::types::{Phase, RoomMode, RoomPlayer};
use super::RoomTask;
use crate::protocol::{BranchStagingOccupant, ReplayBranchSeat, ServerMessage};
use crate::structured_log::{self, MatchStartedLog};

impl RoomTask {
    pub(super) fn on_join_branch_staging(
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

    pub(super) fn on_join_branch_live(
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

    pub(super) fn start_branch_live(&mut self) {
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
        self.capture_replay_start_for(&game);
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
            let mapped_seat = self
                .branch_live_seat_by_connection
                .get(&connection_id)
                .copied();
            let observer_view = mapped_seat
                .is_none()
                .then(|| self.observer_view_selection_for(connection_id));
            let Some(player) = self.players.get_mut(&connection_id) else {
                continue;
            };
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
                observer_view,
                msg_tx: player.msg_tx.clone(),
            });
        }
        let builder = StartPayloadBuilder::simulation(start_policy, &payload);
        super::super::launch::send_start_payloads(&self.room, &builder, recipients);

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

    pub(super) fn on_claim_branch_seat(&mut self, player_id: u32, seat_player_id: u32) {
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

    pub(super) fn on_release_branch_seat(&mut self, player_id: u32, seat_player_id: u32) {
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

    pub(super) fn on_start_branch(&mut self, player_id: u32) {
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

    pub(super) fn on_announce_branch_from_tick(
        &self,
        branch_room: String,
        source_tick: u32,
        seats: Vec<ReplayBranchSeat>,
    ) {
        if !matches!(self.phase, Phase::ReplayViewer(_)) {
            return;
        }
        self.broadcast(&ServerMessage::BranchFromTickCreated {
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

    pub(super) fn broadcast_branch_staging(&self) {
        let Some(staging) = self.branch_staging() else {
            return;
        };
        self.broadcast(&self.branch_staging_message(staging));
    }
}
