use super::dev_scenario_id::DevScenarioId;
use super::room_task::{DevScenarioConfig, LabRoomConfig, RoomMode};
use super::*;
use crate::dev_scenarios::parse_dev_scenario_room;
use crate::lab_scenarios::lab_scenario_exists;
use rts_sim::game::replay::{
    is_supported_replay_artifact_schema, REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION,
};

pub(super) fn room_mode_for(room: &str) -> RoomMode {
    if let Some(raw) = room.strip_prefix(LAB_ROOM_PREFIX) {
        if let Some(config) = parse_lab_room(raw) {
            return RoomMode::Lab(config);
        }
    }
    if let Some(artifact) = room.strip_prefix(REPLAY_ARTIFACT_ROOM_PREFIX) {
        return RoomMode::ReplayArtifact {
            artifact: artifact.to_string(),
        };
    }
    if let Some(raw) = room.strip_prefix(DEV_SCENARIO_ROOM_PREFIX) {
        if let Some(launch) = parse_dev_scenario_room(raw) {
            let Some(id) = DevScenarioId::from_room_id(launch.id) else {
                return RoomMode::Normal;
            };
            return RoomMode::DevScenario(DevScenarioConfig {
                id,
                unit: launch.unit,
                count: launch.count,
                blocker: launch.blocker,
                case: launch.case,
            });
        }
    }
    RoomMode::Normal
}

fn parse_lab_room(raw: &str) -> Option<LabRoomConfig> {
    let mut parts = raw.split(':');
    let public_id = parts.next()?.trim();
    if !safe_lab_token(public_id, 40) {
        return None;
    }
    let mut map_name = "1v1".to_string();
    let mut seed = None;
    let mut scenario = None;
    for part in parts {
        if let Some(map) = part.strip_prefix("map=") {
            if !safe_lab_token(map, 48) {
                return None;
            }
            map_name = map.to_string();
        } else if let Some(raw_seed) = part.strip_prefix("seed=") {
            seed = Some(raw_seed.parse::<u32>().ok()?);
        } else {
            let raw_scenario = part.strip_prefix("scenario=")?;
            if raw_scenario == "blank" {
                scenario = None;
            } else {
                if !safe_lab_token(raw_scenario, 48) {
                    return None;
                }
                if !lab_scenario_exists(raw_scenario) {
                    return None;
                }
                scenario = Some(raw_scenario.to_string());
            }
        }
    }
    Some(LabRoomConfig {
        public_id: public_id.to_string(),
        map_name,
        seed,
        scenario,
        map_draft: None,
    })
}

fn safe_lab_token(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
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
            let Some(schema_version) = schema
                .as_u64()
                .and_then(|version| u32::try_from(version).ok())
            else {
                return Err(format!(
                    "unsupported replay artifact schema {}; expected {}",
                    schema, REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION
                ));
            };
            if !is_supported_replay_artifact_schema(schema_version) {
                return Err(format!(
                    "unsupported replay artifact schema {}; expected {}",
                    schema, REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION
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
    use rts_sim::game::entity::EntityKind;
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
    fn room_mode_for_accepts_bounded_lab_room_config() {
        match room_mode_for("__lab__:sandbox:map=low-econ:seed=12345") {
            RoomMode::Lab(config) => {
                assert_eq!(config.public_id, "sandbox");
                assert_eq!(config.map_name, "low-econ");
                assert_eq!(config.seed, Some(12345));
                assert_eq!(config.scenario, None);
            }
            _ => panic!("safe lab room should parse as lab mode"),
        }
    }

    #[test]
    fn room_mode_for_accepts_default_lab_scenario_preset() {
        match room_mode_for("__lab__:sandbox:scenario=blank") {
            RoomMode::Lab(config) => {
                assert_eq!(config.map_name, "1v1");
                assert_eq!(config.scenario, None);
            }
            _ => panic!("blank lab scenario should use the current default map"),
        }

        match room_mode_for("__lab__:sandbox:map=Default:scenario=lategame") {
            RoomMode::Lab(config) => {
                assert_eq!(config.public_id, "sandbox");
                assert_eq!(config.scenario.as_deref(), Some("lategame"));
            }
            _ => panic!("safe lab scenario room should parse as lab mode"),
        }

        match room_mode_for("__lab__:sandbox:map=Default:scenario=blank") {
            RoomMode::Lab(config) => assert_eq!(config.scenario, None),
            _ => panic!("safe blank lab scenario room should parse as lab mode"),
        }
    }

    #[test]
    fn room_mode_for_rejects_unsafe_lab_room_config() {
        assert!(matches!(
            room_mode_for("__lab__:../../bad:map=Default"),
            RoomMode::Normal
        ));
        assert!(matches!(
            room_mode_for("__lab__:sandbox:map=Low Econ"),
            RoomMode::Normal
        ));
        assert!(matches!(
            room_mode_for("__lab__:sandbox:seed=not-a-number"),
            RoomMode::Normal
        ));
        assert!(matches!(
            room_mode_for("__lab__:sandbox:scenario=unknown"),
            RoomMode::Normal
        ));
    }

    #[test]
    fn room_mode_for_accepts_panzerfaust_dev_scenario_room() {
        match room_mode_for("__dev_scenario__:panzerfaust_target_death:unit=panzerfaust:count=1") {
            RoomMode::DevScenario(config) => {
                assert!(matches!(config.id, DevScenarioId::PanzerfaustTargetDeath));
                assert_eq!(config.unit, EntityKind::Panzerfaust);
                assert_eq!(config.count, 1);
                assert_eq!(config.blocker, None);
                assert_eq!(config.case, None);
            }
            _ => panic!("safe Panzerfaust dev scenario room should parse as dev mode"),
        }
    }

    #[test]
    fn indexed_dev_scenario_ids_all_route_to_dev_mode() {
        let ids: std::collections::BTreeSet<_> = crate::dev_scenarios::all_dev_scenarios()
            .iter()
            .flat_map(|scenario| {
                std::iter::once(scenario.id).chain(scenario.launches.iter().map(|launch| launch.id))
            })
            .collect();

        for id in ids {
            assert!(
                DevScenarioId::from_room_id(id).is_some(),
                "indexed dev scenario {id:?} must map to a room-task scenario id"
            );
        }
    }

    #[test]
    fn load_replay_artifact_accepts_unified_selfplay_artifact() {
        let name = test_artifact_name("unified");
        let dir = artifact_dir(&name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let players = test_players();
        let game = Game::new(&players, 0x1234_5678);
        let replay_start = rts_sim::game::replay::ReplayStartComposition::capture(
            &game,
            crate::build_info::build_id(),
        )
        .unwrap();
        let artifact = replay_start.finalize(&game, None, game.scores());
        std::fs::write(
            dir.join("replay.json"),
            serde_json::to_vec_pretty(&artifact).unwrap(),
        )
        .unwrap();

        let loaded = load_replay_artifact(&name).unwrap();

        assert_eq!(
            loaded.artifact_schema_version,
            REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION
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
