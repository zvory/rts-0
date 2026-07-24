//! Deterministic, client-only snapshot stream generation for the Hellhole workload.

use std::path::Path;

use serde_json::json;

use crate::lab_scenarios::load_lab_scenario_by_id;
use crate::lobby::lab_scenario_driver::{
    lab_scenario_driver_for, LabScenarioAction, LabScenarioDriver,
};
use crate::protocol::{serialize_messagepack_compact_snapshot, Event};
use crate::tools::hellhole_spec::{CENTER, SCENARIO_ID};

pub const STREAM_ID: &str = SCENARIO_ID;
pub const DEFAULT_FRAME_COUNT: u32 = 900;
pub const TICK_RATE_HZ: u32 = 30;
pub const MAGIC: &[u8; 8] = b"RTSSTRM1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotStreamSummary {
    pub frame_count: u32,
    pub first_tick: u32,
    pub last_tick: u32,
    pub initial_entity_count: usize,
    pub byte_len: usize,
    pub death_events: usize,
    pub respawned_units: usize,
    pub minimum_entity_count: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HellholeActionCounts {
    pub(crate) shuttle_commands: usize,
    pub(crate) selected_units: usize,
    pub(crate) respawn_batches: usize,
    pub(crate) respawned_units: usize,
}

impl HellholeActionCounts {
    pub(crate) fn add(&mut self, other: Self) {
        self.shuttle_commands += other.shuttle_commands;
        self.selected_units += other.selected_units;
        self.respawn_batches += other.respawn_batches;
        self.respawned_units += other.respawned_units;
    }
}

pub fn write_hellhole_snapshot_stream(
    output: &Path,
    frame_count: u32,
) -> Result<SnapshotStreamSummary, String> {
    let (bytes, mut summary) = generate_hellhole_snapshot_stream(frame_count)?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    std::fs::write(output, &bytes)
        .map_err(|err| format!("failed to write {}: {err}", output.display()))?;
    summary.byte_len = bytes.len();
    Ok(summary)
}

pub fn generate_hellhole_snapshot_stream(
    frame_count: u32,
) -> Result<(Vec<u8>, SnapshotStreamSummary), String> {
    if frame_count == 0 || frame_count > 10_000 {
        return Err("frame count must be between 1 and 10,000".to_string());
    }

    let (mut game, mut driver) = build_hellhole_game()?;

    let initial_entity_count = game.snapshot_for(1).entities.len();
    let mut start = serde_json::to_value(game.start_payload())
        .map_err(|err| format!("failed to serialize start payload: {err}"))?;
    let start_object = start
        .as_object_mut()
        .ok_or_else(|| "start payload was not a JSON object".to_string())?;
    start_object.insert("playerId".to_string(), json!(1));
    start_object.insert("spectator".to_string(), json!(false));
    start_object.insert(
        "snapshotStream".to_string(),
        json!({
            "id": STREAM_ID,
            "title": "Supply 300 Hellhole — Player 1 offline snapshot stream",
            "sourceScenario": STREAM_ID,
            "serverSimulation": false,
            "initialCamera": {
                "centerX": CENTER.0,
                "centerY": CENTER.1
            }
        }),
    );

    let mut frames = Vec::with_capacity(frame_count as usize);
    let mut first_tick = 0;
    let mut last_tick = 0;
    let mut death_events = 0;
    let mut action_counts = HellholeActionCounts::default();
    let mut minimum_entity_count = initial_entity_count;
    for index in 0..frame_count {
        action_counts.add(apply_hellhole_scenario_actions(&mut game, &mut driver)?);
        let event_sets = game.tick();
        let mut snapshot = game.snapshot_for(1);
        snapshot.events = event_sets
            .into_iter()
            .find_map(|(player_id, events)| (player_id == 1).then_some(events))
            .unwrap_or_default();
        death_events += snapshot
            .events
            .iter()
            .filter(|event| matches!(event, Event::Death { .. }))
            .count();
        minimum_entity_count = minimum_entity_count.min(snapshot.entities.len());
        snapshot.net_status = Default::default();
        if index == 0 {
            first_tick = snapshot.tick;
        }
        last_tick = snapshot.tick;
        let frame = serialize_messagepack_compact_snapshot(&snapshot)
            .map_err(|err| format!("failed to encode snapshot tick {}: {err}", snapshot.tick))?;
        frames.push(frame);
    }

    let header = json!({
        "schemaVersion": 1,
        "id": STREAM_ID,
        "tickRateHz": TICK_RATE_HZ,
        // Keep the benchmark stream finite. Restarting it would emit another start payload,
        // rebuild Match (including its renderer and profiler), and make a run that crosses the
        // boundary report only the post-restart tail instead of one coherent workload.
        "loop": false,
        "frameCount": frame_count,
        "firstTick": first_tick,
        "lastTick": last_tick,
        "initialEntityCount": initial_entity_count,
        "start": start
    });
    let header_bytes = serde_json::to_vec(&header)
        .map_err(|err| format!("failed to serialize snapshot stream header: {err}"))?;
    let header_len = u32::try_from(header_bytes.len())
        .map_err(|_| "snapshot stream header is too large".to_string())?;

    let frame_bytes: usize = frames.iter().map(|frame| frame.len() + 4).sum();
    let mut bytes = Vec::with_capacity(MAGIC.len() + 4 + header_bytes.len() + frame_bytes);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&header_len.to_le_bytes());
    bytes.extend_from_slice(&header_bytes);
    for frame in frames {
        let len =
            u32::try_from(frame.len()).map_err(|_| "snapshot frame is too large".to_string())?;
        bytes.extend_from_slice(&len.to_le_bytes());
        bytes.extend_from_slice(&frame);
    }

    let summary = SnapshotStreamSummary {
        frame_count,
        first_tick,
        last_tick,
        initial_entity_count,
        byte_len: bytes.len(),
        death_events,
        respawned_units: action_counts.respawned_units,
        minimum_entity_count,
    };
    Ok((bytes, summary))
}

