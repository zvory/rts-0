use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::time::{Duration, Instant as StdInstant};

use rts_sim::game::entity::EntityKind;
use rts_sim::game::lab::{
    LabError, LabMoveEntity, LabOp, LabOpOutcome, LabScenarioV1 as SimLabScenarioV1,
    LabSetCompletedResearch, LabSetEntityOwner, LabSetPlayerResources, LabSpawnEntity,
};
use rts_sim::game::map::Map;
use rts_sim::game::upgrade::UpgradeKind;

use super::super::connection::{send_or_log, ConnectionSink};
use super::super::dev_replay::match_seed;
use super::super::lab_timeline::{LabTimeline, LabTimelineEntry, LabTimelineEntryKind};
use super::super::launch::{LaunchPrediction, LaunchRecipient, StartPayloadBuilder};
use super::super::live_tick::LabSnapshotProjection;
use super::super::projection::RecipientRole;
use super::super::session_policy::{RoomTimeSource, SessionPhase, SessionPolicy};
use super::super::snapshot_fanout::{SnapshotFanout, SnapshotFanoutPayload};
use super::super::{normalize_start_team_id, PLAYER_PALETTE};
use super::helpers::{DRAINING_NEW_MATCHES_DISABLED_MSG, LAB_PLAYER_ONE_ID, LAB_PLAYER_TWO_ID};
use super::types::{LabRoomConfig, LabSeekTarget, Phase, RoomMode, RoomPlayer};
use super::RoomTask;
use crate::protocol::{
    Command, Event, LabClientOp, LabResult, LabScenarioLabMetadata, LabStartMetadata, LabStartRole,
    LabState, LabVisionMode, RoomTimeState, ServerMessage, TeamId, DEFAULT_FACTION_ID,
};
use crate::structured_log::{self, MatchStartedLog};
use rts_sim::game::{Game, PlayerInit};

pub(super) struct LabSession {
    pub(super) public_id: String,
    pub(super) operator_id: u32,
    pub(super) viewer_roles: HashMap<u32, LabStartRole>,
    pub(super) viewer_visions: HashMap<u32, LabVisionMode>,
    pub(super) default_vision: LabVisionMode,
    pub(super) dirty: bool,
    pub(super) operation_log: Vec<LabOperationLogEntry>,
    pub(super) view_player_id: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(super) struct LabOperationLogEntry {
    pub(super) tick: u32,
    pub(super) request_id: u32,
    pub(super) operator_id: u32,
    pub(super) op: String,
    pub(super) result: String,
}

impl LabSession {
    pub(super) fn new(config: &LabRoomConfig, operator_id: u32) -> Self {
        let mut viewer_roles = HashMap::new();
        viewer_roles.insert(operator_id, LabStartRole::Operator);
        let default_vision = LabVisionMode::FullWorld;
        let mut viewer_visions = HashMap::new();
        viewer_visions.insert(operator_id, default_vision.clone());
        Self {
            public_id: config.public_id.clone(),
            operator_id,
            viewer_roles,
            viewer_visions,
            default_vision,
            dirty: false,
            operation_log: Vec::new(),
            view_player_id: LAB_PLAYER_ONE_ID,
        }
    }

    pub(super) fn add_viewer(&mut self, player_id: u32) {
        self.viewer_roles.insert(player_id, LabStartRole::Operator);
        self.viewer_visions
            .insert(player_id, self.default_vision.clone());
    }

    pub(super) fn remove_viewer(&mut self, player_id: u32) {
        self.viewer_roles.remove(&player_id);
        self.viewer_visions.remove(&player_id);
    }

    pub(super) fn role_for(&self, player_id: u32) -> LabStartRole {
        self.viewer_roles
            .get(&player_id)
            .copied()
            .unwrap_or(LabStartRole::ReadOnly)
    }

    pub(super) fn can_operate(&self, player_id: u32) -> bool {
        matches!(self.role_for(player_id), LabStartRole::Operator)
    }

    pub(super) fn vision_for(&self, player_id: u32) -> LabVisionMode {
        self.viewer_visions
            .get(&player_id)
            .cloned()
            .unwrap_or_else(|| self.default_vision.clone())
    }

    pub(super) fn set_vision_for(&mut self, player_id: u32, vision: LabVisionMode) {
        self.viewer_visions.insert(player_id, vision);
    }

    pub(super) fn import_vision_for(&mut self, player_id: u32, vision: LabVisionMode) {
        self.default_vision = vision.clone();
        self.set_vision_for(player_id, vision);
    }

    pub(super) fn metadata_for(&self, player_id: u32) -> LabStartMetadata {
        LabStartMetadata {
            room: self.public_id.clone(),
            operator_id: self.operator_id,
            role: self.role_for(player_id),
            vision: self.vision_for(player_id),
            dirty: self.dirty,
            operation_count: self.operation_log.len() as u32,
        }
    }

