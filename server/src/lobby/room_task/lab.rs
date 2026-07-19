use std::collections::HashMap;
use std::str::FromStr;

use rts_sim::game::entity::EntityKind;
use rts_sim::game::lab::{
    LabCommandOptions, LabError, LabMoveEntity, LabOp, LabOpOutcome, LabSetCompletedResearch,
    LabSetEntityOwner, LabSetPlayerResources, LabSpawnEntity, LabUpdate,
};
use rts_sim::game::map::Map;
use rts_sim::game::upgrade::UpgradeKind;

use super::super::connection::{send_or_log, ConnectionSink};
use super::super::dev_replay::match_seed;
use super::super::lab_replay_operations::lab_op_to_replay_operation;
use super::super::lab_timeline::LabTimeline;
use super::super::launch::{LaunchRecipient, StartPayloadBuilder};
use super::super::projection::RecipientRole;
use super::super::session_policy::{RoomTimeSource, SessionPhase, SessionPolicy};
use super::super::{normalize_start_team_id, MAX_PLAYERS, PLAYER_PALETTE};
use super::helpers::DRAINING_NEW_MATCHES_DISABLED_MSG;
use super::types::{LabRoomConfig, Phase, RoomMode, RoomPlayer};
use super::RoomTask;
use crate::lab_scenarios::{
    export_lab_checkpoint_scenario_for_protocol, lab_scenario_payload_lab_metadata,
    lab_scenario_payload_to_lab_op, load_lab_scenario_by_id, validate_lab_scenario_authoring,
};
use crate::lobby::lab_scenario_driver::lab_scenario_driver_for;
use crate::protocol::{
    InitialCamera, LabClientOp, LabResult, LabScenarioLabMetadata, LabScenarioPayload,
    LabStartMetadata, LabStartRole, LabState, LabUpdateSpec, LabVisionMode, ServerMessage,
    DEFAULT_FACTION_ID,
};
use crate::structured_log::{self, MatchStartedLog};
use rts_sim::game::{Game, ObserverView, PlayerInit};

use replay::LabReplayRebaseSource;

mod replay;