pub(crate) fn build_hellhole_game() -> Result<(rts_sim::game::Game, LabScenarioDriver), String> {
    let scenario = load_lab_scenario_by_id(STREAM_ID)?;
    let game = scenario.build_game()?;
    let driver = lab_scenario_driver_for(STREAM_ID)
        .ok_or_else(|| format!("missing Lab scenario driver for {STREAM_ID}"))?;
    Ok((game, driver))
}

pub(crate) fn apply_hellhole_scenario_actions(
    game: &mut rts_sim::game::Game,
    driver: &mut LabScenarioDriver,
) -> Result<HellholeActionCounts, String> {
    let mut counts = HellholeActionCounts::default();
    for action in driver.actions_for_tick(game) {
        let result = match action {
            LabScenarioAction::Command(command) => {
                counts.shuttle_commands += 1;
                counts.selected_units += match &command.command {
                    crate::protocol::Command::Move { units, .. } => units.len(),
                    _ => 0,
                };
                game.issue_lab_command_as(command.player_id, command.command, command.options)
            }
            LabScenarioAction::LabOperation { op, .. } => {
                if let rts_sim::game::lab::LabOp::SpawnEntities(spawns) = &op {
                    counts.respawn_batches += 1;
                    counts.respawned_units += spawns.len();
                }
                game.apply_lab_op(op).map(|_| ())
            }
        };
        result.map_err(|err| format!("failed to apply Hellhole scenario action: {err:?}"))?;
    }
    Ok(counts)
}

pub(crate) fn union_events<'a>(event_sets: impl Iterator<Item = &'a Vec<Event>>) -> Vec<Event> {
    let mut events = Vec::new();
    for set in event_sets {
        for event in set {
            if !events.contains(event) {
                events.push(event.clone());
            }
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_stream_has_bounded_header_and_exact_frame_table() {
        let (bytes, summary) = generate_hellhole_snapshot_stream(3).unwrap();
        assert_eq!(summary.frame_count, 3);
        assert_eq!((summary.first_tick, summary.last_tick), (1, 3));
        assert_eq!(summary.initial_entity_count, 316);
        assert_eq!(&bytes[..MAGIC.len()], MAGIC);

        let header_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let header: serde_json::Value =
            serde_json::from_slice(&bytes[12..12 + header_len]).unwrap();
        assert_eq!(header["frameCount"], 3);
        assert_eq!(header["loop"], false);
        assert_eq!(header["start"]["playerId"], 1);
        assert_eq!(header["start"]["spectator"], false);
        assert_eq!(header["start"]["players"].as_array().unwrap().len(), 4);
        for (index, team_id) in [1, 2, 1, 2].into_iter().enumerate() {
            assert_eq!(header["start"]["players"][index]["teamId"], team_id);
        }
        assert_eq!(header["start"]["map"]["width"], 126);
        assert_eq!(header["start"]["map"]["height"], 126);
        assert_eq!(
            header["start"]["map"]["terrain"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|tile| tile.as_u64() == Some(crate::protocol::terrain::ROCK as u64))
                .count(),
            470
        );
        assert_eq!(
            header["initialEntityCount"].as_u64(),
            Some(summary.initial_entity_count as u64)
        );
        assert_eq!(
            header["start"]["snapshotStream"]["sourceScenario"],
            STREAM_ID
        );
        assert_eq!(header["start"]["snapshotStream"]["serverSimulation"], false);

        let mut offset = 12 + header_len;
        for _ in 0..3 {
            let len = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            offset += 4 + len;
        }
        assert_eq!(offset, bytes.len());
    }

    #[test]
    fn default_artifact_covers_thirty_seconds() {
        assert_eq!(DEFAULT_FRAME_COUNT / TICK_RATE_HZ, 30);
    }

    #[test]
    fn generated_churn_is_deterministic_and_contains_death_respawn_frames() {
        let (first_bytes, first) = generate_hellhole_snapshot_stream(120).unwrap();
        let (second_bytes, second) = generate_hellhole_snapshot_stream(120).unwrap();
        assert_eq!(first_bytes, second_bytes);
        assert_eq!(first, second);
        assert!(first.death_events > 0);
        assert!(first.death_events >= first.respawned_units);
        assert!(first.death_events - first.respawned_units <= 10);
        assert!(first.minimum_entity_count < first.initial_entity_count);
    }
}
