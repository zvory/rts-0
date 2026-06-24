//! Bundled lab scenario catalog loading and validation.
//!
//! The catalog manifest is the source of truth for scenarios shown in the browser. Each listed
//! scenario must parse as protocol JSON and restore through the public lab `Game` API before it is
//! exposed or used to start a lab room.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use rts_sim::game::lab::{LabError, LabOp, LabScenarioV1 as SimLabScenarioV1};
use rts_sim::game::map::Map;
use rts_sim::game::{Game, PlayerInit};
use serde::{Deserialize, Serialize};

use crate::protocol::{LabScenarioV1, LabVisionMode, TeamId};

const LAB_SCENARIO_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/lab-scenarios");
const LAB_SCENARIO_MANIFEST: &str = "manifest.json";
const LAB_SCENARIO_KIND: &str = "labScenario";
const MAX_SCENARIO_ID_LEN: usize = 48;
const MAX_SCENARIO_FILENAME_LEN: usize = 80;
const MAX_SCENARIO_TITLE_LEN: usize = 96;
const MAX_SCENARIO_DESCRIPTION_LEN: usize = 320;
const MAX_SCENARIO_TAGS: usize = 8;
const MAX_SCENARIO_TAG_LEN: usize = 32;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioCatalogEntry {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub map: String,
    pub player_count: usize,
    pub filename: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LabScenarioManifest {
    scenarios: Vec<LabScenarioCatalogEntry>,
}

#[derive(Serialize)]
struct LabScenarioCatalogError {
    error: String,
}

#[derive(Debug, Clone)]
pub struct LoadedLabScenario {
    pub entry: LabScenarioCatalogEntry,
    pub scenario: LabScenarioV1,
    sim_scenario: SimLabScenarioV1,
}

impl LoadedLabScenario {
    pub fn build_game(&self) -> Result<Game, String> {
        build_game_from_scenario(&self.scenario, self.sim_scenario.clone())
    }
}

pub fn load_lab_scenario_catalog() -> Result<Vec<LabScenarioCatalogEntry>, String> {
    load_lab_scenario_catalog_from_dir(&default_lab_scenario_dir())
}

/// GET /api/lab-scenarios - bounded metadata for bundled lab scenarios.
pub async fn catalog_handler() -> impl IntoResponse {
    match load_lab_scenario_catalog() {
        Ok(entries) => Json(entries).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(LabScenarioCatalogError {
                error: format!("Lab scenario catalog unavailable: {err}"),
            }),
        )
            .into_response(),
    }
}

pub fn load_lab_scenario_by_id(id: &str) -> Result<LoadedLabScenario, String> {
    if !safe_scenario_id(id) {
        return Err("invalid lab scenario id".to_string());
    }
    let root = default_lab_scenario_dir();
    let entries = load_lab_scenario_catalog_from_dir(&root)?;
    let entry = entries
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| format!("unknown lab scenario id {id:?}"))?;
    let loaded = load_lab_scenario_entry(&root, &entry)?;
    validate_entry_matches_scenario(&entry, &loaded.scenario)?;
    Ok(loaded)
}

pub fn lab_scenario_exists(id: &str) -> bool {
    safe_scenario_id(id)
        && load_lab_scenario_manifest_entries_from_dir(&default_lab_scenario_dir())
            .map(|entries| entries.iter().any(|entry| entry.id == id))
            .unwrap_or(false)
}

fn default_lab_scenario_dir() -> PathBuf {
    PathBuf::from(LAB_SCENARIO_DIR)
}

fn load_lab_scenario_catalog_from_dir(root: &Path) -> Result<Vec<LabScenarioCatalogEntry>, String> {
    let entries = load_lab_scenario_manifest_entries_from_dir(root)?;

    for entry in &entries {
        let loaded = load_lab_scenario_entry(root, entry)?;
        validate_entry_matches_scenario(entry, &loaded.scenario)?;
        loaded.build_game().map_err(|err| {
            format!(
                "lab scenario {:?} does not restore through Game lab APIs: {err}",
                entry.id
            )
        })?;
    }

    Ok(entries)
}

