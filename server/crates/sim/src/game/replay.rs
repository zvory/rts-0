//! Deterministic command-log replay for the simulation.
//!
//! A live [`Game`] records commands at the tick where they are applied, after AI controllers have
//! emitted their ordinary commands. Replays feed that exact log into a fresh game with AI thinking
//! disabled, so the log is the only source of player intent.

use serde::{Deserialize, Serialize};

use super::replay_artifact::REPLAY_ARTIFACT_SCHEMA_VERSION_V3;
pub use super::replay_artifact::{
    is_supported_replay_artifact_schema, CommandLogEntry, ReplayArtifactV1,
    ReplayStartComposition, ReplayValidationError, REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION,
};
use super::{Game, Map, MapMetadata, PlayerInit, PlayerStartingLoadout};
use crate::game::command::SimCommand;
use crate::protocol::{Event, ReplayStartMetadata, Snapshot};

impl ReplayArtifactV1 {
    pub fn start_metadata(&self) -> ReplayStartMetadata {
        ReplayStartMetadata {
            artifact_schema_version: self.artifact_schema_version,
            server_build_sha: self.server_build_sha.clone(),
            map_name: self.map_name.clone(),
            map_schema_version: self.map_schema_version,
            map_content_hash: self.map_content_hash.clone(),
            seed: self.seed,
            duration_ticks: self.duration_ticks,
        }
    }

    pub fn restore_start_game(
        &self,
        map: Map,
        map_metadata: MapMetadata,
    ) -> Result<Game, ReplayValidationError> {
        match &self.start_state {
            Some(start_state) if self.artifact_schema_version == REPLAY_ARTIFACT_SCHEMA_VERSION_V3 => {
                if start_state.map_name != self.map_name {
                    return Err(ReplayValidationError::StartStateMismatch {
                        field: "mapName",
                    });
                }
                if start_state.map_schema_version != self.map_schema_version {
                    return Err(ReplayValidationError::StartStateMismatch {
                        field: "mapSchemaVersion",
                    });
                }
                if start_state.map_content_hash != self.map_content_hash {
                    return Err(ReplayValidationError::StartStateMismatch {
                        field: "mapContentHash",
                    });
                }
                if start_state.seed != self.seed {
                    return Err(ReplayValidationError::StartStateMismatch { field: "seed" });
                }
                let game = Game::restore_checkpoint_payload_text(
                    &start_state.checkpoint_payload,
                    map,
                    map_metadata,
                )
                .map_err(|err| ReplayValidationError::CheckpointStartInvalid {
                    reason: err.to_string(),
                })?;
                if game.seed() != self.seed {
                    return Err(ReplayValidationError::StartStateMismatch {
                        field: "checkpointSeed",
                    });
                }
                if game.player_inits() != self.players {
                    return Err(ReplayValidationError::StartStateMismatch { field: "players" });
                }
                if game.starting_loadouts() != self.player_loadouts.as_slice() {
                    return Err(ReplayValidationError::StartStateMismatch {
                        field: "playerLoadouts",
                    });
                }
                Ok(game)
            }
            Some(_) | None if self.artifact_schema_version == REPLAY_ARTIFACT_SCHEMA_VERSION_V3 => {
                Err(ReplayValidationError::CheckpointStartMissing)
            }
            _ => Err(ReplayValidationError::UnsupportedArtifactSchema {
                found: self.artifact_schema_version,
                expected: REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION,
            }),
        }
    }

