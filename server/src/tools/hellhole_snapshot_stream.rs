//! Deterministic, client-only snapshot stream generation for the Hellhole workload.

use std::path::Path;

use rts_sim::game::lab::LabCommandOptions;
use serde_json::json;

use crate::lab_scenarios::load_lab_scenario_by_id;
use crate::protocol::{serialize_messagepack_compact_snapshot, Command, Event};

pub const STREAM_ID: &str = "supply-300-hellhole";
pub const DEFAULT_FRAME_COUNT: u32 = 900;
pub const TICK_RATE_HZ: u32 = 30;
pub const MAGIC: &[u8; 8] = b"RTSSTRM1";

const TILE: f32 = 32.0;
const CENTER_TILE: f32 = 63.0;
const SHUTTLE_OFFSET_TILES: f32 = 18.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotStreamSummary {
    pub frame_count: u32,
    pub first_tick: u32,
    pub last_tick: u32,
    pub initial_entity_count: usize,
    pub byte_len: usize,
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

    let scenario = load_lab_scenario_by_id(STREAM_ID)?;
    let mut game = scenario.build_game()?;
    enqueue_initial_shuttles(&mut game)?;

    let initial_entity_count = game.snapshot_full_for(1).entities.len();
    let mut start = serde_json::to_value(game.start_payload())
        .map_err(|err| format!("failed to serialize start payload: {err}"))?;
    let start_object = start
        .as_object_mut()
        .ok_or_else(|| "start payload was not a JSON object".to_string())?;
    start_object.insert("playerId".to_string(), json!(1));
    start_object.insert("spectator".to_string(), json!(true));
    start_object.insert(
        "snapshotStream".to_string(),
        json!({
            "id": STREAM_ID,
            "title": "Supply 300 Hellhole — offline snapshot stream",
            "sourceScenario": STREAM_ID,
            "serverSimulation": false,
            "initialCamera": {
                "centerX": CENTER_TILE * TILE,
                "centerY": CENTER_TILE * TILE
            }
        }),
    );

    let mut frames = Vec::with_capacity(frame_count as usize);
    let mut first_tick = 0;
    let mut last_tick = 0;
    for index in 0..frame_count {
        let event_sets = game.tick();
        let mut snapshot = game.snapshot_full_for(1);
        snapshot.events = union_events(event_sets.iter().map(|(_, events)| events));
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
    };
    Ok((bytes, summary))
}

fn enqueue_initial_shuttles(game: &mut rts_sim::game::Game) -> Result<(), String> {
    for (player_id, x_dir, y_dir) in [(3, -1.0, 1.0), (4, 1.0, 1.0)] {
        let units = game.lab_owned_unit_ids(player_id).map_err(|err| {
            format!("failed to collect player {player_id} shuttle units: {err:?}")
        })?;
        game.issue_lab_command_as(
            player_id,
            Command::Move {
                units,
                x: (CENTER_TILE + x_dir * SHUTTLE_OFFSET_TILES) * TILE,
                y: (CENTER_TILE + y_dir * SHUTTLE_OFFSET_TILES) * TILE,
                queued: false,
            },
            LabCommandOptions {
                ignore_command_limits: true,
            },
        )
        .map_err(|err| format!("failed to enqueue player {player_id} shuttle: {err:?}"))?;
    }
    Ok(())
}

fn union_events<'a>(event_sets: impl Iterator<Item = &'a Vec<Event>>) -> Vec<Event> {
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
        assert_eq!(summary.initial_entity_count, 380);
        assert_eq!(&bytes[..MAGIC.len()], MAGIC);

        let header_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let header: serde_json::Value =
            serde_json::from_slice(&bytes[12..12 + header_len]).unwrap();
        assert_eq!(header["frameCount"], 3);
        assert_eq!(header["loop"], false);
        assert_eq!(header["start"]["spectator"], true);
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
}
