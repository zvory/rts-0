//! Bundled lab checkpoint setup catalog loading and validation.
//!
//! The catalog manifest is the source of truth for setup fixtures shown in the browser. Each listed
//! setup must parse as protocol JSON and restore through the public lab `Game` API before it is
//! exposed or used to start a lab room.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use rts_sim::game::lab::{LabCheckpointScenarioV1 as SimLabCheckpointScenarioV1, LabError, LabOp};
use rts_sim::game::Game;
use serde::{Deserialize, Serialize};

use crate::protocol::{
    InitialCamera, LabCheckpointScenarioV1, LabScenarioAuthoringMetadata, LabScenarioLabMetadata,
    LabScenarioPayload, LabVisionMode, MapInfo, TeamId,
};

const LAB_SCENARIO_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/lab-scenarios");
const LAB_SCENARIO_MANIFEST: &str = "manifest.json";
const LAB_CHECKPOINT_SCENARIO_KIND: &str = "labCheckpointScenario";
const LAB_CHECKPOINT_ASSET_BUILD_SHA: &str = "bundled-lab-scenario-asset-v1";
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
pub const MAX_LAB_SCENARIO_IMPORT_JSON_BYTES: usize = 1_000_000;
const MAX_AUTHORING_SCENARIO_JSON_BYTES: usize = MAX_LAB_SCENARIO_IMPORT_JSON_BYTES;

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
    pub scenario: LabScenarioPayload,
    sim_scenario: SimLabCheckpointScenarioV1,
}

impl LoadedLabScenario {
    pub fn build_game(&self) -> Result<Game, String> {
        build_game_from_checkpoint(
            &checkpoint_payload(&self.scenario),
            self.sim_scenario.clone(),
        )
    }

    pub fn build_checkpoint_game(&self) -> Result<Game, String> {
        self.build_game()
    }

    pub fn is_checkpoint_backed(&self) -> bool {
        true
    }
}

pub fn load_lab_scenario_catalog() -> Result<Vec<LabScenarioCatalogEntry>, String> {
    load_lab_scenario_catalog_from_dir(&default_lab_scenario_dir())
}

/// GET /api/lab-scenarios - bounded metadata for bundled lab checkpoint setups.
pub async fn catalog_handler() -> impl IntoResponse {
    match load_lab_scenario_catalog() {
        Ok(entries) => Json(entries).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(LabScenarioCatalogError {
                error: format!("Lab setup catalog unavailable: {err}"),
            }),
        )
            .into_response(),
    }
}

pub fn load_lab_scenario_by_id(id: &str) -> Result<LoadedLabScenario, String> {
    if !safe_scenario_id(id) {
        return Err("invalid lab setup id".to_string());
    }
    let root = default_lab_scenario_dir();
    let entries = load_lab_scenario_manifest_entries_from_dir(&root)?;
    let entry = entries
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| format!("unknown lab setup id {id:?}"))?;
    let loaded = load_lab_scenario_entry(&root, &entry)?;
    validate_loaded_lab_scenario(&entry, &loaded)?;
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
    scenario: LabScenarioPayload,
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
            "setup name must be non-empty and at most {MAX_SCENARIO_NAME_LEN} bytes"
        ));
    }
    if review_notes.len() > MAX_REVIEW_NOTES_LEN {
        return Err(format!(
            "review notes must be at most {MAX_REVIEW_NOTES_LEN} bytes"
        ));
    }

    let scenario = checkpoint_backed_authoring_scenario(scenario, &name)?;
    let facts = validate_protocol_checkpoint_scenario("authoring", &scenario)?;
    validate_authoring_entity_count(facts.entity_count)?;
    let filename = format!("{slug}.json");
    let entry = LabScenarioCatalogEntry {
        id: slug.clone(),
        title,
        description,
        tags,
        map: facts.map.clone(),
        player_count: facts.player_count,
        filename: filename.clone(),
    };
    validate_manifest_entry(&entry)?;

    let root = default_lab_scenario_dir();
    let existing_entries = load_lab_scenario_manifest_entries_from_dir(&root)?;
    if existing_entries
        .iter()
        .any(|existing| existing.id == entry.id)
    {
        return Err(format!("duplicate lab setup id {:?}", entry.id));
    }
    if existing_entries
        .iter()
        .any(|existing| existing.filename == entry.filename)
    {
        return Err(format!("duplicate lab setup filename {:?}", entry.filename));
    }

    validate_entry_matches_checkpoint_facts(&entry, &scenario, &facts)?;
    let scenario_json = serde_json::to_string_pretty(&scenario)
        .map_err(|err| format!("failed to format lab setup JSON: {err}"))?
        + "\n";
    if scenario_json.len() > MAX_AUTHORING_SCENARIO_JSON_BYTES {
        return Err(format!(
            "setup JSON must be at most {MAX_AUTHORING_SCENARIO_JSON_BYTES} bytes"
        ));
    }
    Ok(LabScenarioAuthoringPreview {
        slug,
        filename: filename.clone(),
        scenario_path: format!("server/assets/lab-scenarios/{filename}"),
        manifest_path: "server/assets/lab-scenarios/manifest.json".to_string(),
        manifest_entry: entry,
        scenario_json,
        review_notes,
        summary: format!("Checkpoint setup ready for server/assets/lab-scenarios/{filename}."),
    })
}