    pub(super) fn state_for(&self, player_id: u32) -> LabState {
        LabState {
            room: self.public_id.clone(),
            operator_id: self.operator_id,
            role: self.role_for(player_id),
            vision: self.vision_for(player_id),
            dirty: self.dirty,
            operation_count: self.operation_log.len() as u32,
        }
    }
}

fn players_on_teams(game: &Game, team_ids: impl IntoIterator<Item = TeamId>) -> Vec<u32> {
    let teams: HashSet<_> = team_ids.into_iter().collect();
    game.start_payload()
        .players
        .into_iter()
        .filter(|player| teams.contains(&player.team_id))
        .map(|player| player.id)
        .collect()
}

fn lab_op_kind(op: &LabClientOp) -> &'static str {
    match op {
        LabClientOp::ExportScenario { .. } => "exportScenario",
        LabClientOp::ImportScenario { .. } => "importScenario",
        LabClientOp::SpawnEntity { .. } => "spawnEntity",
        LabClientOp::DeleteEntity { .. } => "deleteEntity",
        LabClientOp::MoveEntity { .. } => "moveEntity",
        LabClientOp::SetEntityOwner { .. } => "setEntityOwner",
        LabClientOp::SetPlayerResources { .. } => "setPlayerResources",
        LabClientOp::SetCompletedResearch { .. } => "setCompletedResearch",
        LabClientOp::SetVision { .. } => "setVision",
        LabClientOp::IssueCommandAs { .. } => "issueCommandAs",
    }
}

fn lab_client_op_to_game_op(op: LabClientOp) -> Result<LabOp, String> {
    match op {
        LabClientOp::ImportScenario { scenario } => {
            validate_lab_scenario_vision(&scenario.metadata.lab.vision, &scenario.players)?;
            let scenario: SimLabScenarioV1 = serde_json::from_value(
                serde_json::to_value(scenario)
                    .map_err(|err| format!("invalid scenario payload: {err}"))?,
            )
            .map_err(|err| format!("invalid scenario payload: {err}"))?;
            Ok(LabOp::RestoreScenario(Box::new(scenario)))
        }
        LabClientOp::SpawnEntity {
            owner,
            kind,
            x,
            y,
            completed,
        } => {
            let kind =
                EntityKind::from_str(&kind).map_err(|_| "unknown entity kind".to_string())?;
            Ok(LabOp::SpawnEntity(LabSpawnEntity {
                owner,
                kind,
                x,
                y,
                completed,
            }))
        }
        LabClientOp::DeleteEntity { entity_id } => Ok(LabOp::DeleteEntity { entity_id }),
        LabClientOp::MoveEntity { entity_id, x, y } => {
            Ok(LabOp::MoveEntity(LabMoveEntity { entity_id, x, y }))
        }
        LabClientOp::SetEntityOwner { entity_id, owner } => {
            Ok(LabOp::SetEntityOwner(LabSetEntityOwner {
                entity_id,
                owner,
            }))
        }
        LabClientOp::SetPlayerResources {
            player_id,
            steel,
            oil,
        } => Ok(LabOp::SetPlayerResources(LabSetPlayerResources {
            player_id,
            steel,
            oil,
        })),
        LabClientOp::SetCompletedResearch {
            player_id,
            upgrade,
            completed,
        } => {
            let upgrade =
                UpgradeKind::from_str(&upgrade).map_err(|_| "unknown research id".to_string())?;
            Ok(LabOp::SetCompletedResearch(LabSetCompletedResearch {
                player_id,
                upgrade,
                completed,
            }))
        }
        LabClientOp::ExportScenario { .. }
        | LabClientOp::SetVision { .. }
        | LabClientOp::IssueCommandAs { .. } => Err("not a lab mutation".to_string()),
    }
}

fn validate_lab_vision(game: &Game, vision: &LabVisionMode) -> Result<(), String> {
    let players = game.start_payload().players;
    match vision {
        LabVisionMode::FullWorld => Ok(()),
        LabVisionMode::Team { team_id } => {
            if players.iter().any(|player| player.team_id == *team_id) {
                Ok(())
            } else {
                Err("unknown lab team id".to_string())
            }
        }
        LabVisionMode::Teams { team_ids } => {
            if team_ids.is_empty() {
                return Err("teamIds must not be empty".to_string());
            }
            let mut seen = HashSet::new();
            for team_id in team_ids {
                if !seen.insert(*team_id) {
                    return Err("teamIds must not contain duplicates".to_string());
                }
                if !players.iter().any(|player| player.team_id == *team_id) {
                    return Err("unknown lab team id".to_string());
                }
            }
            Ok(())
        }
    }
}

