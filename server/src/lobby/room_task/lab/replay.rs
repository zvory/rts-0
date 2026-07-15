use rts_sim::game::lab::{LabCommandOptions, LabOp, LabOpOutcome};
use rts_sim::game::Game;
use std::time::Instant;

use super::super::super::connection::send_or_log;
use super::super::super::current_unix_ms;
use super::super::super::lab_replay_operations::{
    lab_op_to_replay_operation, lab_replay_operation_kind, lab_replay_operation_to_entry_kind,
};
use super::super::super::lab_scenario_driver::LabScenarioAction;
use super::super::super::lab_timeline::{LabTimeline, LabTimelineEntry, LabTimelineEntryKind};
use super::super::super::session_policy::RoomTimeSource;
use super::super::types::{LabSeekTarget, Phase};
use super::super::RoomTask;
use super::{lab_error_text, lab_result_error, LabOperationLogEntry};
use crate::lab_scenarios::{
    export_lab_checkpoint_scenario_for_protocol, lab_scenario_payload_to_lab_op,
};
use crate::protocol::{
    lab_replay_artifact_from_slice, Command, LabCheckpointScenarioV1, LabReplayArtifactV1,
    LabReplayAuthoringMetadata, LabReplayOperationEntry, LabReplayTimelineMetadata, LabResult,
    LabScenarioLabMetadata, LabScenarioPayload, LabVisionMode, RoomTimeState, ServerMessage,
    LAB_REPLAY_ARTIFACT_KIND, LAB_REPLAY_ARTIFACT_SCHEMA, LAB_REPLAY_ARTIFACT_SCHEMA_VERSION,
    LAB_REPLAY_MAX_AUTHORING_NAME_BYTES, LAB_REPLAY_TIMELINE_KEYFRAME_INTERVAL_TICKS,
};

pub(super) enum LabReplayRebaseSource {
    Checkpoint(Box<LabCheckpointScenarioV1>),
}

#[cfg_attr(not(test), allow(dead_code))]
fn apply_lab_replay_operation(
    game: &mut Game,
    replay_entry: &LabReplayOperationEntry,
) -> Result<LabTimelineEntryKind, String> {
    let entry_kind = lab_replay_operation_to_entry_kind(&replay_entry.op)?;
    match &entry_kind {
        LabTimelineEntryKind::LabOperation { op_kind, op } => {
            game.apply_lab_op(op.clone()).map(|_| ()).map_err(|err| {
                format!(
                    "Lab replay operation {op_kind} failed at sequence {} request {}: {}.",
                    replay_entry.sequence,
                    replay_entry.request_id,
                    lab_error_text(&err)
                )
            })?
        }
        LabTimelineEntryKind::IssueCommandAs {
            player_id,
            command,
            options,
        } => game
            .issue_lab_command_as(*player_id, command.clone(), *options)
            .map_err(|err| {
                format!(
                    "Lab replay issue-as failed at sequence {} request {}: {}.",
                    replay_entry.sequence,
                    replay_entry.request_id,
                    lab_error_text(&err)
                )
            })?,
    }
    Ok(entry_kind)
}

#[cfg_attr(not(test), allow(dead_code))]
fn truncate_lab_replay_name(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if out.len() + ch.len_utf8() > LAB_REPLAY_MAX_AUTHORING_NAME_BYTES {
            break;
        }
        out.push(ch);
    }
    out
}

impl RoomTask {
    pub(in crate::lobby::room_task) fn apply_lab_scenario_actions(&mut self) {
        let actions = match (&mut self.lab_driver, &self.phase) {
            (Some(driver), Phase::InGame(game)) => driver.actions_for_tick(game),
            _ => return,
        };
        let Some(operator_id) = self.lab_session.as_ref().map(|session| session.operator_id) else {
            return;
        };
        for action in actions {
            let result = match action {
                LabScenarioAction::Command(command) => self.apply_lab_issue_command(
                    command.request_id,
                    operator_id,
                    command.player_id,
                    command.command,
                    command.options,
                ),
                LabScenarioAction::LabOperation { request_id, op } => {
                    let op_kind = lab_op_to_replay_operation(&op)
                        .as_ref()
                        .map(lab_replay_operation_kind)
                        .unwrap_or("unserializable");
                    self.apply_and_record_lab_operation(
                        operator_id,
                        request_id,
                        op_kind.to_string(),
                        op,
                    )
                }
            };
            if !result.ok {
                crate::log_warn!(room = %self.room, op = %result.op, error = ?result.error,
                    "lab scenario action rejected");
            }
        }
    }