fn default_lab_scenario_dir() -> PathBuf {
    PathBuf::from(LAB_SCENARIO_DIR)
}

fn load_lab_scenario_catalog_from_dir(root: &Path) -> Result<Vec<LabScenarioCatalogEntry>, String> {
    let entries = load_lab_scenario_manifest_entries_from_dir(root)?;

    for entry in &entries {
        let loaded = load_lab_scenario_entry(root, entry)?;
        validate_loaded_lab_scenario(entry, &loaded)?;
    }

    Ok(entries)
}

fn load_lab_scenario_manifest_entries_from_dir(
    root: &Path,
) -> Result<Vec<LabScenarioCatalogEntry>, String> {
    let manifest_path = root.join(LAB_SCENARIO_MANIFEST);
    let manifest_json = std::fs::read_to_string(&manifest_path)
        .map_err(|err| format!("failed to read lab setup manifest: {err}"))?;
    let manifest: LabScenarioManifest = serde_json::from_str(&manifest_json)
        .map_err(|err| format!("failed to parse lab setup manifest: {err}"))?;
    if manifest.scenarios.is_empty() {
        return Err("lab setup manifest must contain at least one setup".to_string());
    }
    if manifest.scenarios.len() > MAX_SCENARIO_CATALOG_ENTRIES {
        return Err(format!(
            "lab setup manifest has too many setups: {} > {MAX_SCENARIO_CATALOG_ENTRIES}",
            manifest.scenarios.len()
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut seen_files = HashSet::new();
    for entry in &manifest.scenarios {
        validate_manifest_entry(entry)?;
        if !seen_ids.insert(entry.id.clone()) {
            return Err(format!("duplicate lab setup id {:?}", entry.id));
        }
        if !seen_files.insert(entry.filename.clone()) {
            return Err(format!("duplicate lab setup filename {:?}", entry.filename));
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
            "failed to read lab setup {:?} from {:?}: {err}",
            entry.id, entry.filename
        )
    })?;
    let scenario: LabScenarioPayload = serde_json::from_str(&json)
        .map_err(|err| format!("invalid lab setup {:?} JSON: {err}", entry.id))?;
    let sim_scenario = payload_to_sim_checkpoint(&scenario)
        .map_err(|err| format!("invalid lab setup {:?} payload: {err}", entry.id))?;
    Ok(LoadedLabScenario {
        entry: entry.clone(),
        scenario,
        sim_scenario,
    })
}

fn validate_manifest_entry(entry: &LabScenarioCatalogEntry) -> Result<(), String> {
    if !safe_scenario_id(&entry.id) {
        return Err(format!("invalid lab setup id {:?}", entry.id));
    }
    if !safe_scenario_filename(&entry.filename) {
        return Err(format!("invalid lab setup filename {:?}", entry.filename));
    }
    let expected_filename = format!("{}.json", entry.id);
    if entry.filename != expected_filename {
        return Err(format!(
            "lab setup {:?} filename must be {:?}",
            entry.id, expected_filename
        ));
    }
    if entry.title.trim().is_empty() || entry.title.len() > MAX_SCENARIO_TITLE_LEN {
        return Err(format!("invalid title for lab setup {:?}", entry.id));
    }
    if entry.description.trim().is_empty() || entry.description.len() > MAX_SCENARIO_DESCRIPTION_LEN
    {
        return Err(format!("invalid description for lab setup {:?}", entry.id));
    }
    if entry.tags.len() > MAX_SCENARIO_TAGS {
        return Err(format!("too many tags for lab setup {:?}", entry.id));
    }
    for tag in &entry.tags {
        if !safe_scenario_tag(tag) {
            return Err(format!(
                "invalid tag {:?} for lab setup {:?}",
                tag, entry.id
            ));
        }
    }
    if entry.map.trim().is_empty() {
        return Err(format!("missing map for lab setup {:?}", entry.id));
    }
    if entry.player_count == 0 {
        return Err(format!(
            "playerCount must be nonzero for lab setup {:?}",
            entry.id
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct LabScenarioFacts {
    map: String,
    player_count: usize,
    entity_count: usize,
}

fn checkpoint_payload(scenario: &LabScenarioPayload) -> LabCheckpointScenarioV1 {
    match scenario {
        LabScenarioPayload::Checkpoint(scenario) => scenario.clone(),
    }
}

fn checkpoint_payload_ref(scenario: &LabScenarioPayload) -> &LabCheckpointScenarioV1 {
    match scenario {
        LabScenarioPayload::Checkpoint(scenario) => scenario,
    }
}

fn payload_to_sim_checkpoint(
    scenario: &LabScenarioPayload,
) -> Result<SimLabCheckpointScenarioV1, String> {
    protocol_checkpoint_to_sim(checkpoint_payload_ref(scenario))
}

fn protocol_checkpoint_to_sim(
    scenario: &LabCheckpointScenarioV1,
) -> Result<SimLabCheckpointScenarioV1, String> {
    let mut value =
        serde_json::to_value(scenario).map_err(|err| format!("serialize failed: {err}"))?;
    value
        .get_mut("metadata")
        .and_then(|metadata| metadata.as_object_mut())
        .ok_or_else(|| "checkpoint setup metadata must be an object".to_string())?
        .remove("lab");
    serde_json::from_value(value).map_err(|err| err.to_string())
}

fn sim_checkpoint_to_protocol(
    scenario: SimLabCheckpointScenarioV1,
    lab: LabScenarioLabMetadata,
) -> Result<LabCheckpointScenarioV1, String> {
    let mut value =
        serde_json::to_value(scenario).map_err(|err| format!("serialize failed: {err}"))?;
    value
        .get_mut("metadata")
        .and_then(|metadata| metadata.as_object_mut())
        .ok_or_else(|| "checkpoint setup metadata must be an object".to_string())?
        .insert(
            "lab".to_string(),
            serde_json::to_value(lab).map_err(|err| format!("serialize failed: {err}"))?,
        );
    serde_json::from_value(value).map_err(|err| err.to_string())
}

fn checkpoint_backed_authoring_scenario(
    scenario: LabScenarioPayload,
    name: &str,
) -> Result<LabCheckpointScenarioV1, String> {
    match scenario {
        LabScenarioPayload::Checkpoint(mut scenario) => {
            scenario.name = name.to_string();
            Ok(scenario)
        }
    }
}

fn validate_authoring_entity_count(entity_count: usize) -> Result<(), String> {
    if entity_count > MAX_AUTHORING_SCENARIO_ENTITIES {
        return Err(format!(
            "setup has too many entities: {entity_count} > {MAX_AUTHORING_SCENARIO_ENTITIES}"
        ));
    }
    Ok(())
}

fn validate_entry_matches_loaded_scenario(
    entry: &LabScenarioCatalogEntry,
    loaded: &LoadedLabScenario,
) -> Result<(), String> {
    let scenario = checkpoint_payload_ref(&loaded.scenario);
    let facts = validate_protocol_checkpoint_scenario(&entry.id, scenario)?;
    validate_entry_matches_checkpoint_facts(entry, scenario, &facts)
}

fn validate_loaded_lab_scenario(
    entry: &LabScenarioCatalogEntry,
    loaded: &LoadedLabScenario,
) -> Result<(), String> {
    validate_entry_matches_loaded_scenario(entry, loaded)
}

fn validate_entry_matches_checkpoint_facts(
    entry: &LabScenarioCatalogEntry,
    scenario: &LabCheckpointScenarioV1,
    facts: &LabScenarioFacts,
) -> Result<(), String> {
    validate_checkpoint_scenario_identity(&entry.id, scenario)?;
    if facts.map != entry.map {
        return Err(format!(
            "lab setup {:?} manifest map {:?} does not match JSON map {:?}",
            entry.id, entry.map, facts.map
        ));
    }
    if facts.player_count != entry.player_count {
        return Err(format!(
            "lab setup {:?} manifest playerCount {} does not match JSON player count {}",
            entry.id, entry.player_count, facts.player_count
        ));
    }
    Ok(())
}

fn validate_checkpoint_scenario_identity(
    label: &str,
    scenario: &LabCheckpointScenarioV1,
) -> Result<(), String> {
    if scenario.kind != LAB_CHECKPOINT_SCENARIO_KIND {
        return Err(format!(
            "lab setup {label:?} kind must be {LAB_CHECKPOINT_SCENARIO_KIND:?}"
        ));
    }
    Ok(())
}

fn build_game_from_checkpoint(
    scenario: &LabCheckpointScenarioV1,
    sim_scenario: SimLabCheckpointScenarioV1,
) -> Result<Game, String> {
    let game =
        Game::restore_lab_checkpoint_scenario(sim_scenario).map_err(|err| lab_error_text(&err))?;
    validate_checkpoint_lab_metadata_matches_game(&scenario.metadata.lab, &game)?;
    Ok(game)
}

fn validate_protocol_checkpoint_scenario(
    label: &str,
    scenario: &LabCheckpointScenarioV1,
) -> Result<LabScenarioFacts, String> {
    validate_checkpoint_scenario_identity(label, scenario)?;
    let sim_scenario = protocol_checkpoint_to_sim(scenario)
        .map_err(|err| format!("invalid checkpoint setup payload: {err}"))?;
    let game = build_game_from_checkpoint(scenario, sim_scenario)
        .map_err(|err| format!("checkpoint setup restore failed: {err}"))?;
    let start = game.start_payload();
    Ok(LabScenarioFacts {
        map: scenario.map.name.clone(),
        player_count: start.players.len(),
        entity_count: game.perf_entity_counts().entities,
    })
}

fn validate_checkpoint_lab_metadata_matches_game(
    lab: &LabScenarioLabMetadata,
    game: &Game,
) -> Result<(), String> {
    let start = game.start_payload();
    let player_facts: Vec<_> = start
        .players
        .iter()
        .map(|player| (player.id, player.team_id))
        .collect();
    validate_lab_metadata_for_player_facts(lab, &player_facts, &start.map)?;
    let expected: HashSet<_> = game.lab_god_mode_players().into_iter().collect();
    let actual: HashSet<_> = lab.god_mode_players.iter().copied().collect();
    if actual != expected {
        return Err(
            "checkpoint setup lab godModePlayers must match the embedded payload".to_string(),
        );
    }
    Ok(())
}

fn validate_lab_metadata_for_player_facts(
    lab: &LabScenarioLabMetadata,
    players: &[(u32, TeamId)],
    map: &MapInfo,
) -> Result<(), String> {
    validate_lab_scenario_vision(&lab.vision, players)?;
    validate_lab_scenario_god_mode_players(&lab.god_mode_players, players)?;
    validate_lab_scenario_initial_camera(lab.initial_camera.as_ref(), map)
}

fn validate_lab_scenario_initial_camera(
    initial_camera: Option<&InitialCamera>,
    map: &MapInfo,
) -> Result<(), String> {
    let Some(initial_camera) = initial_camera else {
        return Ok(());
    };
    let world_w = map
        .width
        .checked_mul(map.tile_size)
        .ok_or_else(|| "initialCamera map width overflows".to_string())?;
    let world_h = map
        .height
        .checked_mul(map.tile_size)
        .ok_or_else(|| "initialCamera map height overflows".to_string())?;
    if world_w == 0 || world_h == 0 {
        return Err("initialCamera requires a non-empty map".to_string());
    }
    if initial_camera.center_x >= world_w || initial_camera.center_y >= world_h {
        return Err("initialCamera center must be inside the map world bounds".to_string());
    }
    Ok(())
}

fn validate_lab_scenario_vision(
    vision: &LabVisionMode,
    players: &[(u32, TeamId)],
) -> Result<(), String> {
    match vision {
        LabVisionMode::All => Ok(()),
        LabVisionMode::Team { team_id } => {
            if players
                .iter()
                .any(|(_, player_team_id)| player_team_id == team_id)
            {
                Ok(())
            } else {
                Err("unknown setup lab team id".to_string())
            }
        }
    }
}

fn validate_lab_scenario_god_mode_players(
    player_ids: &[u32],
    players: &[(u32, TeamId)],
) -> Result<(), String> {
    let mut seen = HashSet::new();
    for player_id in player_ids {
        if !seen.insert(*player_id) {
            return Err("godModePlayers must not contain duplicates".to_string());
        }
        if !players.iter().any(|(id, _)| id == player_id) {
            return Err("unknown setup god mode player id".to_string());
        }
    }
    Ok(())
}

pub(crate) fn lab_scenario_payload_lab_metadata(
    scenario: &LabScenarioPayload,
) -> &LabScenarioLabMetadata {
    match scenario {
        LabScenarioPayload::Checkpoint(scenario) => &scenario.metadata.lab,
    }
}

pub(crate) fn lab_scenario_payload_to_lab_op(
    scenario: LabScenarioPayload,
) -> Result<LabOp, String> {
    validate_lab_scenario_payload_size(&scenario)?;
    match scenario {
        LabScenarioPayload::Checkpoint(scenario) => {
            validate_protocol_checkpoint_scenario("import", &scenario)?;
            let sim_scenario = protocol_checkpoint_to_sim(&scenario)
                .map_err(|err| format!("invalid checkpoint setup payload: {err}"))?;
            Ok(LabOp::RestoreCheckpointScenario(Box::new(sim_scenario)))
        }
    }
}

fn validate_lab_scenario_payload_size(scenario: &LabScenarioPayload) -> Result<(), String> {
    let bytes = serde_json::to_vec(scenario)
        .map_err(|err| format!("failed to measure lab setup JSON: {err}"))?
        .len();
    if bytes <= MAX_LAB_SCENARIO_IMPORT_JSON_BYTES {
        return Ok(());
    }
    Err(format!(
        "setup JSON must be at most {MAX_LAB_SCENARIO_IMPORT_JSON_BYTES} bytes"
    ))
}

pub(crate) fn export_lab_checkpoint_scenario_for_protocol(
    game: &Game,
    name: String,
    lab: LabScenarioLabMetadata,
    server_build_sha: &str,
) -> Result<LabCheckpointScenarioV1, String> {
    let scenario = game
        .export_lab_checkpoint_scenario(name, server_build_sha)
        .map_err(|err| format!("checkpoint setup export failed: {}", lab_error_text(&err)))?;
    sim_checkpoint_to_protocol(scenario, lab)
}

pub fn convert_lab_scenario_catalog_assets_to_checkpoints(
    root: &Path,
    _server_build_sha: &str,
) -> Result<usize, String> {
    let entries = load_lab_scenario_manifest_entries_from_dir(root)?;
    for entry in &entries {
        let path = root.join(&entry.filename);
        let json = std::fs::read_to_string(&path).map_err(|err| {
            format!(
                "failed to read lab setup {:?} from {:?}: {err}",
                entry.id, entry.filename
            )
        })?;
        let scenario: LabScenarioPayload = serde_json::from_str(&json)
            .map_err(|err| format!("invalid lab setup {:?} JSON: {err}", entry.id))?;
        let scenario = checkpoint_payload_ref(&scenario);
        let facts = validate_protocol_checkpoint_scenario(&entry.id, scenario)?;
        validate_entry_matches_checkpoint_facts(entry, scenario, &facts)?;
    }
    Ok(0)
}

pub fn bundled_lab_scenario_asset_build_sha() -> &'static str {
    LAB_CHECKPOINT_ASSET_BUILD_SHA
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

    fn assert_entity_position(snapshot: &crate::protocol::Snapshot, id: u32, x: f32, y: f32) {
        let entity = snapshot
            .entities
            .iter()
            .find(|entity| entity.id == id)
            .unwrap_or_else(|| panic!("missing scenario entity {id}"));
        assert_eq!((entity.x, entity.y), (x, y));
    }

    fn assert_scenario_buildings_on_grass(
        scenario: &LabScenarioPayload,
        snapshot: &crate::protocol::Snapshot,
        is_corner_entity: impl Fn(u32) -> bool,
    ) {
        let map = &checkpoint_payload_ref(scenario).map.data;
        let tile_size = rts_rules::balance::TILE_SIZE as f32;
        for entity in snapshot
            .entities
            .iter()
            .filter(|entity| is_corner_entity(entity.id))
        {
            let (foot_w, foot_h) = match entity.kind.as_str() {
                "city_centre" | "factory" | "research_complex" | "steelworks" => (3, 3),
                "barracks" | "training_centre" => (3, 2),
                "depot" => (2, 2),
                _ => continue,
            };
            let start_x = (entity.x / tile_size - foot_w as f32 / 2.0).round() as u32;
            let start_y = (entity.y / tile_size - foot_h as f32 / 2.0).round() as u32;
            for tile_y in start_y..start_y + foot_h {
                for tile_x in start_x..start_x + foot_w {
                    let terrain = map.terrain[(tile_y * map.size + tile_x) as usize];
                    assert_eq!(
                        terrain,
                        rts_rules::terrain::MAP_TERRAIN_GRASS,
                        "scenario entity {} occupies non-grass tile ({tile_x}, {tile_y})",
                        entity.id
                    );
                }
            }
        }
    }

    #[test]
    fn bundled_lab_scenario_catalog_loads_bundled_scenarios_and_restores() {
        let catalog = load_lab_scenario_catalog().expect("bundled lab catalog should load");
        let lategame = catalog
            .iter()
            .find(|entry| entry.id == "lategame")
            .expect("lategame catalog row");
        assert_eq!(lategame.map, "1v1");
        assert_eq!(lategame.player_count, 2);
        assert_eq!(lategame.filename, "lategame.json");

        let loaded =
            load_lab_scenario_by_id("lategame").expect("bundled lategame scenario should load");
        assert!(loaded.is_checkpoint_backed());
        let mut game = loaded
            .build_game()
            .expect("lategame scenario should restore through lab APIs");
        let checkpoint_game = loaded
            .build_checkpoint_game()
            .expect("lategame scenario should restore through checkpoint adapter");
        assert_eq!(game.start_payload(), checkpoint_game.start_payload());
        assert_eq!(
            game.snapshot_full_for(1),
            checkpoint_game.snapshot_full_for(1)
        );
        let snapshot = game.snapshot_full_for(1);
        assert_eq!(game.seed(), 3_566_641_871);
        assert_eq!(game.start_payload().players.len(), 2);
        assert_eq!(game.perf_entity_counts().entities, 227);
        assert_entity_position(&snapshot, 1, 2960.0, 1040.0);
        assert_entity_position(&snapshot, 21, 1072.0, 2992.0);
        assert_scenario_buildings_on_grass(&loaded.scenario, &snapshot, |id| {
            id <= 5 || (21..=25).contains(&id) || (101..=172).contains(&id)
        });
        let tile_size = rts_rules::balance::TILE_SIZE as f32;
        let mut oil_tiles = Vec::new();
        for entity in snapshot
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
        game.tick();

        let render_preview = catalog
            .iter()
            .find(|entry| entry.id == "render-preview")
            .expect("render-preview catalog row");
        assert_eq!(render_preview.map, "1v1");
        assert_eq!(render_preview.player_count, 2);
        assert_eq!(render_preview.filename, "render-preview.json");

        let loaded = load_lab_scenario_by_id("render-preview")
            .expect("bundled render-preview scenario should load");
        assert!(loaded.is_checkpoint_backed());
        lab_scenario_payload_to_lab_op(loaded.scenario.clone())
            .expect("checkpoint scenario should fit import cap");
        let game = loaded
            .build_game()
            .expect("render-preview scenario should restore through lab APIs");
        let snapshot = game.snapshot_full_for(1);
        assert_eq!(game.seed(), 126_097_607);
        assert_eq!(game.start_payload().players.len(), 2);
        assert_eq!(game.lab_god_mode_players(), vec![1, 2]);
        assert_entity_position(&snapshot, 1, 1072.0, 2992.0);
        assert_entity_position(&snapshot, 21, 2960.0, 1040.0);
        assert_scenario_buildings_on_grass(&loaded.scenario, &snapshot, |id| {
            id <= 5 || (21..=25).contains(&id) || (175..=202).contains(&id)
        });

        let render_kinds: HashSet<_> = snapshot
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
        let panzerfaust_count = snapshot
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
            .snapshot_full_for(1)
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
        assert_eq!(preview.manifest_entry.map, "1v1");
        assert_eq!(preview.manifest_entry.player_count, 2);
        assert!(preview
            .scenario_json
            .contains("\"name\": \"Fresh Lab Scenario\""));
        assert!(preview
            .scenario_json
            .contains("\"kind\": \"labCheckpointScenario\""));
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
            err.contains("duplicate lab setup id"),
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
    fn lab_scenario_authoring_validation_rejects_oversized_checkpoint_json() {
        let loaded =
            load_lab_scenario_by_id("lategame").expect("bundled lategame scenario should load");
        let LabScenarioPayload::Checkpoint(mut scenario) = loaded.scenario;
        scenario.checkpoint_payload.push('\n');
        scenario
            .checkpoint_payload
            .push_str(&" ".repeat(MAX_AUTHORING_SCENARIO_JSON_BYTES));
        let import_err =
            lab_scenario_payload_to_lab_op(LabScenarioPayload::Checkpoint(scenario.clone()))
                .expect_err("oversized import payload should be rejected before restore");
        assert!(import_err.contains("setup JSON must be at most"));

        let err = validate_lab_scenario_authoring(
            LabScenarioAuthoringMetadata {
                slug: "oversized-payload".to_string(),
                name: "Oversized Payload".to_string(),
                title: "Oversized Payload".to_string(),
                description: "Exercises the authoring JSON byte cap.".to_string(),
                tags: vec!["test".to_string()],
                review_notes: None,
            },
            LabScenarioPayload::Checkpoint(scenario),
        )
        .expect_err("authoring should reject oversized setup JSON");

        assert!(
            err.contains("setup JSON must be at most"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn lab_scenario_import_rejects_initial_camera_outside_map() {
        let loaded =
            load_lab_scenario_by_id("lategame").expect("bundled lategame scenario should load");
        let LabScenarioPayload::Checkpoint(mut scenario) = loaded.scenario;
        scenario.metadata.lab.initial_camera = Some(InitialCamera {
            center_x: u32::MAX,
            center_y: 0,
        });

        let err = lab_scenario_payload_to_lab_op(LabScenarioPayload::Checkpoint(scenario))
            .expect_err("out-of-bounds initialCamera should reject");

        assert!(
            err.contains("initialCamera center must be inside the map world bounds"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn lab_scenario_catalog_rejects_legacy_v1_files() {
        let dir = temp_catalog_dir("legacy-v1");
        std::fs::write(
            dir.join(LAB_SCENARIO_MANIFEST),
            r#"{
              "scenarios": [
                {"id":"legacy-v1","title":"Legacy V1","description":"Legacy compatibility fixture","tags":["test"],"map":"Default","playerCount":2,"filename":"legacy-v1.json"}
              ]
            }"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("legacy-v1.json"),
            r#"{
              "schemaVersion": 1,
              "kind": "labScenario",
              "name": "Legacy V1",
              "seed": 1,
              "map": {"name": "Default", "schemaVersion": 2, "contentHash": "legacy"},
              "players": [],
              "entities": [],
              "metadata": {"exportedTick": 0, "lab": {"vision": {"mode": "all"}}}
            }"#,
        )
        .unwrap();

        let err =
            load_lab_scenario_catalog_from_dir(&dir).expect_err("legacy V1 fixture should reject");
        assert!(
            err.contains("invalid lab setup") || err.contains("labCheckpointScenario"),
            "unexpected error: {err}"
        );
        let _ = std::fs::remove_dir_all(dir);
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
            err.contains("duplicate lab setup id"),
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
        assert!(err.contains("too many setups"), "unexpected error: {err}");
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
            err.contains("invalid lab setup id"),
            "unexpected error: {err}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
