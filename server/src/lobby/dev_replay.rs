use super::room_task::{DevScenarioConfig, DevScenarioId, RoomMode};
use super::*;
use crate::dev_scenarios::parse_dev_scenario_room;
use rts_sim::game::replay::REPLAY_ARTIFACT_SCHEMA_VERSION_V2;

pub(super) fn room_mode_for(room: &str) -> RoomMode {
    if let Some(artifact) = room.strip_prefix(REPLAY_ARTIFACT_ROOM_PREFIX) {
        return RoomMode::ReplayArtifact {
            artifact: artifact.to_string(),
        };
    }
    if let Some(raw) = room.strip_prefix(DEV_SCENARIO_ROOM_PREFIX) {
        if let Some(launch) = parse_dev_scenario_room(raw) {
            return RoomMode::DevScenario(DevScenarioConfig {
                id: match launch.id {
                    "scout_car_snaking_corridor" => DevScenarioId::ScoutCarSnakingCorridor,
                    "direct_reverse_order" => DevScenarioId::DirectReverseOrder,
                    "scout_car_wall_chokepoint" => DevScenarioId::ScoutCarWallChokepoint,
                    "vehicle_corner_wall" => DevScenarioId::VehicleCornerWall,
                    "vehicle_small_block_baseline" => DevScenarioId::VehicleSmallBlockBaseline,
                    "factory_zero_gap_perpendicular" => DevScenarioId::FactoryZeroGapPerpendicular,
                    "tank_trap_line_horizontal" => DevScenarioId::TankTrapLineHorizontal,
                    "tank_trap_line_vertical" => DevScenarioId::TankTrapLineVertical,
                    "tank_trap_line_diagonal" => DevScenarioId::TankTrapLineDiagonal,
                    "tank_trap_pathing_matrix" => DevScenarioId::TankTrapPathingMatrix,
                    _ => return RoomMode::Normal,
                },
                unit: launch.unit,
                count: launch.count,
                blocker: launch.blocker,
                case: launch.case,
            });
        }
    }
    RoomMode::Normal
}

pub(super) fn match_seed() -> u32 {
    if let Ok(raw) = std::env::var(MATCH_SEED_ENV) {
        match raw.parse::<u32>() {
            Ok(seed) => return seed,
            Err(err) => crate::log_warn!(
                env = MATCH_SEED_ENV,
                value = %raw,
                error = %err,
                "invalid match seed override; using time-based seed"
            ),
        }
    }

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u32)
        .unwrap_or(0x1234_5678)
}

pub(super) fn load_replay_artifact(name: &str) -> Result<ReplayArtifactV1, String> {
    if !is_safe_artifact_name(name) {
        return Err("invalid replay artifact name".to_string());
    }
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target");
    let candidates = [
        root.join("selfplay-artifacts")
            .join(name)
            .join("replay.json"),
        root.join("selfplay-failures")
            .join(name)
            .join("replay.json"),
    ];
    for path in candidates {
        if let Ok(json) = fs::read_to_string(&path) {
            let value: serde_json::Value = serde_json::from_str(&json)
                .map_err(|e| format!("failed to parse replay artifact JSON: {e}"))?;
            let Some(schema) = value.get("artifactSchemaVersion") else {
                return Err(
                    "unsupported replay artifact format: expected ReplayArtifactV1 with artifactSchemaVersion"
                        .to_string(),
                );
            };
            if schema.as_u64() != Some(REPLAY_ARTIFACT_SCHEMA_VERSION_V2 as u64) {
                return Err(format!(
                    "unsupported replay artifact schema {}; expected {}",
                    schema, REPLAY_ARTIFACT_SCHEMA_VERSION_V2
                ));
            }
            return serde_json::from_value(value)
                .map_err(|e| format!("failed to parse ReplayArtifactV1: {e}"));
        }
    }
    Err(format!(
        "failed to read replay artifact {name:?} from target/selfplay-artifacts or target/selfplay-failures"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sim::game::replay::ReplayArtifactV1;
    use rts_sim::game::{Game, PlayerInit};

    fn artifact_dir(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("selfplay-artifacts")
            .join(name)
    }

    fn test_artifact_name(suffix: &str) -> String {
        format!("loader_{suffix}_{}", std::process::id())
    }

    fn test_players() -> Vec<PlayerInit> {
        vec![PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Loader".to_string(),
            color: "#ffffff".to_string(),
            is_ai: false,
        }]
    }

    #[test]
    fn load_replay_artifact_accepts_unified_selfplay_artifact() {
        let name = test_artifact_name("unified");
        let dir = artifact_dir(&name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let players = test_players();
        let game = Game::new(&players, 0x1234_5678);
        let artifact = ReplayArtifactV1::capture_from_game(
            &game,
            crate::build_info::build_id(),
            None,
            game.scores(),
        );
        std::fs::write(
            dir.join("replay.json"),
            serde_json::to_vec_pretty(&artifact).unwrap(),
        )
        .unwrap();

        let loaded = load_replay_artifact(&name).unwrap();

        assert_eq!(
            loaded.artifact_schema_version,
            REPLAY_ARTIFACT_SCHEMA_VERSION_V2
        );
        assert_eq!(loaded.command_log, artifact.command_log);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_replay_artifact_rejects_legacy_selfplay_payload() {
        let name = test_artifact_name("legacy");
        let dir = artifact_dir(&name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("replay.json"),
            r#"{"replayCommands":[],"players":[],"seed":1}"#,
        )
        .unwrap();

        let err = load_replay_artifact(&name).unwrap_err();

        assert!(err.contains("unsupported replay artifact format"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