fn load_lab_scenario_manifest_entries_from_dir(
    root: &Path,
) -> Result<Vec<LabScenarioCatalogEntry>, String> {
    let manifest_path = root.join(LAB_SCENARIO_MANIFEST);
    let manifest_json = std::fs::read_to_string(&manifest_path)
        .map_err(|err| format!("failed to read lab scenario manifest: {err}"))?;
    let manifest: LabScenarioManifest = serde_json::from_str(&manifest_json)
        .map_err(|err| format!("failed to parse lab scenario manifest: {err}"))?;
    if manifest.scenarios.is_empty() {
        return Err("lab scenario manifest must contain at least one scenario".to_string());
    }

    let mut seen_ids = HashSet::new();
    let mut seen_files = HashSet::new();
    for entry in &manifest.scenarios {
        validate_manifest_entry(entry)?;
        if !seen_ids.insert(entry.id.clone()) {
            return Err(format!("duplicate lab scenario id {:?}", entry.id));
        }
        if !seen_files.insert(entry.filename.clone()) {
            return Err(format!(
                "duplicate lab scenario filename {:?}",
                entry.filename
            ));
        }
    }

    Ok(manifest.scenarios)
}

fn load_lab_scenario_entry(
    root: &Path,
    entry: &LabScenarioCatalogEntry,
) -> Result<LoadedLabScenario, String> {
    let path = root.join(&entry.filename);
    let json = std::fs::read_to_string(&path).map_err(|err| {
        format!(
            "failed to read lab scenario {:?} from {:?}: {err}",
            entry.id, entry.filename
        )
    })?;
    let scenario: LabScenarioV1 = serde_json::from_str(&json)
        .map_err(|err| format!("invalid lab scenario {:?} JSON: {err}", entry.id))?;
    validate_lab_scenario_vision(&scenario.metadata.lab.vision, &scenario.players)?;
    let sim_scenario = protocol_scenario_to_sim(&scenario)
        .map_err(|err| format!("invalid lab scenario {:?} payload: {err}", entry.id))?;
    Ok(LoadedLabScenario {
        entry: entry.clone(),
        scenario,
        sim_scenario,
    })
}

fn protocol_scenario_to_sim(scenario: &LabScenarioV1) -> Result<SimLabScenarioV1, String> {
    serde_json::from_value(
        serde_json::to_value(scenario).map_err(|err| format!("serialize failed: {err}"))?,
    )
    .map_err(|err| err.to_string())
}

fn validate_manifest_entry(entry: &LabScenarioCatalogEntry) -> Result<(), String> {
    if !safe_scenario_id(&entry.id) {
        return Err(format!("invalid lab scenario id {:?}", entry.id));
    }
    if !safe_scenario_filename(&entry.filename) {
        return Err(format!(
            "invalid lab scenario filename {:?}",
            entry.filename
        ));
    }
    if entry.title.trim().is_empty() || entry.title.len() > MAX_SCENARIO_TITLE_LEN {
        return Err(format!("invalid title for lab scenario {:?}", entry.id));
    }
    if entry.description.trim().is_empty() || entry.description.len() > MAX_SCENARIO_DESCRIPTION_LEN
    {
        return Err(format!(
            "invalid description for lab scenario {:?}",
            entry.id
        ));
    }
    if entry.tags.len() > MAX_SCENARIO_TAGS {
        return Err(format!("too many tags for lab scenario {:?}", entry.id));
    }
    for tag in &entry.tags {
        if !safe_scenario_tag(tag) {
            return Err(format!(
                "invalid tag {:?} for lab scenario {:?}",
                tag, entry.id
            ));
        }
    }
    if entry.map.trim().is_empty() {
        return Err(format!("missing map for lab scenario {:?}", entry.id));
    }
    if entry.player_count == 0 {
        return Err(format!(
            "playerCount must be nonzero for lab scenario {:?}",
            entry.id
        ));
    }
    Ok(())
}

fn validate_entry_matches_scenario(
    entry: &LabScenarioCatalogEntry,
    scenario: &LabScenarioV1,
) -> Result<(), String> {
    if scenario.kind != LAB_SCENARIO_KIND {
        return Err(format!(
            "lab scenario {:?} kind must be {:?}",
            entry.id, LAB_SCENARIO_KIND
        ));
    }
    if scenario.map.name != entry.map {
        return Err(format!(
            "lab scenario {:?} manifest map {:?} does not match JSON map {:?}",
            entry.id, entry.map, scenario.map.name
        ));
    }
    if scenario.players.len() != entry.player_count {
        return Err(format!(
            "lab scenario {:?} manifest playerCount {} does not match JSON player count {}",
            entry.id,
            entry.player_count,
            scenario.players.len()
        ));
    }
    Ok(())
}

