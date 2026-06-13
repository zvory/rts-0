//! Deterministic command-log replay for the simulation.
//!
//! A live [`Game`] records commands at the tick where they are applied, after AI controllers have
//! emitted their ordinary commands. Replays feed that exact log into a fresh game with AI thinking
//! disabled, so the log is the only source of player intent.

use serde::{Deserialize, Serialize};

use super::{Game, MapMetadata, PlayerInit, StartingLoadout};
use crate::game::command::SimCommand;
use crate::protocol::{Command, Event, PlayerScore, ReplayStartMetadata, Snapshot};

pub const REPLAY_ARTIFACT_SCHEMA_VERSION_V1: u32 = 1;

/// One authoritative gameplay command, stamped with the simulation tick that applied it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandLogEntry {
    pub tick: u32,
    pub player_id: u32,
    pub command: Command,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReplayStartingLoadoutMode {
    Standard,
    DebugHuman,
}

impl From<StartingLoadout> for ReplayStartingLoadoutMode {
    fn from(value: StartingLoadout) -> Self {
        match value {
            StartingLoadout::Standard => ReplayStartingLoadoutMode::Standard,
            StartingLoadout::DebugHuman => ReplayStartingLoadoutMode::DebugHuman,
        }
    }
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
    pub starting_steel: u32,
    pub starting_oil: u32,
    pub starting_loadout_mode: ReplayStartingLoadoutMode,
    pub players: Vec<PlayerInit>,
    pub duration_ticks: u32,
    pub command_log: Vec<CommandLogEntry>,
    pub winner_id: Option<u32>,
    #[serde(default)]
    pub winner_team_id: Option<u32>,
    pub final_scores: Vec<PlayerScore>,
}

impl ReplayArtifactV1 {
    pub fn capture_from_game(
        game: &Game,
        server_build_sha: impl Into<String>,
        winner_id: Option<u32>,
        final_scores: Vec<PlayerScore>,
    ) -> Self {
        let map = game.map_metadata();
        ReplayArtifactV1 {
            artifact_schema_version: REPLAY_ARTIFACT_SCHEMA_VERSION_V1,
            server_build_sha: server_build_sha.into(),
            map_name: map.name.clone(),
            map_schema_version: map.schema_version,
            map_content_hash: map.content_hash.clone(),
            seed: game.seed(),
            starting_steel: game.starting_steel(),
            starting_oil: game.starting_oil(),
            starting_loadout_mode: game.starting_loadout().into(),
            players: game.player_inits(),
            duration_ticks: game.tick_count(),
            command_log: game.command_log().to_vec(),
            winner_id,
            winner_team_id: winner_id.and_then(|id| game.team_of_player(id)),
            final_scores,
        }
    }

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

    pub fn validate_against(
        &self,
        expected_server_build_sha: &str,
        running_map: &MapMetadata,
    ) -> Result<(), ReplayValidationError> {
        if self.artifact_schema_version != REPLAY_ARTIFACT_SCHEMA_VERSION_V1 {
            return Err(ReplayValidationError::UnsupportedArtifactSchema {
                found: self.artifact_schema_version,
                expected: REPLAY_ARTIFACT_SCHEMA_VERSION_V1,
            });
        }
        if self.server_build_sha != expected_server_build_sha {
            return Err(ReplayValidationError::BuildShaMismatch {
                artifact: self.server_build_sha.clone(),
                running: expected_server_build_sha.to_string(),
            });
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
        Ok(())
    }
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
        }
    }
}

