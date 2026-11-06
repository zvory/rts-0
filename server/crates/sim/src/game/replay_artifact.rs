use serde::{Deserialize, Serialize};

use super::{Game, PlayerInit, PlayerStartingLoadout};
use crate::protocol::{ChatChannel, Command, MatchConclusion, PlayerScore};

pub(in crate::game) const REPLAY_ARTIFACT_SCHEMA_VERSION_V3: u32 = 3;
pub const REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION: u32 = REPLAY_ARTIFACT_SCHEMA_VERSION_V3;

pub fn is_supported_replay_artifact_schema(version: u32) -> bool {
    version == REPLAY_ARTIFACT_SCHEMA_VERSION_V3
}

/// One authoritative gameplay command, stamped with the simulation tick that applied it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandLogEntry {
    pub tick: u32,
    pub player_id: u32,
    pub command: Command,
}

/// One accepted in-game chat message, stamped for presentation during replay playback.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatLogEntry {
    pub tick: u32,
    pub sender_id: u32,
    pub sender_name: String,
    pub channel: ChatChannel,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(in crate::game) struct ReplayStartStateV1 {
    pub(in crate::game) map_name: String,
    pub(in crate::game) map_schema_version: u32,
    pub(in crate::game) map_content_hash: String,
    pub(in crate::game) seed: u32,
    pub(in crate::game) checkpoint_payload: String,
}

/// Administrative removal applied after the recorded tick, including tick zero.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayElimination {
    pub tick: u32,
    pub player_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayEndStateHash {
    pub algorithm: String,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayArtifactV1 {
    pub artifact_schema_version: u32,
    pub server_build_sha: String,
    pub map_name: String,
    pub map_schema_version: u32,
    pub map_content_hash: String,
    pub seed: u32,
    pub player_loadouts: Vec<PlayerStartingLoadout>,
    pub players: Vec<PlayerInit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::game) start_state: Option<ReplayStartStateV1>,
    pub duration_ticks: u32,
    pub command_log: Vec<CommandLogEntry>,
    #[serde(default)]
    pub chat_log: Vec<ChatLogEntry>,
    pub winner_id: Option<u32>,
    #[serde(default)]
    pub winner_team_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<MatchConclusion>,
    pub final_scores: Vec<PlayerScore>,
    #[serde(default)]
    pub eliminations: Vec<ReplayElimination>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_state_hash: Option<ReplayEndStateHash>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayValidationError {
    UnsupportedArtifactSchema {
        found: u32,
        expected: u32,
    },
    BuildShaMismatch {
        artifact: String,
        running: String,
    },
    MapMissing {
        name: String,
    },
    MapSchemaMismatch {
        map_name: String,
        artifact: u32,
        running: u32,
    },
    MapHashMismatch {
        map_name: String,
        artifact: String,
        running: String,
    },
    CheckpointStartMissing,
    CheckpointStartInvalid {
        reason: String,
    },
    StartStateMismatch {
        field: &'static str,
    },
}

impl std::fmt::Display for ReplayValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayValidationError::UnsupportedArtifactSchema { found, expected } => write!(
                f,
                "unsupported replay artifact schema {found}; expected {expected}"
            ),
            ReplayValidationError::BuildShaMismatch { artifact, running } => write!(
                f,
                "replay was recorded by server build {artifact}; running build is {running}"
            ),
            ReplayValidationError::MapMissing { name } => {
                write!(f, "replay map {name:?} is not available on this server")
            }
            ReplayValidationError::MapSchemaMismatch {
                map_name,
                artifact,
                running,
            } => write!(
                f,
                "replay map {map_name:?} schema is {artifact}; running map schema is {running}"
            ),
            ReplayValidationError::MapHashMismatch {
                map_name,
                artifact,
                running,
            } => write!(
                f,
                "replay map {map_name:?} hash is {artifact}; running map hash is {running}"
            ),
            ReplayValidationError::CheckpointStartMissing => {
                write!(f, "checkpoint-backed replay is missing startState")
            }
            ReplayValidationError::CheckpointStartInvalid { reason } => {
                write!(
                    f,
                    "checkpoint-backed replay start state is invalid: {reason}"
                )
            }
            ReplayValidationError::StartStateMismatch { field } => {
                write!(
                    f,
                    "checkpoint-backed replay startState mismatch for {field}"
                )
            }
        }
    }
}