#[derive(Clone)]
pub(super) struct LabSession {
    pub(super) public_id: String,
    pub(super) operator_id: u32,
    pub(super) viewer_roles: HashMap<u32, LabStartRole>,
    pub(super) viewer_visions: HashMap<u32, LabVisionMode>,
    pub(super) default_vision: LabVisionMode,
    pub(super) initial_camera: Option<InitialCamera>,
    pub(super) dirty: bool,
    pub(super) operation_log: Vec<LabOperationLogEntry>,
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

struct LabLaunch {
    game: Game,
    seed: u32,
    map_name: String,
    player_count: usize,
    participants: Vec<String>,
    default_vision: Option<LabVisionMode>,
    initial_camera: Option<InitialCamera>,
    god_mode_players: Vec<u32>,
}

impl LabSession {
    pub(super) fn new(config: &LabRoomConfig, operator_id: u32) -> Self {
        let mut viewer_roles = HashMap::new();
        viewer_roles.insert(operator_id, LabStartRole::Operator);
        let default_vision = LabVisionMode::All;
        let mut viewer_visions = HashMap::new();
        viewer_visions.insert(operator_id, default_vision.clone());
        Self {
            public_id: config.public_id.clone(),
            operator_id,
            viewer_roles,
            viewer_visions,
            default_vision,
            initial_camera: None,
            dirty: false,
            operation_log: Vec::new(),
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

    pub(super) fn metadata_for(
        &self,
        player_id: u32,
        god_mode_players: Vec<u32>,
    ) -> LabStartMetadata {
        LabStartMetadata {
            room: self.public_id.clone(),
            operator_id: self.operator_id,
            role: self.role_for(player_id),
            vision: self.vision_for(player_id),
            god_mode_players,
            initial_camera: self.initial_camera,
            dirty: self.dirty,
            operation_count: self.operation_log.len() as u32,
        }
    }

    pub(super) fn state_for(&self, player_id: u32, god_mode_players: Vec<u32>) -> LabState {
        LabState {
            room: self.public_id.clone(),
            operator_id: self.operator_id,
            role: self.role_for(player_id),
            vision: self.vision_for(player_id),
            god_mode_players,
            dirty: self.dirty,
            operation_count: self.operation_log.len() as u32,
        }
    }
}

fn player_ids_for_vision(players: &[PlayerInit], vision: &LabVisionMode) -> Vec<u32> {
    players
        .iter()
        .filter(|player| match vision {
            LabVisionMode::All => true,
            LabVisionMode::Team { team_id } => player.team_id == *team_id,
        })
        .map(|player| player.id)
        .collect()
}

fn observer_view_for_lab_vision(players: &[PlayerInit], vision: &LabVisionMode) -> ObserverView {
    ObserverView::Players(player_ids_for_vision(players, vision))
}

fn lab_op_kind(op: &LabClientOp) -> &'static str {
    match op {
        LabClientOp::ExportMap => "exportMap",
        LabClientOp::ExportScenario { .. } => "exportScenario",
        LabClientOp::ImportScenario { .. } => "importScenario",
        LabClientOp::ValidateScenario { .. } => "validateScenario",
        LabClientOp::SpawnEntity { .. } => "spawnEntity",
        LabClientOp::SpawnEntities { .. } => "spawnEntities",
        LabClientOp::DeleteEntity { .. } => "deleteEntity",
        LabClientOp::DeleteEntities { .. } => "deleteEntities",
        LabClientOp::MoveEntity { .. } => "moveEntity",
        LabClientOp::ApplyUpdates { .. } => "applyUpdates",
        LabClientOp::SetEntityOwner { .. } => "setEntityOwner",
        LabClientOp::SetPlayerResources { .. } => "setPlayerResources",
        LabClientOp::SetPlayerGodMode { .. } => "setPlayerGodMode",
        LabClientOp::SetCompletedResearch { .. } => "setCompletedResearch",
        LabClientOp::SetVision { .. } => "setVision",
        LabClientOp::IssueCommandAs { .. } => "issueCommandAs",
    }
}

fn lab_client_op_to_game_op(op: LabClientOp) -> Result<LabOp, (String, Option<u32>)> {
    match op {
        LabClientOp::ImportScenario { scenario } => {
            lab_scenario_payload_to_lab_op(*scenario).map_err(|error| (error, None))
        }
        LabClientOp::SpawnEntity {
            owner,
            kind,
            x,
            y,
            completed,
        } => {
            let kind = EntityKind::from_str(&kind)
                .map_err(|_| ("unknown entity kind".to_string(), None))?;
            Ok(LabOp::SpawnEntity(LabSpawnEntity {
                owner,
                kind,
                x,
                y,
                completed,
            }))
        }
        LabClientOp::SpawnEntities { spawns } => Ok(LabOp::SpawnEntities(
            spawns
                .into_iter()
                .enumerate()
                .map(|(index, spawn)| {
                    let kind = EntityKind::from_str(&spawn.kind).map_err(|_| {
                        ("unknown entity kind".to_string(), u32::try_from(index).ok())
                    })?;
                    Ok(LabSpawnEntity {
                        owner: spawn.owner,
                        kind,
                        x: spawn.x,
                        y: spawn.y,
                        completed: spawn.completed,
                    })
                })
                .collect::<Result<Vec<_>, (String, Option<u32>)>>()?,
        )),
        LabClientOp::DeleteEntity { entity_id } => Ok(LabOp::DeleteEntity { entity_id }),
        LabClientOp::DeleteEntities { entity_ids } => Ok(LabOp::DeleteEntities(entity_ids)),
        LabClientOp::MoveEntity { entity_id, x, y } => {
            Ok(LabOp::MoveEntity(LabMoveEntity { entity_id, x, y }))
        }
        LabClientOp::ApplyUpdates { updates } => Ok(LabOp::ApplyUpdates(
            updates
                .into_iter()
                .enumerate()
                .map(|(index, update)| match update {
                    LabUpdateSpec::Move { entity_id, x, y } => {
                        Ok(LabUpdate::Move(LabMoveEntity { entity_id, x, y }))
                    }
                    LabUpdateSpec::Reassign { entity_id, owner } => {
                        Ok(LabUpdate::SetEntityOwner(LabSetEntityOwner {
                            entity_id,
                            owner,
                        }))
                    }
                    LabUpdateSpec::Resources {
                        player_id,
                        steel,
                        oil,
                    } => Ok(LabUpdate::SetPlayerResources(LabSetPlayerResources {
                        player_id,
                        steel,
                        oil,
                    })),
                    LabUpdateSpec::Research {
                        player_id,
                        upgrade,
                        completed,
                    } => {
                        let upgrade = UpgradeKind::from_str(&upgrade).map_err(|_| {
                            ("unknown research id".to_string(), u32::try_from(index).ok())
                        })?;
                        Ok(LabUpdate::SetCompletedResearch(LabSetCompletedResearch {
                            player_id,
                            upgrade,
                            completed,
                        }))
                    }
                    LabUpdateSpec::GodMode { player_id, enabled } => {
                        Ok(LabUpdate::SetPlayerGodMode { player_id, enabled })
                    }
                })
                .collect::<Result<Vec<_>, (String, Option<u32>)>>()?,
        )),
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
        LabClientOp::SetPlayerGodMode { player_id, enabled } => {
            Ok(LabOp::SetPlayerGodMode { player_id, enabled })
        }
        LabClientOp::SetCompletedResearch {
            player_id,
            upgrade,
            completed,
        } => {
            let upgrade = UpgradeKind::from_str(&upgrade)
                .map_err(|_| ("unknown research id".to_string(), None))?;
            Ok(LabOp::SetCompletedResearch(LabSetCompletedResearch {
                player_id,
                upgrade,
                completed,
            }))
        }
        LabClientOp::ExportMap
        | LabClientOp::ExportScenario { .. }
        | LabClientOp::ValidateScenario { .. }
        | LabClientOp::SetVision { .. }
        | LabClientOp::IssueCommandAs { .. } => Err(("not a lab mutation".to_string(), None)),
    }
}

fn validate_lab_vision(game: &Game, vision: &LabVisionMode) -> Result<(), String> {
    let players = game.player_inits();
    match vision {
        LabVisionMode::All => Ok(()),
        LabVisionMode::Team { team_id } => {
            if players.iter().any(|player| player.team_id == *team_id) {
                Ok(())
            } else {
                Err("unknown lab team id".to_string())
            }
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

fn lab_result_error(request_id: u32, op: String, error: &str) -> LabResult {
    LabResult {
        request_id,
        ok: false,
        op,
        error: Some(error.to_string()),
        failed_index: None,
        details: None,
        outcome: None,
    }
}

fn lab_result_ok(request_id: u32, op: String, outcome: Option<serde_json::Value>) -> LabResult {
    LabResult {
        request_id,
        ok: true,
        op,
        error: None,
        failed_index: None,
        details: None,
        outcome,
    }
}

fn lab_result_from_lab_error(request_id: u32, op: String, error: &LabError) -> LabResult {
    let (failed_index, leaf) = match error {
        LabError::BatchFailed {
            failed_index,
            error,
        } => (u32::try_from(*failed_index).ok(), error.as_ref()),
        error => (None, error),
    };
    let details = match leaf {
        LabError::Placement {
            x,
            y,
            blockers,
            suggestions,
        } => Some(serde_json::json!({
            "attempted": { "x": x, "y": y },
            "blockers": blockers,
            "suggestions": suggestions.iter().map(|(x, y)| serde_json::json!({ "x": x, "y": y })).collect::<Vec<_>>(),
        })),
        _ => None,
    };
    LabResult {
        request_id,
        ok: false,
        op,
        error: Some(lab_error_text(error)),
        failed_index,
        details,
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
        LabError::Placement { x, y, .. } => {
            format!("blocked placement ({x}, {y})")
        }
        LabError::BatchSize { count, maximum } => {
            format!("batch contains {count} items; expected 1 to {maximum}")
        }
        LabError::DuplicateMutation { reason } => reason.clone(),
        LabError::BatchFailed {
            failed_index,
            error,
        } => format!(
            "batch item {failed_index} failed: {}",
            lab_error_text(error)
        ),
        LabError::InvalidResearch { player_id, upgrade } => {
            format!("invalid research {upgrade:?} for player {player_id}")
        }
        LabError::InvalidScenarioVersion { version } => {
            format!("unsupported setup JSON version {version}")
        }
        LabError::InvalidScenario { reason } => lab_setup_error_text(reason),
        LabError::InvalidMap { name, reason } => format!("invalid map {name:?}: {reason}"),
        LabError::InvalidCommand { reason } => reason.clone(),
    }
}

fn lab_setup_error_text(reason: &str) -> String {
    reason
        .replace("scenario kind", "legacy scenario kind")
        .replace("scenario name", "setup name")
        .replace("scenario must contain", "setup must contain")
        .replace("scenario has too many", "setup has too many")
}

fn lab_outcome_json(outcome: &LabOpOutcome) -> serde_json::Value {
    match outcome {
        LabOpOutcome::Batch(outcomes) => serde_json::json!({
            "items": outcomes.iter().enumerate().map(|(index, outcome)| serde_json::json!({
                "index": index,
                "outcome": lab_outcome_json(outcome),
            })).collect::<Vec<_>>()
        }),
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
        LabOpOutcome::PlayerGodModeSet { player_id, enabled } => {
            serde_json::json!({ "playerId": player_id, "enabled": enabled })
        }
        LabOpOutcome::CompletedResearchSet {
            player_id,
            upgrade,
            completed,
        } => serde_json::json!({
            "playerId": player_id,
            "upgrade": upgrade.to_protocol_str(),
            "completed": completed
        }),
        LabOpOutcome::MapDraftApplied {
            name,
            size,
            battle_reset,
        } => serde_json::json!({
            "name": name,
            "size": size,
            "battleReset": battle_reset
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
        if let (Some(game), Some(session)) = (self.live_game(), self.lab_session.as_ref()) {
            self.observer_views.insert(
                player_id,
                observer_view_for_lab_vision(&game.player_inits(), &session.vision_for(player_id)),
            );
        }
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
        let launch = match self.build_lab_launch(&config) {
            Ok(launch) => launch,
            Err(err) => {
                self.send_lab_error(err);
                return;
            }
        };
        if let Some(vision) = launch.default_vision.clone() {
            if let Some(session) = &mut self.lab_session {
                session.import_vision_for(session.operator_id, vision);
            }
        }
        if let Some(session) = &mut self.lab_session {
            session.initial_camera = launch.initial_camera;
        }
        let launch_god_mode_players = launch.god_mode_players.clone();
        let launch_driver = config.scenario.as_deref().and_then(lab_scenario_driver_for);
        let game = launch.game;
        let initial_setup =
            match self.export_lab_replay_initial_setup(&game, "Initial lab setup".to_string()) {
                Ok(setup) => setup,
                Err(err) => {
                    self.send_lab_error(format!("Cannot start lab replay timeline: {err}"));
                    return;
                }
            };
        self.capture_replay_start_for(&game);
        self.record_live_match_started(
            launch.player_count,
            0,
            launch.map_name.clone(),
            launch.participants,
        );
        let mut payload = game.start_payload();
        payload.match_run_id = self.match_run_id.clone();
        self.ai_controllers.clear();
        self.dev_driver = None;
        self.dev_view_player_id = None;
        self.lab_driver = launch_driver;

        let projection_policy = self.projection_policy_for_phase(SessionPhase::LiveMatch);
        self.sync_lab_legacy_observer_views(&game);
        let valid_player_ids = game
            .player_inits()
            .iter()
            .map(|player| player.id)
            .collect::<Vec<_>>();
        let start_policy = SessionPolicy::for_room(&self.mode, SessionPhase::LiveMatch);
        let recipients: Vec<_> = self
            .order
            .iter()
            .filter_map(|&id| {
                self.players.get(&id).map(|player| {
                    LaunchRecipient::observer(
                        id,
                        projection_policy.diagnostic_capabilities_for(RecipientRole::Spectator),
                        false,
                        self.lab_session.as_ref().map(|session| {
                            session.metadata_for(id, launch_god_mode_players.clone())
                        }),
                        self.observer_view_selection_for_player_ids(id, &valid_player_ids),
                        player.msg_tx.clone(),
                    )
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
            seed: launch.seed,
            players: self.match_player_count,
            humans: self.match_human_count,
            ai: 0,
            participants: &self.match_participants,
        });
        self.mark_match_started_for_drain();
        self.lab_timeline = Some(LabTimeline::new(&game, initial_setup));
        self.phase = Phase::InGame(Box::new(game));
        self.broadcast_lab_room_time_state();
    }

    fn build_lab_launch(&self, config: &LabRoomConfig) -> Result<LabLaunch, String> {
        match config.scenario {
            Some(ref scenario_id) => self.build_lab_launch_from_scenario(scenario_id),
            None => self.build_blank_lab_launch(config),
        }
    }

    fn build_lab_launch_from_scenario(&self, scenario_id: &str) -> Result<LabLaunch, String> {
        let loaded = load_lab_scenario_by_id(scenario_id)
            .map_err(|err| format!("Cannot load lab setup \"{scenario_id}\": {err}"))?;
        let game = loaded
            .build_game()
            .map_err(|err| format!("Cannot load lab setup \"{scenario_id}\": {err}"))?;
        let god_mode_players = game.lab_god_mode_players();
        let start_payload = game.start_payload();
        let (seed, map_name) = match &loaded.scenario {
            LabScenarioPayload::Checkpoint(scenario) => (scenario.seed, scenario.map.name.clone()),
        };
        let lab_metadata = lab_scenario_payload_lab_metadata(&loaded.scenario);
        Ok(LabLaunch {
            game,
            seed,
            map_name,
            player_count: start_payload.players.len(),
            participants: start_payload
                .players
                .iter()
                .map(|player| player.name.clone())
                .collect(),
            default_vision: Some(lab_metadata.vision.clone()),
            initial_camera: lab_metadata.initial_camera,
            god_mode_players,
        })
    }

    fn build_blank_lab_launch(&self, config: &LabRoomConfig) -> Result<LabLaunch, String> {
        let seed = config.seed.unwrap_or_else(match_seed);
        let draft = config.map_draft.as_ref();
        let player_count = draft.map_or(2, |draft| draft.starts.len());
        if !(1..=MAX_PLAYERS).contains(&player_count) {
            return Err(format!(
                "Expected 1-{MAX_PLAYERS} Map Editor starts; got {player_count}"
            ));
        }
        let inits = Self::default_lab_player_template(player_count);
        let start_players: Vec<_> = inits
            .iter()
            .map(|player| {
                (
                    player.id,
                    normalize_start_team_id(player.id, player.team_id),
                )
            })
            .collect();
        let map_metadata = Map::metadata_for_name(&config.map_name)
            .map_err(|err| format!("Cannot load lab map \"{}\": {err}", config.map_name))?;
        let map = Map::load_for_players(&config.map_name, &start_players, seed)
            .map_err(|err| format!("Cannot load lab map \"{}\": {err}", config.map_name))?;
        let mut game =
            Game::new_with_random_ai_profiles_and_map_metadata(&inits, seed, map, map_metadata);
        let mut map_name = config.map_name.clone();
        if let Some(draft) = draft {
            map_name.clone_from(&draft.name);
            game.apply_lab_op(LabOp::ApplyMapDraft(draft.clone()))
                .map_err(|err| {
                    format!(
                        "Cannot materialize Map Editor handoff: {}",
                        lab_error_text(&err)
                    )
                })?;
        }
        Ok(LabLaunch {
            game,
            seed,
            map_name,
            player_count: inits.len(),
            participants: inits.iter().map(|player| player.name.clone()).collect(),
            default_vision: None,
            initial_camera: None,
            god_mode_players: Vec::new(),
        })
    }

    fn default_lab_player_template(player_count: usize) -> Vec<PlayerInit> {
        const NAMES: [&str; MAX_PLAYERS] = ["Alpha", "Bravo", "Charlie", "Delta"];
        (0..player_count)
            .map(|index| PlayerInit {
                id: index as u32 + 1,
                team_id: index as u32 + 1,
                faction_id: DEFAULT_FACTION_ID.to_string(),
                name: format!("Lab {}", NAMES[index]),
                color: PLAYER_PALETTE[index].to_string(),
                is_ai: false,
            })
            .collect()
    }

    fn send_lab_error(&self, msg: String) {
        let error = ServerMessage::Error { msg };
        self.broadcast(&error);
    }

    fn send_lab_start_payloads(&self, recipients: impl IntoIterator<Item = LaunchRecipient>) {
        let Phase::InGame(game) = &self.phase else {
            return;
        };
        let payload = game.start_payload();
        let builder = StartPayloadBuilder::simulation(self.session_policy(), &payload);
        super::super::launch::send_start_payloads(&self.room, &builder, recipients);
    }

    pub(super) fn send_lab_start_to(&self, watcher_id: u32) {
        let Some(player) = self.players.get(&watcher_id) else {
            return;
        };
        let diagnostics = self
            .projection_policy()
            .diagnostic_capabilities_for(RecipientRole::Spectator);
        self.send_lab_start_payloads([LaunchRecipient::observer(
            watcher_id,
            diagnostics,
            false,
            self.lab_start_metadata_for(watcher_id),
            self.observer_view_selection_for(watcher_id),
            player.msg_tx.clone(),
        )]);
    }

    fn send_lab_start_payloads_to_all(&self, clear_pending_snapshot: bool) {
        self.send_lab_start_payloads_except(clear_pending_snapshot, None);
    }

    fn send_lab_start_payloads_except(
        &self,
        clear_pending_snapshot: bool,
        excluded_player_id: Option<u32>,
    ) {
        let diagnostics = self
            .projection_policy()
            .diagnostic_capabilities_for(RecipientRole::Spectator);
        let recipients = self
            .order
            .iter()
            .filter(|&&id| Some(id) != excluded_player_id)
            .filter_map(|&id| {
                self.players.get(&id).map(|player| {
                    LaunchRecipient::observer(
                        id,
                        diagnostics,
                        clear_pending_snapshot,
                        self.lab_start_metadata_for(id),
                        self.observer_view_selection_for(id),
                        player.msg_tx.clone(),
                    )
                })
            });
        self.send_lab_start_payloads(recipients);
    }

    fn lab_start_metadata_for(&self, player_id: u32) -> Option<LabStartMetadata> {
        let god_mode_players = self
            .live_game()
            .map(Game::lab_god_mode_players)
            .unwrap_or_default();
        self.lab_session
            .as_ref()
            .map(|session| session.metadata_for(player_id, god_mode_players))
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
                    failed_index: None,
                    details: None,
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
                    failed_index: None,
                    details: None,
                    outcome: None,
                },
            );
            return;
        }
        if matches!(
            op,
            LabClientOp::ExportScenario { .. }
                | LabClientOp::ImportScenario { .. }
                | LabClientOp::ValidateScenario { .. }
        ) && !policy.allows_lab_scenario_io()
        {
            self.send_lab_result_to(
                player_id,
                LabResult {
                    request_id,
                    ok: false,
                    op: op_kind,
                    error: Some("lab setup import/export is not enabled in this room".to_string()),
                    failed_index: None,
                    details: None,
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
                    failed_index: None,
                    details: None,
                    outcome: None,
                },
            );
            return;
        }

        let mut refresh_snapshot_after_result = false;
        let result = match op {
            LabClientOp::SetVision { vision } => {
                refresh_snapshot_after_result = true;
                Some(self.apply_lab_vision(player_id, request_id, vision))
            }
            LabClientOp::ExportMap => Some(self.export_lab_map(request_id)),
            LabClientOp::ExportScenario { name } => {
                Some(self.export_lab_scenario(player_id, request_id, name))
            }
            LabClientOp::ValidateScenario { metadata } => {
                Some(self.validate_lab_scenario(player_id, request_id, metadata))
            }
            LabClientOp::IssueCommandAs {
                player_id: command_player_id,
                cmd,
                ignore_command_limits,
            } => Some(self.apply_lab_issue_command(
                request_id,
                player_id,
                command_player_id,
                cmd,
                LabCommandOptions {
                    ignore_command_limits,
                },
            )),
            op => {
                refresh_snapshot_after_result = true;
                Some(self.apply_lab_mutation(player_id, request_id, op))
            }
        };
        if let Some(result) = result {
            let refresh_snapshot = refresh_snapshot_after_result && result.ok;
            self.send_lab_result_to(player_id, result);
            if refresh_snapshot
                && self.session_policy().clock.room_time_source() == Some(RoomTimeSource::Lab)
            {
                self.fanout_current_observer_snapshots_to(self.order.clone());
            }
        }
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
        let observer_view = observer_view_for_lab_vision(&game.player_inits(), &vision);
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
        self.observer_views.insert(operator_id, observer_view);
        self.broadcast_lab_state();
        lab_result_ok(request_id, op, None)
    }

    fn export_lab_scenario(
        &self,
        operator_id: u32,
        request_id: u32,
        name: Option<String>,
    ) -> LabResult {
        let op = "exportScenario".to_string();
        if !self.session_policy().allows_lab_scenario_io() {
            return lab_result_error(request_id, op, "lab setup export is not enabled");
        }
        let scenario = match self.export_lab_scenario_value(operator_id, name.as_deref()) {
            Ok(value) => value,
            Err(err) => return lab_result_error(request_id, op, &err),
        };
        lab_result_ok(
            request_id,
            op,
            Some(serde_json::json!({ "scenario": scenario })),
        )
    }

    fn export_lab_map(&self, request_id: u32) -> LabResult {
        let op = "exportMap".to_string();
        let Some(game) = self.live_game() else {
            return lab_result_error(request_id, op, "lab game is not running");
        };
        lab_result_ok(
            request_id,
            op,
            Some(serde_json::json!({ "map": game.export_lab_map() })),
        )
    }

    fn validate_lab_scenario(
        &self,
        operator_id: u32,
        request_id: u32,
        metadata: crate::protocol::LabScenarioAuthoringMetadata,
    ) -> LabResult {
        let op = "validateScenario".to_string();
        if !self.session_policy().allows_lab_scenario_io() {
            return lab_result_error(request_id, op, "lab setup validation is not enabled");
        }
        let scenario_value = match self.export_lab_scenario_value(operator_id, None) {
            Ok(value) => value,
            Err(err) => return lab_result_error(request_id, op, &err),
        };
        let scenario = match serde_json::from_value(scenario_value) {
            Ok(scenario) => scenario,
            Err(err) => {
                return lab_result_error(
                    request_id,
                    op,
                    &format!("setup export did not produce a lab setup payload: {err}"),
                );
            }
        };
        let preview = match validate_lab_scenario_authoring(metadata, scenario) {
            Ok(preview) => preview,
            Err(err) => return lab_result_error(request_id, op, &err),
        };
        lab_result_ok(
            request_id,
            op,
            Some(serde_json::json!({
                "summary": preview.summary,
                "preview": preview,
            })),
        )
    }

    fn export_lab_scenario_value(
        &self,
        operator_id: u32,
        name: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let Some(game) = self.live_game() else {
            return Err("lab game is not running".to_string());
        };
        let Some(session) = &self.lab_session else {
            return Err("lab session is not running".to_string());
        };
        let scenario_name = name
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(truncate_lab_scenario_name)
            .unwrap_or_else(|| "Untitled lab setup".to_string());
        let scenario = export_lab_checkpoint_scenario_for_protocol(
            game,
            scenario_name,
            LabScenarioLabMetadata {
                vision: session.vision_for(operator_id),
                god_mode_players: game.lab_god_mode_players(),
                initial_camera: session.initial_camera,
            },
            crate::build_info::build_id(),
        )?;
        serde_json::to_value(scenario).map_err(|err| format!("setup export failed: {err}"))
    }

    fn apply_lab_mutation(
        &mut self,
        operator_id: u32,
        request_id: u32,
        op: LabClientOp,
    ) -> LabResult {
        let op_kind = lab_op_kind(&op).to_string();
        let rebase_source = match &op {
            LabClientOp::ImportScenario { scenario } => match scenario.as_ref() {
                LabScenarioPayload::Checkpoint(scenario) => Some(
                    LabReplayRebaseSource::Checkpoint(Box::new(scenario.clone())),
                ),
            },
            _ => None,
        };
        let imported_vision = match &op {
            LabClientOp::ImportScenario { scenario } => {
                Some(lab_scenario_payload_lab_metadata(scenario).vision.clone())
            }
            _ => None,
        };
        let imported_initial_camera = match &op {
            LabClientOp::ImportScenario { scenario } => {
                lab_scenario_payload_lab_metadata(scenario).initial_camera
            }
            _ => None,
        };
        let lab_op = match lab_client_op_to_game_op(op) {
            Ok(op) => op,
            Err((err, failed_index)) => {
                let mut result = lab_result_error(request_id, op_kind, &err);
                result.failed_index = failed_index;
                return result;
            }
        };
        let resets_timeline = matches!(
            lab_op,
            LabOp::RestoreCheckpointScenario(_) | LabOp::ApplyMapDraft(_)
        );
        if !resets_timeline {
            return self.apply_and_record_lab_operation(operator_id, request_id, op_kind, lab_op);
        }

        let log_operations = self.session_policy().logs_lab_operations();
        let (tick, outcome) = {
            let Some(game) = self.live_game_mut() else {
                return lab_result_error(request_id, op_kind, "lab game is not running");
            };
            let outcome = match game.apply_lab_op(lab_op) {
                Ok(outcome) => outcome,
                Err(err) => return lab_result_from_lab_error(request_id, op_kind, &err),
            };
            (game.tick_count(), outcome)
        };
        let outcome_json = match &outcome {
            LabOpOutcome::MapDraftApplied {
                name,
                size,
                battle_reset,
            } => {
                let Some(payload) = self.live_game().map(Game::start_payload) else {
                    return lab_result_error(request_id, op_kind, "lab game is not running");
                };
                serde_json::json!({
                    "name": name,
                    "size": size,
                    "battleReset": battle_reset,
                    "tick": payload.tick,
                    "map": payload.map,
                    "players": payload.players,
                })
            }
            _ => lab_outcome_json(&outcome),
        };
        let Some(game) = self.live_game().map(Game::clone_for_replay_keyframe) else {
            return lab_result_error(request_id, op_kind, "lab game is not running");
        };
        let imported_observer_view = imported_vision
            .as_ref()
            .map(|vision| observer_view_for_lab_vision(&game.player_inits(), vision));
        let Some(source) = rebase_source else {
            return lab_result_error(
                request_id,
                op_kind,
                "lab replay setup import requires a rebase source",
            );
        };
        let initial_setup = match self.lab_replay_initial_setup_for_rebase(source, &outcome) {
            Ok(setup) => setup,
            Err(err) => return lab_result_error(request_id, op_kind, &err),
        };
        if let Some(timeline) = &mut self.lab_timeline {
            timeline.reset(&game, initial_setup);
        }
        if let Some(session) = &mut self.lab_session {
            session.dirty = true;
            if let Some(vision) = imported_vision {
                session.import_vision_for(operator_id, vision);
            }
            session.initial_camera = imported_initial_camera;
            self.lab_driver = None;
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
        if let Some(observer_view) = imported_observer_view {
            self.observer_views.insert(operator_id, observer_view);
        }
        self.broadcast_lab_state();
        self.broadcast_lab_room_time_state();
        lab_result_ok(request_id, op_kind, Some(outcome_json))
    }

    pub(super) fn apply_and_record_lab_operation(
        &mut self,
        operator_id: u32,
        request_id: u32,
        op_kind: String,
        lab_op: LabOp,
    ) -> LabResult {
        let timeline_capacity_reset = match self.lab_timeline_entry_cap_reset() {
            Ok(reset) => reset,
            Err(err) => return lab_result_error(request_id, op_kind, &err),
        };
        let timeline_op = lab_op.clone();
        let Some(replay_op) = lab_op_to_replay_operation(&timeline_op) else {
            return lab_result_error(
                request_id,
                op_kind,
                "lab operation is not serializable as a lab replay entry",
            );
        };
        let log_operations = self.session_policy().logs_lab_operations();
        let (tick, outcome) = {
            let Some(game) = self.live_game_mut() else {
                return lab_result_error(request_id, op_kind, "lab game is not running");
            };
            let outcome = match game.apply_lab_op(lab_op) {
                Ok(outcome) => outcome,
                Err(err) => return lab_result_from_lab_error(request_id, op_kind, &err),
            };
            (game.tick_count(), outcome)
        };
        let outcome_json = lab_outcome_json(&outcome);
        let mut timeline_truncated = false;
        if let Some(timeline) = &mut self.lab_timeline {
            if let Some((game, initial_setup)) = timeline_capacity_reset.as_ref() {
                timeline.reset(game, initial_setup.clone());
            } else {
                timeline_truncated = timeline.truncate_future(tick);
            }
            timeline.record_lab_operation(
                tick,
                request_id,
                operator_id,
                op_kind.clone(),
                timeline_op,
                replay_op,
            );
        }
        if let Some(session) = &mut self.lab_session {
            session.dirty = true;
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
        if timeline_capacity_reset.is_some() || timeline_truncated {
            self.broadcast_lab_room_time_state();
        }
        lab_result_ok(request_id, op_kind, Some(outcome_json))
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

    fn sync_lab_legacy_observer_views(&mut self, game: &Game) {
        let Some(session) = self.lab_session.as_ref() else {
            return;
        };
        let players = game.player_inits();
        self.observer_views
            .extend(self.order.iter().copied().map(|viewer_id| {
                (
                    viewer_id,
                    observer_view_for_lab_vision(&players, &session.vision_for(viewer_id)),
                )
            }));
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

    pub(in crate::lobby::room_task) fn broadcast_lab_state(&self) {
        let Some(session) = &self.lab_session else {
            return;
        };
        let god_mode_players = self
            .live_game()
            .map(Game::lab_god_mode_players)
            .unwrap_or_default();
        for &id in &self.order {
            let Some(player) = self.players.get(&id) else {
                continue;
            };
            let state = session.state_for(id, god_mode_players.clone());
            send_or_log(
                &self.room,
                id,
                &player.msg_tx,
                ServerMessage::LabState(state),
            );
        }
    }
}