fn build_game_from_scenario(
    scenario: &LabScenarioV1,
    sim_scenario: SimLabScenarioV1,
) -> Result<Game, String> {
    let inits: Vec<_> = scenario
        .players
        .iter()
        .map(|player| PlayerInit {
            id: player.id,
            team_id: player.team_id,
            faction_id: player.faction_id.clone(),
            name: player.name.clone(),
            color: player.color.clone(),
            is_ai: player.is_ai,
        })
        .collect();
    let start_players: Vec<_> = inits
        .iter()
        .map(|player| (player.id, normalize_team_id(player.id, player.team_id)))
        .collect();
    let map_metadata = Map::metadata_for_name(&scenario.map.name).map_err(|err| {
        format!(
            "Cannot load lab scenario map {:?}: {err}",
            scenario.map.name
        )
    })?;
    let map = Map::load_for_players(&scenario.map.name, &start_players, scenario.seed).map_err(
        |err| {
            format!(
                "Cannot load lab scenario map {:?}: {err}",
                scenario.map.name
            )
        },
    )?;
    let mut game = Game::new_lab(&inits, scenario.seed, map, map_metadata);
    game.apply_lab_op(LabOp::RestoreScenario(Box::new(sim_scenario)))
        .map_err(|err| lab_error_text(&err))?;
    Ok(game)
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

fn safe_scenario_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SCENARIO_ID_LEN
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn safe_scenario_filename(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SCENARIO_FILENAME_LEN
        && value.ends_with(".json")
        && !value.contains("..")
        && value != LAB_SCENARIO_MANIFEST
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
}

fn safe_scenario_tag(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SCENARIO_TAG_LEN
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn normalize_team_id(player_id: u32, team_id: TeamId) -> TeamId {
    if team_id == 0 {
        player_id
    } else {
        team_id
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_catalog_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rts-lab-catalog-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn bundled_lab_scenario_catalog_loads_lategame_and_restores() {
        let catalog = load_lab_scenario_catalog().expect("bundled lab catalog should load");
        let lategame = catalog
            .iter()
            .find(|entry| entry.id == "lategame")
            .expect("lategame catalog row");
        assert_eq!(lategame.map, "Default");
        assert_eq!(lategame.player_count, 2);
        assert_eq!(lategame.filename, "lategame.json");

        let loaded =
            load_lab_scenario_by_id("lategame").expect("bundled lategame scenario should load");
        let game = loaded
            .build_game()
            .expect("lategame scenario should restore through lab APIs");
        let scenario = game.export_lab_scenario();
        assert_eq!(scenario.seed, 3_566_641_871);
        assert_eq!(scenario.players.len(), 2);
        assert_eq!(scenario.entities.len(), 227);
    }

    #[test]
    fn lab_scenario_catalog_rejects_duplicate_ids_before_reading_json() {
        let dir = temp_catalog_dir("duplicate");
        std::fs::write(
            dir.join(LAB_SCENARIO_MANIFEST),
            r#"{
              "scenarios": [
                {"id":"dupe","title":"One","description":"First","tags":[],"map":"Default","playerCount":2,"filename":"one.json"},
                {"id":"dupe","title":"Two","description":"Second","tags":[],"map":"Default","playerCount":2,"filename":"two.json"}
              ]
            }"#,
        )
        .unwrap();

        let err = load_lab_scenario_catalog_from_dir(&dir).expect_err("duplicate id should reject");
        assert!(
            err.contains("duplicate lab scenario id"),
            "unexpected error: {err}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn lab_scenario_catalog_rejects_unsafe_ids_before_reading_json() {
        let dir = temp_catalog_dir("unsafe");
        std::fs::write(
            dir.join(LAB_SCENARIO_MANIFEST),
            r#"{
              "scenarios": [
                {"id":"../bad","title":"Bad","description":"Bad id","tags":[],"map":"Default","playerCount":2,"filename":"bad.json"}
              ]
            }"#,
        )
        .unwrap();

        let err = load_lab_scenario_catalog_from_dir(&dir).expect_err("unsafe id should reject");
        assert!(
            err.contains("invalid lab scenario id"),
            "unexpected error: {err}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