fn truncate_lab_scenario_name(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if out.len() + ch.len_utf8() > 80 {
            break;
        }
        out.push(ch);
    }
    out
}

fn validate_lab_scenario_vision(
    vision: &LabVisionMode,
    players: &[crate::protocol::LabScenarioPlayer],
) -> Result<(), String> {
    match vision {
        LabVisionMode::FullWorld => Ok(()),
        LabVisionMode::Team { team_id } => {
            if players.iter().any(|player| player.team_id == *team_id) {
                Ok(())
            } else {
                Err("unknown scenario lab team id".to_string())
            }
        }
        LabVisionMode::Teams { team_ids } => {
            if team_ids.is_empty() {
                return Err("teamIds must not be empty".to_string());
            }
            let mut seen = HashSet::new();
            for team_id in team_ids {
                if !seen.insert(*team_id) {
                    return Err("teamIds must not contain duplicates".to_string());
                }
                if !players.iter().any(|player| player.team_id == *team_id) {
                    return Err("unknown scenario lab team id".to_string());
                }
            }
            Ok(())
        }
    }
}

fn lab_result_error(request_id: u32, op: String, error: &str) -> LabResult {
    LabResult {
        request_id,
        ok: false,
        op,
        error: Some(error.to_string()),
        outcome: None,
    }
}

fn lab_error_text(err: &LabError) -> String {
    match err {
        LabError::StaleEntity { entity_id } => format!("stale entity id {entity_id}"),
        LabError::InvalidKind { kind, operation } => {
            format!("invalid kind {kind:?} for {operation}")
        }
        LabError::InvalidPlayer { player_id } => format!("invalid player id {player_id}"),
        LabError::InvalidOwner { owner } => format!("invalid owner id {owner}"),
        LabError::InvalidPosition { x, y, reason } => {
            format!("invalid position ({x}, {y}): {reason}")
        }
        LabError::OccupiedPosition { x, y } => format!("occupied position ({x}, {y})"),
        LabError::InvalidResearch { player_id, upgrade } => {
            format!("invalid research {upgrade:?} for player {player_id}")
        }
        LabError::InvalidScenarioVersion { version } => {
            format!("unsupported scenario version {version}")
        }
        LabError::InvalidScenario { reason } => reason.clone(),
        LabError::InvalidMap { name, reason } => format!("invalid map {name:?}: {reason}"),
        LabError::InvalidCommand { reason } => reason.clone(),
    }
}

fn lab_outcome_json(outcome: &LabOpOutcome) -> serde_json::Value {
    match outcome {
        LabOpOutcome::Spawned { entity_id } => serde_json::json!({ "entityId": entity_id }),
        LabOpOutcome::Deleted { entity_id } => serde_json::json!({ "entityId": entity_id }),
        LabOpOutcome::Moved { entity_id, x, y } => {
            serde_json::json!({ "entityId": entity_id, "x": x, "y": y })
        }
        LabOpOutcome::OwnerSet { entity_id, owner } => {
            serde_json::json!({ "entityId": entity_id, "owner": owner })
        }
        LabOpOutcome::PlayerResourcesSet {
            player_id,
            steel,
            oil,
        } => serde_json::json!({ "playerId": player_id, "steel": steel, "oil": oil }),
        LabOpOutcome::CompletedResearchSet {
            player_id,
            upgrade,
            completed,
        } => serde_json::json!({
            "playerId": player_id,
            "upgrade": upgrade.to_protocol_str(),
            "completed": completed
        }),
        LabOpOutcome::ScenarioRestored(restore) => serde_json::to_value(restore)
            .unwrap_or_else(|_| serde_json::json!({ "scenarioRestored": true })),
    }
}