    pub fn validate_against(
        &self,
        expected_server_build_sha: &str,
        running_map: &MapMetadata,
    ) -> Result<(), ReplayValidationError> {
        if !is_supported_replay_artifact_schema(self.artifact_schema_version) {
            return Err(ReplayValidationError::UnsupportedArtifactSchema {
                found: self.artifact_schema_version,
                expected: REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION,
            });
        }
        if self.artifact_schema_version == REPLAY_ARTIFACT_SCHEMA_VERSION_V3
            && self.start_state.is_none()
        {
            return Err(ReplayValidationError::CheckpointStartMissing);
        }
        if self.map_name != running_map.name {
            return Err(ReplayValidationError::MapMissing {
                name: self.map_name.clone(),
            });
        }
        if self.map_schema_version != running_map.schema_version {
            return Err(ReplayValidationError::MapSchemaMismatch {
                map_name: self.map_name.clone(),
                artifact: self.map_schema_version,
                running: running_map.schema_version,
            });
        }
        if self.map_content_hash != running_map.content_hash {
            return Err(ReplayValidationError::MapHashMismatch {
                map_name: self.map_name.clone(),
                artifact: self.map_content_hash.clone(),
                running: running_map.content_hash.clone(),
            });
        }
        if let Some(start_state) = &self.start_state {
            if start_state.map_name != self.map_name {
                return Err(ReplayValidationError::StartStateMismatch {
                    field: "mapName",
                });
            }
            if start_state.map_schema_version != self.map_schema_version {
                return Err(ReplayValidationError::StartStateMismatch {
                    field: "mapSchemaVersion",
                });
            }
            if start_state.map_content_hash != self.map_content_hash {
                return Err(ReplayValidationError::StartStateMismatch {
                    field: "mapContentHash",
                });
            }
            if start_state.seed != self.seed {
                return Err(ReplayValidationError::StartStateMismatch { field: "seed" });
            }
        }
        if self.server_build_sha != expected_server_build_sha {
            return Err(ReplayValidationError::BuildShaMismatch {
                artifact: self.server_build_sha.clone(),
                running: expected_server_build_sha.to_string(),
            });
        }
        Ok(())
    }
}

/// One transient event emitted during replay, stamped with the tick that produced it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventLogEntry {
    pub tick: u32,
    pub player_id: u32,
    pub event: Event,
}

/// Output from replaying a command log through a fresh [`Game`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayOutcome {
    pub ticks: u32,
    pub events: Vec<EventLogEntry>,
    pub final_snapshots: Vec<PlayerSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub player_id: u32,
    pub snapshot: Snapshot,
}

/// Replay `commands` through tick `ticks`, preserving command order within each tick.
/// Used only by the test harness (`selfplay/`); kept alive for future replay UI.
#[allow(dead_code)]
pub fn replay_commands(
    players: &[PlayerInit],
    commands: &[CommandLogEntry],
    ticks: u32,
    seed: u32,
    starting_loadouts: &[PlayerStartingLoadout],
) -> Result<ReplayOutcome, ReplayError> {
    let mut replay = Game::new_for_replay_with_starting_loadouts(players, starting_loadouts, seed);
    let mut next_command = 0usize;
    let mut events = Vec::new();

    for tick in 1..=ticks {
        while let Some(entry) = commands.get(next_command) {
            if entry.tick < tick {
                return Err(ReplayError::OutOfOrder {
                    index: next_command,
                    tick: entry.tick,
                    previous_tick: tick,
                });
            }
            if entry.tick != tick {
                break;
            }
            replay.enqueue(
                entry.player_id,
                SimCommand::from_protocol(entry.command.clone()),
            );
            next_command += 1;
        }

        for (player_id, player_events) in replay.tick() {
            for event in player_events {
                events.push(EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    if let Some(entry) = commands.get(next_command) {
        return Err(ReplayError::CommandAfterEnd {
            index: next_command,
            tick: entry.tick,
            replay_ticks: ticks,
        });
    }

    let final_snapshots = players
        .iter()
        .map(|p| PlayerSnapshot {
            player_id: p.id,
            snapshot: replay.snapshot_for(p.id),
        })
        .collect();

    Ok(ReplayOutcome {
        ticks,
        events,
        final_snapshots,
    })
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum ReplayError {
    OutOfOrder {
        index: usize,
        tick: u32,
        previous_tick: u32,
    },
    CommandAfterEnd {
        index: usize,
        tick: u32,
        replay_ticks: u32,
    },
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::OutOfOrder {
                index,
                tick,
                previous_tick,
            } => write!(
                f,
                "command log entry {index} has tick {tick}, before replay cursor {previous_tick}"
            ),
            ReplayError::CommandAfterEnd {
                index,
                tick,
                replay_ticks,
            } => write!(
                f,
                "command log entry {index} has tick {tick}, beyond replay length {replay_ticks}"
            ),
        }
    }
}

impl std::error::Error for ReplayError {}