    pub(super) fn apply_lab_issue_command(
        &mut self,
        request_id: u32,
        operator_id: u32,
        command_player_id: u32,
        cmd: Command,
        options: LabCommandOptions,
    ) -> LabResult {
        let op = "issueCommandAs".to_string();
        let log_operations = self.session_policy().logs_lab_operations();
        let timeline_capacity_reset = match self.lab_timeline_entry_cap_reset() {
            Ok(reset) => reset,
            Err(err) => return lab_result_error(request_id, op, &err),
        };
        let tick = {
            let Some(game) = self.live_game_mut() else {
                return lab_result_error(request_id, op, "lab game is not running");
            };
            if let Err(err) = game.issue_lab_command_as(command_player_id, cmd.clone(), options) {
                return lab_result_error(request_id, op, &lab_error_text(&err));
            }
            game.tick_count()
        };
        let mut timeline_truncated = false;
        if let Some(timeline) = &mut self.lab_timeline {
            if let Some((game, initial_setup)) = timeline_capacity_reset.as_ref() {
                timeline.reset(game, initial_setup.clone());
            } else {
                timeline_truncated = timeline.truncate_future(tick);
            }
            timeline.record_issue_command_as(
                tick,
                request_id,
                operator_id,
                command_player_id,
                cmd,
                options,
            );
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
            failed_index: None,
            details: None,
            outcome: Some(serde_json::json!({
                "accepted": true,
                "admission": "enqueued",
                "playerId": command_player_id,
                "queuedAtTick": tick,
            })),
        }
    }

    pub(super) fn export_lab_replay_initial_setup(
        &self,
        game: &Game,
        name: String,
    ) -> Result<LabCheckpointScenarioV1, String> {
        let vision = self
            .lab_session
            .as_ref()
            .map(|session| session.default_vision.clone())
            .unwrap_or(LabVisionMode::All);
        let initial_camera = self
            .lab_session
            .as_ref()
            .and_then(|session| session.initial_camera);
        export_lab_checkpoint_scenario_for_protocol(
            game,
            name,
            LabScenarioLabMetadata {
                vision,
                god_mode_players: game.lab_god_mode_players(),
                initial_camera,
            },
            crate::build_info::build_id(),
        )
    }

    pub(super) fn lab_replay_initial_setup_for_rebase(
        &self,
        source: LabReplayRebaseSource,
        _outcome: &LabOpOutcome,
    ) -> Result<LabCheckpointScenarioV1, String> {
        match source {
            LabReplayRebaseSource::Checkpoint(scenario) => Ok(*scenario),
        }
    }

    pub(super) fn lab_timeline_entry_cap_reset(
        &self,
    ) -> Result<Option<(Game, LabCheckpointScenarioV1)>, String> {
        if !self
            .lab_timeline
            .as_ref()
            .is_some_and(LabTimeline::is_entry_cap_reached)
        {
            return Ok(None);
        }
        let Some(game) = self.live_game().map(Game::clone_for_replay_keyframe) else {
            return Ok(None);
        };
        let initial_setup = self.export_lab_replay_initial_setup(
            &game,
            "Lab replay rebased at entry cap".to_string(),
        )?;
        Ok(Some((game, initial_setup)))
    }

