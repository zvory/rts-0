use super::room_task::{DevScenarioConfig, DevScenarioId, DevSelfPlayConfig, RoomMode};
use super::*;

pub(super) fn room_mode_for(room: &str) -> RoomMode {
    if room == format!("{DEV_SELFPLAY_ROOM_PREFIX}live") {
        return RoomMode::DevSelfPlay(DevSelfPlayConfig::Live);
    }
    if let Some(artifact) = room.strip_prefix(&format!("{DEV_SELFPLAY_ROOM_PREFIX}replay:")) {
        return RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay {
            artifact: artifact.to_string(),
        });
    }
    if let Some(raw) = room.strip_prefix(DEV_SCENARIO_ROOM_PREFIX) {
        if let Some((id, cars)) = raw.split_once(":cars=") {
            if id == "scout_car_snaking_corridor" {
                if let Ok(cars) = cars.parse::<usize>() {
                    if matches!(cars, 1 | 4) {
                        return RoomMode::DevScenario(DevScenarioConfig {
                            id: DevScenarioId::ScoutCarSnakingCorridor,
                            cars,
                        });
                    }
                }
            }
        }
    }
    RoomMode::Normal
}

pub(super) fn match_seed() -> u32 {
    if let Ok(raw) = std::env::var(MATCH_SEED_ENV) {
        match raw.parse::<u32>() {
            Ok(seed) => return seed,
            Err(err) => warn!(
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

pub(super) fn load_replay_artifact(name: &str) -> Result<ReplayArtifact, String> {
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
            return serde_json::from_str(&json)
                .map_err(|e| format!("failed to parse replay artifact: {e}"));
        }
    }
    Err(format!(
        "failed to read replay artifact {name:?} from target/selfplay-artifacts or target/selfplay-failures"
    ))
}