impl std::error::Error for ReplayValidationError {}

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
    starting_steel: u32,
    starting_oil: u32,
) -> Result<ReplayOutcome, ReplayError> {
    let mut replay =
        Game::new_for_replay_with_starting_resources(players, starting_steel, starting_oil, seed);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn players() -> [PlayerInit; 1] {
        [PlayerInit {
            id: 1,
            team_id: 1,
            name: "Replay".into(),
            color: "#fff".into(),
            is_ai: false,
        }]
    }

    #[test]
    fn replay_commands_preserves_explicit_starting_resources() {
        let players = players();
        let outcome = replay_commands(&players, &[], 0, 0x1234_5678, 99_999, 88_888)
            .expect("replay should succeed");
        let snapshot = &outcome.final_snapshots[0].snapshot;

        assert_eq!(snapshot.steel, 99_999);
        assert_eq!(snapshot.oil, 88_888);
    }

    #[test]
    fn replay_artifact_captures_live_game_contract() {
        let players = players();
        let mut game = Game::new_with_starting_resources(&players, 777, 333, 0x1234_5678);
        game.tick();
        let scores = game.scores();

        let artifact = ReplayArtifactV1::capture_from_game(&game, "test-sha", Some(1), scores);

        assert_eq!(
            artifact.artifact_schema_version,
            REPLAY_ARTIFACT_SCHEMA_VERSION_V1
        );
        assert_eq!(artifact.server_build_sha, "test-sha");
        assert_eq!(artifact.map_name, "Default");
        assert_eq!(artifact.seed, 0x1234_5678);
        assert_eq!(artifact.starting_steel, 777);
        assert_eq!(artifact.starting_oil, 333);
        assert_eq!(
            artifact.starting_loadout_mode,
            ReplayStartingLoadoutMode::Standard
        );
        assert_eq!(artifact.players, players);
        assert_eq!(artifact.duration_ticks, 1);
        assert_eq!(artifact.winner_id, Some(1));
        assert_eq!(artifact.winner_team_id, Some(1));
        assert!(!artifact.final_scores.is_empty());
        assert_eq!(artifact.start_metadata().duration_ticks, 1);
    }

    #[test]
    fn replay_artifact_defaults_missing_winner_team_for_old_json() {
        let json = serde_json::json!({
            "artifactSchemaVersion": REPLAY_ARTIFACT_SCHEMA_VERSION_V1,
            "serverBuildSha": "test-sha",
            "mapName": "Default",
            "mapSchemaVersion": 1,
            "mapContentHash": "hash",
            "seed": 1,
            "startingSteel": 75,
            "startingOil": 0,
            "startingLoadoutMode": "standard",
            "players": [{
                "id": 1,
                "name": "Replay",
                "color": "#fff",
                "is_ai": false
            }],
            "durationTicks": 0,
            "commandLog": [],
            "winnerId": 1,
            "finalScores": []
        });

        let artifact: ReplayArtifactV1 = serde_json::from_value(json).unwrap();

        assert_eq!(artifact.players[0].team_id, 0);
        assert_eq!(artifact.winner_id, Some(1));
        assert_eq!(artifact.winner_team_id, None);
    }

    #[test]
    fn replay_validation_rejects_incompatible_build_and_map_metadata() {
        let players = players();
        let game = Game::new(&players, 0x1234_5678);
        let artifact = ReplayArtifactV1::capture_from_game(&game, "sha-a", None, game.scores());
        let map = game.map_metadata().clone();

        assert!(matches!(
            artifact.validate_against("sha-b", &map),
            Err(ReplayValidationError::BuildShaMismatch { .. })
        ));

        let mut missing = map.clone();
        missing.name = "Other".to_string();
        assert!(matches!(
            artifact.validate_against("sha-a", &missing),
            Err(ReplayValidationError::MapMissing { .. })
        ));

        let mut wrong_schema = map.clone();
        wrong_schema.schema_version = wrong_schema.schema_version.saturating_add(1);
        assert!(matches!(
            artifact.validate_against("sha-a", &wrong_schema),
            Err(ReplayValidationError::MapSchemaMismatch { .. })
        ));

        let mut wrong_hash = map;
        wrong_hash.content_hash.push_str("-changed");
        assert!(matches!(
            artifact.validate_against("sha-a", &wrong_hash),
            Err(ReplayValidationError::MapHashMismatch { .. })
        ));
    }
}