    pub(in crate::lobby::room_task) fn send_lab_room_time_state_to(&self, player_id: u32) {
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

    pub(in crate::lobby::room_task) fn broadcast_lab_room_time_state(&self) {
        let Some(state) = self.lab_room_time_state() else {
            return;
        };
        self.broadcast(&ServerMessage::RoomTimeState(state));
    }

    pub(in crate::lobby::room_task) fn lab_room_time_control_allowed(
        &self,
        player_id: u32,
    ) -> bool {
        self.lab_session
            .as_ref()
            .map(|session| session.can_operate(player_id))
            .unwrap_or(false)
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

    pub(in crate::lobby::room_task) fn on_seek_lab_room_time(
        &mut self,
        player_id: u32,
        target: LabSeekTarget,
    ) {
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
                if let (Some(driver), Some(timeline)) = (&mut self.lab_driver, &self.lab_timeline) {
                    driver.sync_to_tick(seek.target_tick, timeline.replay_entries());
                }
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
            LabTimelineEntryKind::IssueCommandAs {
                player_id,
                command,
                options,
            } => game
                .issue_lab_command_as(*player_id, command.clone(), *options)
                .map_err(|err| {
                    format!(
                        "Lab timeline issue-as failed at sequence {} request {}: {err:?}.",
                        entry.sequence, entry.request_id
                    )
                }),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::lobby::room_task) fn export_lab_replay_artifact(
        &self,
        operator_id: u32,
        name: Option<&str>,
    ) -> Result<LabReplayArtifactV1, String> {
        let Some(game) = self.live_game() else {
            return Err("lab game is not running".to_string());
        };
        let Some(session) = &self.lab_session else {
            return Err("lab session is not running".to_string());
        };
        let Some(timeline) = &self.lab_timeline else {
            return Err("lab replay timeline is not available".to_string());
        };
        let replay_name = name
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(truncate_lab_replay_name)
            .unwrap_or_else(|| "Untitled lab replay".to_string());
        let mut initial_setup = timeline.initial_setup().clone();
        initial_setup.metadata.lab.vision = session.vision_for(operator_id);
        initial_setup.metadata.lab.initial_camera = session.initial_camera;
        let artifact = LabReplayArtifactV1 {
            schema: LAB_REPLAY_ARTIFACT_SCHEMA.to_string(),
            schema_version: LAB_REPLAY_ARTIFACT_SCHEMA_VERSION,
            kind: LAB_REPLAY_ARTIFACT_KIND.to_string(),
            server_build_sha: crate::build_info::build_id().to_string(),
            authoring: LabReplayAuthoringMetadata {
                name: replay_name,
                author: None,
                created_at_unix_ms: Some(current_unix_ms()),
                description: None,
                tags: Vec::new(),
            },
            timeline: LabReplayTimelineMetadata {
                initial_tick: initial_setup.metadata.exported_tick,
                duration_ticks: timeline.duration_ticks(game.tick_count()),
                keyframe_interval_ticks: LAB_REPLAY_TIMELINE_KEYFRAME_INTERVAL_TICKS,
            },
            initial_setup,
            operations: timeline.replay_entries().to_vec(),
        };
        let bytes = serde_json::to_vec(&artifact)
            .map_err(|err| format!("lab replay export failed: {err}"))?;
        lab_replay_artifact_from_slice(&bytes)
            .map_err(|err| format!("lab replay export validation failed: {err}"))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::lobby::room_task) fn load_lab_replay_artifact(
        &mut self,
        operator_id: u32,
        artifact: LabReplayArtifactV1,
    ) -> Result<(), String> {
        self.load_lab_replay_artifact_with_deadline(operator_id, artifact, None)
    }

    pub(in crate::lobby::room_task) fn load_lab_replay_artifact_before(
        &mut self,
        operator_id: u32,
        artifact: LabReplayArtifactV1,
        deadline: Instant,
    ) -> Result<(), String> {
        self.load_lab_replay_artifact_with_deadline(operator_id, artifact, Some(deadline))
    }

    fn load_lab_replay_artifact_with_deadline(
        &mut self,
        operator_id: u32,
        artifact: LabReplayArtifactV1,
        deadline: Option<Instant>,
    ) -> Result<(), String> {
        let bytes = serde_json::to_vec(&artifact)
            .map_err(|err| format!("lab replay artifact could not be serialized: {err}"))?;
        let artifact = lab_replay_artifact_from_slice(&bytes)
            .map_err(|err| format!("lab replay artifact rejected: {err}"))?;
        let (game, timeline) = Self::rebuild_lab_replay_artifact(&artifact, deadline)?;
        self.phase = Phase::InGame(Box::new(game));
        self.lab_timeline = Some(timeline);
        self.lab_driver = None;
        if let Some(session) = &mut self.lab_session {
            session.import_vision_for(operator_id, artifact.initial_setup.metadata.lab.vision);
            session.initial_camera = artifact.initial_setup.metadata.lab.initial_camera;
            session.dirty = false;
            session.operation_log.clear();
        }
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn rebuild_lab_replay_artifact(
        artifact: &LabReplayArtifactV1,
        deadline: Option<Instant>,
    ) -> Result<(Game, LabTimeline), String> {
        ensure_rebuild_before(deadline)?;
        let lab_op = lab_scenario_payload_to_lab_op(LabScenarioPayload::Checkpoint(
            artifact.initial_setup.clone(),
        ))?;
        let LabOp::RestoreCheckpointScenario(scenario) = lab_op else {
            return Err(
                "lab replay initial setup did not produce a checkpoint restore".to_string(),
            );
        };
        let mut game =
            Game::restore_lab_checkpoint_scenario(*scenario).map_err(|err| lab_error_text(&err))?;
        let mut timeline = LabTimeline::new(&game, artifact.initial_setup.clone());
        for replay_entry in &artifact.operations {
            ensure_rebuild_before(deadline)?;
            if replay_entry.tick < game.tick_count() {
                return Err(format!(
                    "Lab replay operation {} is out of order: tick {} before {}.",
                    replay_entry.sequence,
                    replay_entry.tick,
                    game.tick_count()
                ));
            }
            while game.tick_count() < replay_entry.tick {
                ensure_rebuild_before(deadline)?;
                game.tick();
                timeline.record_keyframe_if_due(&game);
            }
            let entry_kind = apply_lab_replay_operation(&mut game, replay_entry)?;
            timeline.record_replayed_entry(replay_entry.clone(), entry_kind)?;
        }
        while game.tick_count() < artifact.timeline.duration_ticks {
            ensure_rebuild_before(deadline)?;
            game.tick();
            timeline.record_keyframe_if_due(&game);
        }
        Ok((game, timeline))
    }
}

fn ensure_rebuild_before(deadline: Option<Instant>) -> Result<(), String> {
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return Err("Lab replay import timed out before changing the room.".to_string());
    }
    Ok(())
}