impl std::error::Error for ReplayValidationError {}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplayStartComposition {
    server_build_sha: String,
    start_state: ReplayStartStateV1,
    start_tick: u32,
    eliminations: Vec<ReplayElimination>,
    player_loadouts: Vec<PlayerStartingLoadout>,
    players: Vec<PlayerInit>,
}

impl ReplayStartComposition {
    pub fn capture(game: &Game, server_build_sha: impl Into<String>) -> Result<Self, String> {
        if !game.state.pending.is_empty() {
            return Err(format!(
                "cannot capture replay start with {} pending commands",
                game.state.pending.len()
            ));
        }
        let server_build_sha = server_build_sha.into();
        let map = game.map_metadata();
        let checkpoint_payload = game
            .checkpoint_payload_text_for_container("replay", &server_build_sha)
            .map_err(|err| err.to_string())?;
        Ok(Self {
            server_build_sha,
            start_state: ReplayStartStateV1 {
                map_name: map.name.clone(),
                map_schema_version: map.schema_version,
                map_content_hash: map.content_hash.clone(),
                seed: game.seed(),
                checkpoint_payload,
            },
            start_tick: game.tick_count(),
            eliminations: Vec::new(),
            player_loadouts: game.starting_loadouts().to_vec(),
            players: game.player_inits(),
        })
    }

    /// Record room-authored give-up/disconnect removal at the current post-tick boundary.
    pub fn record_elimination(&mut self, game: &Game, player_id: u32) {
        self.eliminations.push(ReplayElimination {
            tick: game.tick_count(),
            player_id,
        });
    }

    pub fn finalize(
        &self,
        game: &Game,
        winner_id: Option<u32>,
        final_scores: Vec<PlayerScore>,
    ) -> ReplayArtifactV1 {
        let map = game.map_metadata();
        debug_assert_eq!(self.start_state.map_name, map.name);
        debug_assert_eq!(self.start_state.map_schema_version, map.schema_version);
        debug_assert_eq!(self.start_state.map_content_hash, map.content_hash);
        debug_assert_eq!(self.start_state.seed, game.seed());
        debug_assert!(game.tick_count() >= self.start_tick);
        let started = std::time::Instant::now();
        let end_state_hash = match game.replay_end_state_hash() {
            Ok(digest) => Some(ReplayEndStateHash {
                algorithm: super::checkpoint::END_STATE_HASH_ALGORITHM.to_string(),
                digest,
            }),
            Err(error) => {
                tracing::warn!(%error, "replay end-state hash capture failed");
                None
            }
        };
        tracing::info!(
            hash_us = started.elapsed().as_micros() as u64,
            tick = game.tick_count(),
            captured = end_state_hash.is_some(),
            "replay end-state hash capture finished"
        );
        ReplayArtifactV1 {
            artifact_schema_version: REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION,
            server_build_sha: self.server_build_sha.clone(),
            map_name: self.start_state.map_name.clone(),
            map_schema_version: self.start_state.map_schema_version,
            map_content_hash: self.start_state.map_content_hash.clone(),
            seed: self.start_state.seed,
            player_loadouts: self.player_loadouts.clone(),
            players: self.players.clone(),
            start_state: Some(self.start_state.clone()),
            duration_ticks: game.tick_count(),
            command_log: game
                .command_log()
                .iter()
                .filter(|entry| entry.tick > self.start_tick)
                .cloned()
                .collect(),
            chat_log: Vec::new(),
            winner_id,
            winner_team_id: winner_id.and_then(|id| game.team_of_player(id)),
            conclusion: None,
            final_scores,
            eliminations: self.eliminations.clone(),
            end_state_hash,
        }
    }
}
