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

use crate::protocol::{LabScenarioAuthoringMetadata, LabScenarioV1, LabVisionMode, TeamId};

const LAB_SCENARIO_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/lab-scenarios");
const LAB_SCENARIO_MANIFEST: &str = "manifest.json";
const LAB_SCENARIO_KIND: &str = "labScenario";
const MAX_SCENARIO_ID_LEN: usize = 48;
const MAX_SCENARIO_FILENAME_LEN: usize = 80;
const MAX_SCENARIO_TITLE_LEN: usize = 96;
const MAX_SCENARIO_DESCRIPTION_LEN: usize = 320;
const MAX_SCENARIO_TAGS: usize = 8;
const MAX_SCENARIO_TAG_LEN: usize = 32;
const MAX_SCENARIO_NAME_LEN: usize = 80;
const MAX_REVIEW_NOTES_LEN: usize = 2000;
const MAX_SCENARIO_CATALOG_ENTRIES: usize = 256;
const MAX_AUTHORING_SCENARIO_ENTITIES: usize = 2000;
const MAX_AUTHORING_SCENARIO_JSON_BYTES: usize = 1_000_000;

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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioAuthoringPreview {
    pub slug: String,
    pub filename: String,
    pub scenario_path: String,
    pub manifest_path: String,
    pub manifest_entry: LabScenarioCatalogEntry,
    pub scenario_json: String,
    pub review_notes: String,
    pub summary: String,
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

pub fn validate_lab_scenario_authoring(
    metadata: LabScenarioAuthoringMetadata,
    mut scenario: LabScenarioV1,
) -> Result<LabScenarioAuthoringPreview, String> {
    let slug = metadata.slug.trim().to_string();
    let name = metadata.name.trim().to_string();
    let title = metadata.title.trim().to_string();
    let description = metadata.description.trim().to_string();
    let tags: Vec<_> = metadata
        .tags
        .into_iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .collect();
    let review_notes = metadata.review_notes.unwrap_or_default().trim().to_string();

    if name.is_empty() || name.len() > MAX_SCENARIO_NAME_LEN {
        return Err(format!(
            "scenario name must be non-empty and at most {MAX_SCENARIO_NAME_LEN} bytes"
        ));
    }
    if review_notes.len() > MAX_REVIEW_NOTES_LEN {
        return Err(format!(
            "review notes must be at most {MAX_REVIEW_NOTES_LEN} bytes"
        ));
    }

    scenario.name = name;
    if scenario.entities.len() > MAX_AUTHORING_SCENARIO_ENTITIES {
        return Err(format!(
            "scenario has too many entities: {} > {MAX_AUTHORING_SCENARIO_ENTITIES}",
            scenario.entities.len()
        ));
    }
    let filename = format!("{slug}.json");
    let entry = LabScenarioCatalogEntry {
        id: slug.clone(),
        title,
        description,
        tags,
        map: scenario.map.name.clone(),
        player_count: scenario.players.len(),
        filename: filename.clone(),
    };
    validate_manifest_entry(&entry)?;

    let root = default_lab_scenario_dir();
    let existing_entries = load_lab_scenario_manifest_entries_from_dir(&root)?;
    if existing_entries
        .iter()
        .any(|existing| existing.id == entry.id)
    {
        return Err(format!("duplicate lab scenario id {:?}", entry.id));
    }
    if existing_entries
        .iter()
        .any(|existing| existing.filename == entry.filename)
    {
        return Err(format!(
            "duplicate lab scenario filename {:?}",
            entry.filename
        ));
    }

    validate_entry_matches_scenario(&entry, &scenario)?;
    validate_lab_scenario_lab_metadata(&scenario.metadata.lab, &scenario.players)?;
    let scenario_json = serde_json::to_string_pretty(&scenario)
        .map_err(|err| format!("failed to format lab scenario JSON: {err}"))?
        + "\n";
    if scenario_json.len() > MAX_AUTHORING_SCENARIO_JSON_BYTES {
        return Err(format!(
            "scenario JSON must be at most {MAX_AUTHORING_SCENARIO_JSON_BYTES} bytes"
        ));
    }
    let sim_scenario = protocol_scenario_to_sim(&scenario)
        .map_err(|err| format!("invalid lab scenario payload: {err}"))?;
    build_game_from_scenario(&scenario, sim_scenario).map_err(|err| {
        format!(
            "lab scenario {:?} does not restore through Game lab APIs: {err}",
            entry.id
        )
    })?;

    Ok(LabScenarioAuthoringPreview {
        slug,
        filename: filename.clone(),
        scenario_path: format!("server/assets/lab-scenarios/{filename}"),
        manifest_path: "server/assets/lab-scenarios/manifest.json".to_string(),
        manifest_entry: entry,
        scenario_json,
        review_notes,
        summary: format!("Scenario ready for server/assets/lab-scenarios/{filename}."),
    })
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
    if manifest.scenarios.len() > MAX_SCENARIO_CATALOG_ENTRIES {
        return Err(format!(
            "lab scenario manifest has too many scenarios: {} > {MAX_SCENARIO_CATALOG_ENTRIES}",
            manifest.scenarios.len()
        ));
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
    validate_lab_scenario_lab_metadata(&scenario.metadata.lab, &scenario.players)?;
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
    let expected_filename = format!("{}.json", entry.id);
    if entry.filename != expected_filename {
        return Err(format!(
            "lab scenario {:?} filename must be {:?}",
            entry.id, expected_filename
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
    for &player_id in &scenario.metadata.lab.god_mode_players {
        game.apply_lab_op(LabOp::SetPlayerGodMode {
            player_id,
            enabled: true,
        })
        .map_err(|err| lab_error_text(&err))?;
    }
    Ok(game)
}

pub(crate) fn validate_lab_scenario_lab_metadata(
    lab: &crate::protocol::LabScenarioLabMetadata,
    players: &[crate::protocol::LabScenarioPlayer],
) -> Result<(), String> {
    validate_lab_scenario_vision(&lab.vision, players)?;
    validate_lab_scenario_god_mode_players(&lab.god_mode_players, players)
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

fn validate_lab_scenario_god_mode_players(
    player_ids: &[u32],
    players: &[crate::protocol::LabScenarioPlayer],
) -> Result<(), String> {
    let mut seen = HashSet::new();
    for player_id in player_ids {
        if !seen.insert(*player_id) {
            return Err("godModePlayers must not contain duplicates".to_string());
        }
        if !players.iter().any(|player| player.id == *player_id) {
            return Err("unknown scenario god mode player id".to_string());
        }
    }
    Ok(())
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
    use crate::protocol::Event;

    fn temp_catalog_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rts-lab-catalog-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn bundled_lab_scenario_catalog_loads_bundled_scenarios_and_restores() {
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
        let tile_size = rts_rules::balance::TILE_SIZE as f32;
        let mut oil_tiles = Vec::new();
        for entity in scenario
            .entities
            .iter()
            .filter(|entity| entity.kind == "oil")
        {
            let tile_x = (entity.x / tile_size).floor() as u32;
            let tile_y = (entity.y / tile_size).floor() as u32;
            let center_x = tile_x as f32 * tile_size + tile_size * 0.5;
            let center_y = tile_y as f32 * tile_size + tile_size * 0.5;
            assert!(
                (entity.x - center_x).abs() < 0.001 && (entity.y - center_y).abs() < 0.001,
                "lategame oil node {} should restore at tile center ({tile_x}, {tile_y})",
                entity.id
            );
            oil_tiles.push((entity.id, tile_x, tile_y));
        }
        assert_eq!(oil_tiles.len(), 18);
        for (index, &(a_id, a_x, a_y)) in oil_tiles.iter().enumerate() {
            for &(b_id, b_x, b_y) in oil_tiles.iter().skip(index + 1) {
                assert!(
                    a_x.abs_diff(b_x) > 1 || a_y.abs_diff(b_y) > 1,
                    "lategame oil nodes {a_id} and {b_id} should have one free tile between them, got tiles ({a_x}, {a_y}) and ({b_x}, {b_y})"
                );
            }
        }

        let render_preview = catalog
            .iter()
            .find(|entry| entry.id == "render-preview")
            .expect("render-preview catalog row");
        assert_eq!(render_preview.map, "Default");
        assert_eq!(render_preview.player_count, 2);
        assert_eq!(render_preview.filename, "render-preview.json");

        let loaded = load_lab_scenario_by_id("render-preview")
            .expect("bundled render-preview scenario should load");
        let game = loaded
            .build_game()
            .expect("render-preview scenario should restore through lab APIs");
        let scenario = game.export_lab_scenario();
        assert_eq!(scenario.seed, 126_097_607);
        assert_eq!(scenario.players.len(), 2);
        assert_eq!(game.lab_god_mode_players(), vec![1, 2]);

        let render_kinds: HashSet<_> = scenario
            .entities
            .iter()
            .map(|entity| entity.kind.as_str())
            .collect();
        for kind in [
            "anti_tank_gun",
            "artillery",
            "barracks",
            "city_centre",
            "command_car",
            "depot",
            "factory",
            "machine_gunner",
            "mortar_team",
            "oil",
            "panzerfaust",
            "pump_jack",
            "research_complex",
            "rifleman",
            "scout_car",
            "steel",
            "steelworks",
            "tank",
            "tank_trap",
            "training_centre",
            "worker",
        ] {
            assert!(
                render_kinds.contains(kind),
                "render-preview scenario should include {kind} render coverage"
            );
        }
        let panzerfaust_count = scenario
            .entities
            .iter()
            .filter(|entity| entity.kind == "panzerfaust")
            .count();
        assert!(
            panzerfaust_count >= 12,
            "render-preview scenario should include Panzerfaust formation coverage"
        );
    }

    #[test]
    fn render_preview_anti_tank_guns_emit_attack_events() {
        let loaded = load_lab_scenario_by_id("render-preview")
            .expect("bundled render-preview scenario should load");
        let mut game = loaded
            .build_game()
            .expect("render-preview scenario should restore through lab APIs");
        let anti_tank_gun_ids: HashSet<_> = game
            .export_lab_scenario()
            .entities
            .into_iter()
            .filter(|entity| entity.kind == "anti_tank_gun")
            .map(|entity| entity.id)
            .collect();
        assert_eq!(
            anti_tank_gun_ids.len(),
            8,
            "render-preview should keep both AT gun batteries"
        );

        let mut saw_anti_tank_attack = false;
        for _ in 0..180 {
            let events = game.tick();
            saw_anti_tank_attack |=
                events
                    .iter()
                    .flat_map(|(_, events)| events.iter())
                    .any(|event| {
                        matches!(
                            event,
                            Event::Attack { from, .. } if anti_tank_gun_ids.contains(from)
                        )
                    });
            if saw_anti_tank_attack {
                break;
            }
        }

        assert!(
            saw_anti_tank_attack,
            "render-preview AT guns should auto-acquire in-arc targets and emit tracer events"
        );
    }

    #[test]
    fn lab_scenario_authoring_validation_accepts_repo_ready_metadata() {
        let loaded =
            load_lab_scenario_by_id("lategame").expect("bundled lategame scenario should load");
        let preview = validate_lab_scenario_authoring(
            LabScenarioAuthoringMetadata {
                slug: "fresh-lab-scenario".to_string(),
                name: "Fresh Lab Scenario".to_string(),
                title: "Fresh Lab Scenario".to_string(),
                description: "A deterministic lab setup ready for catalog review.".to_string(),
                tags: vec!["two-player".to_string(), "test".to_string()],
                review_notes: Some("Check army positioning before merge.".to_string()),
            },
            loaded.scenario,
        )
        .expect("authoring metadata should validate");

        assert_eq!(preview.filename, "fresh-lab-scenario.json");
        assert_eq!(preview.manifest_entry.id, "fresh-lab-scenario");
        assert_eq!(preview.manifest_entry.map, "Default");
        assert_eq!(preview.manifest_entry.player_count, 2);
        assert!(preview
            .scenario_json
            .contains("\"name\": \"Fresh Lab Scenario\""));
        assert!(preview
            .summary
            .contains("server/assets/lab-scenarios/fresh-lab-scenario.json"));
    }

    #[test]
    fn lab_scenario_authoring_validation_rejects_duplicate_id() {
        let loaded =
            load_lab_scenario_by_id("lategame").expect("bundled lategame scenario should load");
        let err = validate_lab_scenario_authoring(
            LabScenarioAuthoringMetadata {
                slug: "lategame".to_string(),
                name: "Duplicate".to_string(),
                title: "Duplicate".to_string(),
                description: "Duplicates the bundled lategame scenario id.".to_string(),
                tags: vec!["test".to_string()],
                review_notes: None,
            },
            loaded.scenario,
        )
        .expect_err("duplicate ids should be rejected");

        assert!(
            err.contains("duplicate lab scenario id"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn lab_scenario_authoring_validation_rejects_malformed_tags() {
        let loaded =
            load_lab_scenario_by_id("lategame").expect("bundled lategame scenario should load");
        let err = validate_lab_scenario_authoring(
            LabScenarioAuthoringMetadata {
                slug: "bad-tag-scenario".to_string(),
                name: "Bad Tag Scenario".to_string(),
                title: "Bad Tag Scenario".to_string(),
                description: "Uses malformed authoring tags.".to_string(),
                tags: vec!["bad tag".to_string()],
                review_notes: None,
            },
            loaded.scenario,
        )
        .expect_err("malformed tags should be rejected");

        assert!(err.contains("invalid tag"), "unexpected error: {err}");
    }

    #[test]
    fn lab_scenario_authoring_validation_rejects_entity_cap_before_restore() {
        let loaded =
            load_lab_scenario_by_id("lategame").expect("bundled lategame scenario should load");
        let mut scenario = loaded.scenario;
        let template = scenario
            .entities
            .first()
            .expect("lategame should include entities")
            .clone();
        let mut next_id = scenario
            .entities
            .iter()
            .map(|entity| entity.id)
            .max()
            .unwrap_or(0)
            + 1;
        while scenario.entities.len() <= MAX_AUTHORING_SCENARIO_ENTITIES {
            let mut entity = template.clone();
            entity.id = next_id;
            next_id += 1;
            scenario.entities.push(entity);
        }

        let err = validate_lab_scenario_authoring(
            LabScenarioAuthoringMetadata {
                slug: "too-many-entities".to_string(),
                name: "Too Many Entities".to_string(),
                title: "Too Many Entities".to_string(),
                description: "Exercises the authoring entity cap.".to_string(),
                tags: vec!["test".to_string()],
                review_notes: None,
            },
            scenario,
        )
        .expect_err("authoring should reject scenarios over the entity cap");

        assert!(
            err.contains("scenario has too many entities"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn lab_scenario_authoring_validation_rejects_oversized_payload_before_restore() {
        let loaded =
            load_lab_scenario_by_id("lategame").expect("bundled lategame scenario should load");
        let mut scenario = loaded.scenario;
        let template = scenario
            .entities
            .first()
            .expect("lategame should include entities")
            .clone();
        let mut next_id = scenario
            .entities
            .iter()
            .map(|entity| entity.id)
            .max()
            .unwrap_or(0)
            + 1;
        while scenario.entities.len() < 850 {
            let mut entity = template.clone();
            entity.id = next_id;
            entity.kind = "oversized-authoring-kind".repeat(90);
            next_id += 1;
            scenario.entities.push(entity);
        }

        let err = validate_lab_scenario_authoring(
            LabScenarioAuthoringMetadata {
                slug: "oversized-payload".to_string(),
                name: "Oversized Payload".to_string(),
                title: "Oversized Payload".to_string(),
                description: "Exercises the authoring JSON byte cap.".to_string(),
                tags: vec!["test".to_string()],
                review_notes: None,
            },
            scenario,
        )
        .expect_err("authoring should reject oversized scenario JSON");

        assert!(
            err.contains("scenario JSON must be at most"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn lab_scenario_catalog_rejects_duplicate_ids_before_reading_json() {
        let dir = temp_catalog_dir("duplicate");
        std::fs::write(
            dir.join(LAB_SCENARIO_MANIFEST),
            r#"{
              "scenarios": [
                {"id":"dupe","title":"One","description":"First","tags":[],"map":"Default","playerCount":2,"filename":"dupe.json"},
                {"id":"dupe","title":"Two","description":"Second","tags":[],"map":"Default","playerCount":2,"filename":"dupe.json"}
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
    fn lab_scenario_catalog_rejects_filename_that_does_not_match_id() {
        let dir = temp_catalog_dir("filename-mismatch");
        std::fs::write(
            dir.join(LAB_SCENARIO_MANIFEST),
            r#"{
              "scenarios": [
                {"id":"safe-id","title":"Safe","description":"Filename mismatch","tags":[],"map":"Default","playerCount":2,"filename":"other-safe-id.json"}
              ]
            }"#,
        )
        .unwrap();

        let err =
            load_lab_scenario_catalog_from_dir(&dir).expect_err("filename mismatch should reject");
        assert!(err.contains("filename must be"), "unexpected error: {err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn lab_scenario_catalog_rejects_too_many_manifest_entries() {
        let dir = temp_catalog_dir("too-many");
        let entries = (0..=MAX_SCENARIO_CATALOG_ENTRIES)
            .map(|index| {
                format!(
                    r#"{{"id":"scenario-{index}","title":"Scenario {index}","description":"Catalog cap test","tags":[],"map":"Default","playerCount":2,"filename":"scenario-{index}.json"}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        std::fs::write(
            dir.join(LAB_SCENARIO_MANIFEST),
            format!(r#"{{"scenarios":[{entries}]}}"#),
        )
        .unwrap();

        let err =
            load_lab_scenario_catalog_from_dir(&dir).expect_err("large manifest should reject");
        assert!(
            err.contains("too many scenarios"),
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