impl RoomTask {
    pub(super) fn on_join_lab(
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
        if !matches!(self.phase, Phase::Lobby | Phase::InGame(_)) {
            let _ = ack.send(false);
            return;
        }
        let config = match &self.mode {
            RoomMode::Lab(config) => config.clone(),
            _ => {
                let _ = ack.send(false);
                return;
            }
        };
        if matches!(self.phase, Phase::Lobby) && self.new_live_session_blocked_by_drain() {
            send_or_log(
                &self.room,
                player_id,
                &msg_tx,
                ServerMessage::Error {
                    msg: DRAINING_NEW_MATCHES_DISABLED_MSG.to_string(),
                },
            );
            crate::log_debug!(
                room = %self.room,
                player_id,
                "rejecting lab join; launch blocked while server is draining"
            );
            let _ = ack.send(false);
            return;
        }
        if self.lab_session.is_none() {
            self.lab_session = Some(LabSession::new(&config, player_id));
        } else if let Some(session) = &mut self.lab_session {
            session.add_viewer(player_id);
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

        if matches!(self.phase, Phase::Lobby) {
            self.start_lab_session();
        } else {
            self.send_lab_start_to(player_id);
            self.send_lab_room_time_state_to(player_id);
        }
    }
    pub(super) fn start_lab_session(&mut self) {
        self.prepare_live_match_launch();
        let config = match &self.mode {
            RoomMode::Lab(config) => config.clone(),
            _ => return,
        };
        if self.lab_session.is_none() {
            if let Some(operator_id) = self.order.first().copied() {
                self.lab_session = Some(LabSession::new(&config, operator_id));
            }
        }
        let seed = config.seed.unwrap_or_else(match_seed);
        let inits = self.default_lab_player_template();
        let start_players: Vec<_> = inits
            .iter()
            .map(|player| {
                (
                    player.id,
                    normalize_start_team_id(player.id, player.team_id),
                )
            })
            .collect();
        let map_metadata = match Map::metadata_for_name(&config.map_name) {
            Ok(metadata) => metadata,
            Err(err) => {
                self.send_lab_error(format!(
                    "Cannot load lab map \"{}\": {err}",
                    config.map_name
                ));
                return;
            }
        };
        let map = match Map::load_for_players(&config.map_name, &start_players, seed) {
            Ok(map) => map,
            Err(err) => {
                self.send_lab_error(format!(
                    "Cannot load lab map \"{}\": {err}",
                    config.map_name
                ));
                return;
            }
        };
        let game =
            Game::new_with_random_ai_profiles_and_map_metadata(&inits, seed, map, map_metadata);
        self.record_live_match_started(
            inits.len(),
            0,
            config.map_name.clone(),
            inits.iter().map(|player| player.name.clone()).collect(),
        );
        let mut payload = game.start_payload();
        payload.match_run_id = self.match_run_id.clone();
        self.ai_controllers.clear();
        self.dev_driver = None;
        self.dev_view_player_id = None;

        let projection_policy = self.projection_policy_for_phase(SessionPhase::LiveMatch);
        let start_policy = SessionPolicy::for_room(&self.mode, SessionPhase::LiveMatch);
        let recipients: Vec<_> = self
            .order
            .iter()
            .filter_map(|&id| {
                self.players.get(&id).map(|player| LaunchRecipient {
                    connection_id: id,
                    payload_player_id: self
                        .lab_session
                        .as_ref()
                        .map(|session| session.view_player_id)
                        .unwrap_or(LAB_PLAYER_ONE_ID),
                    role: RecipientRole::Spectator,
                    prediction: LaunchPrediction::Disabled,
                    diagnostics: projection_policy
                        .diagnostic_capabilities_for(RecipientRole::Spectator),
                    clear_pending_snapshot: false,
                    lab: self.lab_start_metadata_for(id),
                    msg_tx: player.msg_tx.clone(),
                })
            })
            .collect();
        let builder = StartPayloadBuilder::simulation(start_policy, &payload);
        super::super::launch::send_start_payloads(&self.room, &builder, recipients);

        structured_log::log_match_started(MatchStartedLog {
            room: &self.room,
            match_run_id: self.match_run_id.as_deref().unwrap_or(""),
            mode: "lab",
            map: &self.match_map_name,
            seed,
            players: self.match_player_count,
            humans: self.match_human_count,
            ai: 0,
            participants: &self.match_participants,
        });
        self.mark_match_started_for_drain();
        self.lab_timeline = Some(LabTimeline::new(&game));
        self.phase = Phase::InGame(Box::new(game));
        self.broadcast_lab_room_time_state();
    }

    fn default_lab_player_template(&self) -> Vec<PlayerInit> {
        vec![
            PlayerInit {
                id: LAB_PLAYER_ONE_ID,
                team_id: 1,
                faction_id: DEFAULT_FACTION_ID.to_string(),
                name: "Lab Alpha".to_string(),
                color: PLAYER_PALETTE[0].to_string(),
                is_ai: false,
            },
            PlayerInit {
                id: LAB_PLAYER_TWO_ID,
                team_id: 2,
                faction_id: DEFAULT_FACTION_ID.to_string(),
                name: "Lab Bravo".to_string(),
                color: PLAYER_PALETTE[1].to_string(),
                is_ai: false,
            },
        ]
    }

    fn send_lab_error(&self, msg: String) {
        let error = ServerMessage::Error { msg };
        self.broadcast(&error);
    }

    pub(super) fn send_lab_start_to(&self, watcher_id: u32) {
        let Some(Phase::InGame(game)) = Some(&self.phase) else {
            return;
        };
        let Some(player) = self.players.get(&watcher_id) else {
            return;
        };
        let payload = game.start_payload();
        let diagnostics = self
            .projection_policy()
            .diagnostic_capabilities_for(RecipientRole::Spectator);
        let start_policy = self.session_policy();
        let builder = StartPayloadBuilder::simulation(start_policy, &payload);
        super::super::launch::send_start_payloads(
            &self.room,
            &builder,
            [LaunchRecipient {
                connection_id: watcher_id,
                payload_player_id: self
                    .lab_session
                    .as_ref()
                    .map(|session| session.view_player_id)
                    .unwrap_or(LAB_PLAYER_ONE_ID),
                prediction: LaunchPrediction::Disabled,
                role: RecipientRole::Spectator,
                diagnostics,
                clear_pending_snapshot: false,
                lab: self.lab_start_metadata_for(watcher_id),
                msg_tx: player.msg_tx.clone(),
            }],
        );
    }

    fn send_lab_start_payloads_to_all(&self, clear_pending_snapshot: bool) {
        let Phase::InGame(game) = &self.phase else {
            return;
        };
        let payload = game.start_payload();
        let diagnostics = self
            .projection_policy()
            .diagnostic_capabilities_for(RecipientRole::Spectator);
        let start_policy = self.session_policy();
        let recipients = self.order.iter().filter_map(|&id| {
            self.players.get(&id).map(|player| LaunchRecipient {
                connection_id: id,
                payload_player_id: self
                    .lab_session
                    .as_ref()
                    .map(|session| session.view_player_id)
                    .unwrap_or(LAB_PLAYER_ONE_ID),
                prediction: LaunchPrediction::Disabled,
                role: RecipientRole::Spectator,
                diagnostics,
                clear_pending_snapshot,
                lab: self.lab_start_metadata_for(id),
                msg_tx: player.msg_tx.clone(),
            })
        });
        let builder = StartPayloadBuilder::simulation(start_policy, &payload);
        super::super::launch::send_start_payloads(&self.room, &builder, recipients);
    }

    fn lab_start_metadata_for(&self, player_id: u32) -> Option<LabStartMetadata> {
        self.lab_session
            .as_ref()
            .map(|session| session.metadata_for(player_id))
    }

    pub(super) fn lab_snapshot_projections(
        &self,
        game: &Game,
    ) -> HashMap<u32, LabSnapshotProjection> {
        let Some(session) = &self.lab_session else {
            return HashMap::new();
        };
        let mut projections = HashMap::new();
        for &id in &self.order {
            if !self.players.contains_key(&id) {
                continue;
            }
            let projection = match session.vision_for(id) {
                LabVisionMode::FullWorld => LabSnapshotProjection::FullWorld {
                    view_player_id: session.view_player_id,
                },
                LabVisionMode::Team { team_id } => LabSnapshotProjection::PlayerUnion {
                    player_ids: players_on_teams(game, std::iter::once(team_id)),
                },
                LabVisionMode::Teams { team_ids } => LabSnapshotProjection::PlayerUnion {
                    player_ids: players_on_teams(game, team_ids),
                },
            };
            projections.insert(id, projection);
        }
        projections
    }

    fn fanout_current_lab_snapshots(&mut self) {
        if self.session_policy().clock.room_time_source() != Some(RoomTimeSource::Lab) {
            return;
        }
        let projection_policy = self.projection_policy();
        let tick_budget = self.current_tick_interval();
        let tick_start = StdInstant::now();
        let game = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::InGame(game) => game,
            other => {
                self.phase = other;
                return;
            }
        };
        let mut per_player_events: HashMap<u32, Vec<Event>> = HashMap::new();
        let lab_snapshot_projections = self.lab_snapshot_projections(&game);
        let recipients = self.order.clone();
        SnapshotFanout::new(
            &self.room,
            Duration::ZERO,
            tick_budget,
            tick_start,
            &mut self.slow_tick_count,
            None,
        )
        .send_to_recipients(&mut self.players, recipients, |id, player| {
            let projection = match lab_snapshot_projections.get(&id) {
                Some(LabSnapshotProjection::FullWorld { view_player_id }) => projection_policy
                    .live_snapshot_for(RecipientRole::Spectator, id, Some(*view_player_id), &[]),
                Some(LabSnapshotProjection::PlayerUnion { player_ids }) => {
                    projection_policy.replay_snapshot_for(player_ids.clone())
                }
                None => projection_policy.live_snapshot_for(
                    RecipientRole::Spectator,
                    id,
                    Some(LAB_PLAYER_ONE_ID),
                    &[],
                ),
            };
            let snapshot = projection.snapshot_with_events(&game, &mut per_player_events, &[]);
            Some(SnapshotFanoutPayload::new(snapshot, player.spectator))
        });
        self.phase = Phase::InGame(game);
    }

    fn lab_room_time_state(&self) -> Option<RoomTimeState> {
        if self.session_policy().clock.room_time_source() != Some(RoomTimeSource::Lab) {
            return None;
        }
        let Phase::InGame(game) = &self.phase else {
            return None;
        };
        let mut state = self.room_time_state_for_live_game(game, self.lab_room_time_controller_id);
        if let Some(timeline) = &self.lab_timeline {
            state.duration_ticks = timeline.duration_ticks(game.tick_count());
            state.keyframe_ticks = timeline.keyframe_ticks();
        }
        Some(state)
    }

    pub(super) fn send_lab_room_time_state_to(&self, player_id: u32) {
        let Some(state) = self.lab_room_time_state() else {
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

    pub(super) fn broadcast_lab_room_time_state(&self) {
        let Some(state) = self.lab_room_time_state() else {
            return;
        };
        self.broadcast(&ServerMessage::RoomTimeState(state));
    }

    pub(super) fn lab_room_time_control_allowed(&self, player_id: u32) -> bool {
        self.lab_session
            .as_ref()
            .map(|session| session.can_operate(player_id))
            .unwrap_or(false)
    }

    fn lab_timeline_entry_cap_reset_keyframe(&self) -> Option<Game> {
        if !self
            .lab_timeline
            .as_ref()
            .is_some_and(LabTimeline::is_entry_cap_reached)
        {
            return None;
        }
        self.live_game().map(Game::clone_for_replay_keyframe)
    }

    pub(super) fn on_lab_request(&mut self, player_id: u32, request_id: u32, op: LabClientOp) {
        let op_kind = lab_op_kind(&op).to_string();
        if request_id == 0 {
            self.send_lab_result_to(
                player_id,
                LabResult {
                    request_id,
                    ok: false,
                    op: op_kind,
                    error: Some("requestId must be nonzero".to_string()),
                    outcome: None,
                },
            );
            return;
        }
        let policy = self.session_policy();
        if !policy.allows_lab_privileged_ops() {
            self.send_lab_result_to(
                player_id,
                LabResult {
                    request_id,
                    ok: false,
                    op: op_kind,
                    error: Some("lab requests are only valid in lab rooms".to_string()),
                    outcome: None,
                },
            );
            return;
        }
        if matches!(
            op,
            LabClientOp::ExportScenario { .. } | LabClientOp::ImportScenario { .. }
        ) && !policy.allows_lab_scenario_io()
        {
            self.send_lab_result_to(
                player_id,
                LabResult {
                    request_id,
                    ok: false,
                    op: op_kind,
                    error: Some(
                        "lab scenario import/export is not enabled in this room".to_string(),
                    ),
                    outcome: None,
                },
            );
            return;
        }
        if !self
            .lab_session
            .as_ref()
            .map(|session| session.can_operate(player_id))
            .unwrap_or(false)
        {
            self.send_lab_result_to(
                player_id,
                LabResult {
                    request_id,
                    ok: false,
                    op: op_kind,
                    error: Some("only lab operators can send lab requests".to_string()),
                    outcome: None,
                },
            );
            return;
        }

        let result = match op {
            LabClientOp::SetVision { vision } => {
                self.apply_lab_vision(player_id, request_id, vision)
            }
            LabClientOp::ExportScenario { name } => {
                self.export_lab_scenario(player_id, request_id, name)
            }
            LabClientOp::IssueCommandAs {
                player_id: command_player_id,
                cmd,
            } => self.apply_lab_issue_command(request_id, player_id, command_player_id, cmd),
            op => self.apply_lab_mutation(player_id, request_id, op),
        };
        self.send_lab_result_to(player_id, result);
    }

    fn apply_lab_vision(
        &mut self,
        operator_id: u32,
        request_id: u32,
        vision: LabVisionMode,
    ) -> LabResult {
        let op = "setVision".to_string();
        let Some(game) = self.live_game() else {
            return lab_result_error(request_id, op, "lab game is not running");
        };
        if let Err(err) = validate_lab_vision(game, &vision) {
            return lab_result_error(request_id, op, &err);
        }
        let tick = game.tick_count();
        let log_operations = self.session_policy().logs_lab_operations();
        if let Some(session) = &mut self.lab_session {
            session.set_vision_for(operator_id, vision);
            if log_operations {
                session.operation_log.push(LabOperationLogEntry {
                    tick,
                    request_id,
                    operator_id,
                    op: op.clone(),
                    result: "accepted".to_string(),
                });
            }
        }
        self.broadcast_lab_state();
        LabResult {
            request_id,
            ok: true,
            op,
            error: None,
            outcome: None,
        }
    }

    fn apply_lab_issue_command(
        &mut self,
        request_id: u32,
        operator_id: u32,
        command_player_id: u32,
        cmd: Command,
    ) -> LabResult {
        let op = "issueCommandAs".to_string();
        let log_operations = self.session_policy().logs_lab_operations();
        let timeline_capacity_reset = self.lab_timeline_entry_cap_reset_keyframe();
        let tick = {
            let Some(game) = self.live_game_mut() else {
                return lab_result_error(request_id, op, "lab game is not running");
            };
            if let Err(err) = game.issue_lab_command_as(command_player_id, cmd.clone()) {
                return lab_result_error(request_id, op, &lab_error_text(&err));
            }
            game.tick_count()
        };
        let mut timeline_truncated = false;
        if let Some(timeline) = &mut self.lab_timeline {
            if let Some(game) = timeline_capacity_reset.as_ref() {
                timeline.reset(game);
            } else {
                timeline_truncated = timeline.truncate_future(tick);
            }
            timeline.record_issue_command_as(tick, request_id, operator_id, command_player_id, cmd);
        }
        if let Some(session) = &mut self.lab_session {
            session.dirty = true;
            if log_operations {
                session.operation_log.push(LabOperationLogEntry {
                    tick,
                    request_id,
                    operator_id,
                    op: op.clone(),
                    result: format!("playerId={command_player_id}"),
                });
            }
        }
        self.broadcast_lab_state();
        if timeline_capacity_reset.is_some() || timeline_truncated {
            self.broadcast_lab_room_time_state();
        }
        LabResult {
            request_id,
            ok: true,
            op,
            error: None,
            outcome: None,
        }
    }

    fn export_lab_scenario(
        &self,
        operator_id: u32,
        request_id: u32,
        name: Option<String>,
    ) -> LabResult {
        let op = "exportScenario".to_string();
        if !self.session_policy().allows_lab_scenario_io() {
            return lab_result_error(request_id, op, "lab scenario export is not enabled");
        }
        let Some(game) = self.live_game() else {
            return lab_result_error(request_id, op, "lab game is not running");
        };
        let Some(session) = &self.lab_session else {
            return lab_result_error(request_id, op, "lab session is not running");
        };
        let mut scenario = match serde_json::to_value(game.export_lab_scenario()) {
            Ok(value) => value,
            Err(err) => {
                return lab_result_error(request_id, op, &format!("scenario export failed: {err}"));
            }
        };
        if let Some(object) = scenario.as_object_mut() {
            let scenario_name = name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Untitled lab scenario");
            object.insert(
                "name".to_string(),
                serde_json::Value::String(truncate_lab_scenario_name(scenario_name)),
            );
            if let Some(metadata) = object
                .get_mut("metadata")
                .and_then(|value| value.as_object_mut())
            {
                metadata.insert(
                    "lab".to_string(),
                    serde_json::to_value(LabScenarioLabMetadata {
                        vision: session.vision_for(operator_id),
                    })
                    .unwrap_or_else(|_| serde_json::json!({ "vision": { "mode": "fullWorld" } })),
                );
            }
        }
        LabResult {
            request_id,
            ok: true,
            op,
            error: None,
            outcome: Some(serde_json::json!({ "scenario": scenario })),
        }
    }

    fn apply_lab_mutation(
        &mut self,
        operator_id: u32,
        request_id: u32,
        op: LabClientOp,
    ) -> LabResult {
        let op_kind = lab_op_kind(&op).to_string();
        let imported_vision = match &op {
            LabClientOp::ImportScenario { scenario } => Some(scenario.metadata.lab.vision.clone()),
            _ => None,
        };
        let lab_op = match lab_client_op_to_game_op(op) {
            Ok(op) => op,
            Err(err) => return lab_result_error(request_id, op_kind, &err),
        };
        let resets_timeline = matches!(lab_op, LabOp::RestoreScenario(_));
        let timeline_op = lab_op.clone();
        let log_operations = self.session_policy().logs_lab_operations();
        let timeline_capacity_reset = if resets_timeline {
            None
        } else {
            self.lab_timeline_entry_cap_reset_keyframe()
        };
        let (tick, outcome_json) = {
            let Some(game) = self.live_game_mut() else {
                return lab_result_error(request_id, op_kind, "lab game is not running");
            };
            let outcome = match game.apply_lab_op(lab_op) {
                Ok(outcome) => outcome,
                Err(err) => return lab_result_error(request_id, op_kind, &lab_error_text(&err)),
            };
            (game.tick_count(), lab_outcome_json(&outcome))
        };
        let reset_game = if resets_timeline {
            self.live_game().map(Game::clone_for_replay_keyframe)
        } else {
            None
        };
        let mut timeline_truncated = false;
        if let Some(timeline) = &mut self.lab_timeline {
            if let Some(game) = reset_game.as_ref() {
                timeline.reset(game);
            } else {
                if let Some(game) = timeline_capacity_reset.as_ref() {
                    timeline.reset(game);
                } else {
                    timeline_truncated = timeline.truncate_future(tick);
                }
                timeline.record_lab_operation(
                    tick,
                    request_id,
                    operator_id,
                    op_kind.clone(),
                    timeline_op,
                );
            }
        }
        if let Some(session) = &mut self.lab_session {
            session.dirty = true;
            if let Some(vision) = imported_vision {
                session.import_vision_for(operator_id, vision);
            }
            if log_operations {
                session.operation_log.push(LabOperationLogEntry {
                    tick,
                    request_id,
                    operator_id,
                    op: op_kind.clone(),
                    result: outcome_json.to_string(),
                });
            }
        }
        self.broadcast_lab_state();
        if reset_game.is_some() || timeline_capacity_reset.is_some() || timeline_truncated {
            self.broadcast_lab_room_time_state();
        }
        LabResult {
            request_id,
            ok: true,
            op: op_kind,
            error: None,
            outcome: Some(outcome_json),
        }
    }

    fn live_game(&self) -> Option<&Game> {
        match &self.phase {
            Phase::InGame(game) => Some(game),
            _ => None,
        }
    }

    fn live_game_mut(&mut self) -> Option<&mut Game> {
        match &mut self.phase {
            Phase::InGame(game) => Some(game),
            _ => None,
        }
    }

    fn send_lab_result_to(&self, player_id: u32, result: LabResult) {
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        send_or_log(
            &self.room,
            player_id,
            &player.msg_tx,
            ServerMessage::LabResult(result),
        );
    }

    fn broadcast_lab_state(&self) {
        let Some(session) = &self.lab_session else {
            return;
        };
        for &id in &self.order {
            let Some(player) = self.players.get(&id) else {
                continue;
            };
            send_or_log(
                &self.room,
                id,
                &player.msg_tx,
                ServerMessage::LabState(session.state_for(id)),
            );
        }
    }

    pub(super) fn on_seek_lab_room_time(&mut self, player_id: u32, target: LabSeekTarget) {
        if !self.lab_room_time_control_allowed(player_id) {
            self.send_error_to(player_id, "Only lab operators can seek lab time.");
            return;
        }
        let Some(current_tick) = self.live_game().map(Game::tick_count) else {
            self.send_error_to(player_id, "Lab seek failed: lab game is not running.");
            return;
        };
        let viewer_count = self.players.len();
        let seek_result = {
            let Some(timeline) = &mut self.lab_timeline else {
                self.send_error_to(player_id, "Lab seek failed: timeline is not available.");
                return;
            };
            match target {
                LabSeekTarget::Relative(ticks_back) => {
                    timeline.seek_back(current_tick, ticks_back, Self::replay_lab_timeline_entry)
                }
                LabSeekTarget::Absolute(tick) => {
                    timeline.seek_to(current_tick, tick, Self::replay_lab_timeline_entry)
                }
            }
        };
        match seek_result {
            Ok(seek) => {
                crate::log_info!(
                    room = %self.room,
                    controller_id = player_id,
                    viewer_count,
                    from_tick = current_tick,
                    to_tick = seek.target_tick,
                    keyframe_tick = seek.keyframe_tick,
                    rebuild_ms = seek.rebuild_ms,
                    "lab seek rebuilt"
                );
                self.phase = Phase::InGame(Box::new(seek.game));
                self.lab_room_time_controller_id = Some(player_id);
                self.send_lab_start_payloads_to_all(true);
                self.broadcast_lab_room_time_state();
                self.broadcast_lab_state();
                self.fanout_current_lab_snapshots();
            }
            Err(err) => {
                crate::log_warn!(room = %self.room, error = %err, "lab seek failed");
                self.send_error_to(player_id, &err);
            }
        }
    }

    fn replay_lab_timeline_entry(game: &mut Game, entry: &LabTimelineEntry) -> Result<(), String> {
        match &entry.kind {
            LabTimelineEntryKind::LabOperation { op_kind, op } => game
                .apply_lab_op(op.clone())
                .map(|_| ())
                .map_err(|err| {
                    format!(
                        "Lab timeline operation {op_kind} failed at sequence {} request {}: {err:?}.",
                        entry.sequence, entry.request_id
                    )
                }),
            LabTimelineEntryKind::IssueCommandAs { player_id, command } => game
                .issue_lab_command_as(*player_id, command.clone())
                .map_err(|err| {
                    format!(
                        "Lab timeline issue-as failed at sequence {} request {}: {err:?}.",
                        entry.sequence, entry.request_id
                    )
            }),
        }
    }
}
