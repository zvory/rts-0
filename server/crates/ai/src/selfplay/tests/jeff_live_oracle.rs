use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::ai_core::profile_manifest::profile_identity_by_id;
use crate::ai_core::profiles::JEFFS_AI_ID;
use crate::{AiController, AiThinkContext};
use rts_sim::game::command::SimCommand;
use rts_sim::game::map::Map;
use rts_sim::game::replay::CommandLogEntry;
use rts_sim::game::{Game, PlayerInit};

const SCHEMA_VERSION: u32 = 1;
const FIXTURE_ID: &str = "jeff-vs-jeff-chokes-v1";
const FIXTURE_SEED: u32 = 0x4a45_4646;
const NORMAL_HORIZON: u32 = 3_600;
const FULL_HORIZON: u32 = 9_000;
const MAP_NAME: &str = "Chokes";
const FIXTURE_PATH: &str = "fixtures/jeff_live_oracle_v1.jsonl";
const CANDIDATE_ENV: &str = "RTS_GENERATE_JEFF_ORACLE_CANDIDATE";
const FINGERPRINT_FLOAT_STEPS_PER_UNIT: f64 = 1_024.0;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "record", rename_all = "snake_case")]
enum TranscriptRecord {
    Manifest(Manifest),
    Tick(TickRecord),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct Manifest {
    schema_version: u32,
    fixture_id: String,
    canonical_encoding: String,
    hash_algorithm: String,
    seed: u32,
    full_horizon: u32,
    normal_horizon: u32,
    map_name: String,
    map_schema_version: u32,
    map_content_hash: String,
    start_payload_fingerprint: String,
    players: Vec<PlayerSpec>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct PlayerSpec {
    player_id: u32,
    team_id: u32,
    faction_id: String,
    profile_id: String,
    profile_fingerprint: String,
    start_tile_x: u32,
    start_tile_y: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct TickRecord {
    tick: u32,
    alive_player_ids: Vec<u32>,
    controllers: Vec<ControllerRecord>,
    post_tick: PostTickRecord,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct ControllerRecord {
    player_id: u32,
    profile_id: String,
    invocation: Invocation,
    snapshot_fingerprint: Option<String>,
    retreat_commands: Vec<SimCommand>,
    emitted_commands: Vec<SimCommand>,
    decision_trace: Option<TraceRecord>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Invocation {
    Invoked,
    SkippedDead,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct TraceRecord {
    tick: u32,
    lines: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct PostTickRecord {
    command_log_delta: Vec<CommandLogEntry>,
    recipient_event_fingerprints: Vec<RecipientFingerprint>,
    objective_alive_player_ids: Vec<u32>,
    snapshot_fingerprints: Vec<PlayerFingerprint>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct RecipientFingerprint {
    player_id: u32,
    fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct PlayerFingerprint {
    player_id: u32,
    fingerprint: String,
}

#[test]
fn jeff_live_oracle_matches_production_controller() {
    let full = crate::full_ai_tests_enabled();
    if env_flag(CANDIDATE_ENV) {
        assert!(
            full,
            "candidate generation requires RTS_FULL_AI_TESTS=1 so the full fixture is written"
        );
        let transcript = run_transcript(FULL_HORIZON);
        let bytes = encode_jsonl(&transcript);
        let path = candidate_path();
        std::fs::create_dir_all(path.parent().expect("candidate parent"))
            .expect("create candidate directory under target");
        std::fs::write(&path, bytes).expect("write candidate under target");
        eprintln!("JEFF_ORACLE_CANDIDATE={}", path.display());
        return;
    }

    let expected = read_jsonl(&fixture_path()).expect("read committed Jeff oracle fixture");
    let horizon = if full { FULL_HORIZON } else { NORMAL_HORIZON };
    let actual = run_transcript(horizon);
    compare_transcripts(&expected, &actual, horizon).unwrap_or_else(|failure| {
        panic!("Jeff live-controller transcript diverged:\n{failure}");
    });
}

fn run_transcript(horizon: u32) -> Vec<TranscriptRecord> {
    let players = fixture_players();
    let player_slots: Vec<_> = players
        .iter()
        .map(|player| (player.id, player.team_id))
        .collect();
    let map = Map::load_for_players(MAP_NAME, &player_slots, FIXTURE_SEED)
        .expect("load authored Chokes oracle map");
    let map_metadata = Map::metadata_for_name(MAP_NAME).expect("load Chokes metadata");
    let mut game = Game::new_with_random_ai_profiles_and_map_metadata(
        &players,
        FIXTURE_SEED,
        map,
        map_metadata.clone(),
    );
    let start = game.start_payload();
    let profile = profile_identity_by_id(JEFFS_AI_ID).expect("Jeff profile identity");
    let player_specs = players
        .iter()
        .map(|player| {
            let start_player = start
                .players
                .iter()
                .find(|candidate| candidate.id == player.id)
                .expect("fixture player start");
            PlayerSpec {
                player_id: player.id,
                team_id: player.team_id,
                faction_id: player.faction_id.clone(),
                profile_id: JEFFS_AI_ID.to_string(),
                profile_fingerprint: profile.fingerprint.clone(),
                start_tile_x: start_player.start_tile_x,
                start_tile_y: start_player.start_tile_y,
            }
        })
        .collect();
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        fixture_id: FIXTURE_ID.to_string(),
        canonical_encoding:
            "UTF-8 JSON Lines; serde struct field order; fingerprint floats rounded to 1/1024; one compact JSON object and LF per record"
                .to_string(),
        hash_algorithm:
            "FNV-1a 64 over canonical serde_json values, lowercase hex with fnv1a64 prefix"
                .to_string(),
        seed: FIXTURE_SEED,
        full_horizon: FULL_HORIZON,
        normal_horizon: NORMAL_HORIZON,
        map_name: map_metadata.name,
        map_schema_version: map_metadata.schema_version,
        map_content_hash: map_metadata.content_hash,
        start_payload_fingerprint: fingerprint(&start),
        players: player_specs,
    };
    let mut records = Vec::with_capacity(horizon as usize + 1);
    records.push(TranscriptRecord::Manifest(manifest));

    let mut controllers: Vec<_> = players
        .iter()
        .map(|player| AiController::with_profile_id(player.id, JEFFS_AI_ID))
        .collect();
    let mut command_log_cursor = 0usize;
    for _ in 0..horizon {
        let tick = game.tick_count();
        let start = game.start_payload();
        let alive_player_ids = game.primary_base_alive_players();
        let mut controller_records = Vec::with_capacity(controllers.len());
        let mut pending_commands = Vec::new();

        // This intentionally mirrors lobby/live_tick.rs: collect every controller result in
        // controller order before enqueueing any result.
        for controller in &mut controllers {
            let player_id = controller.player_id();
            if !alive_player_ids.contains(&player_id) {
                controller_records.push(ControllerRecord {
                    player_id,
                    profile_id: controller.profile_id().to_string(),
                    invocation: Invocation::SkippedDead,
                    snapshot_fingerprint: None,
                    retreat_commands: Vec::new(),
                    emitted_commands: Vec::new(),
                    decision_trace: None,
                });
                continue;
            }
            let snapshot = game.snapshot_for(player_id);
            let snapshot_fingerprint = fingerprint(&snapshot);
            let retreat_commands = game.worker_retreat_commands_for(player_id);
            let emitted_commands = controller.think(AiThinkContext {
                start: &start,
                snapshot: &snapshot,
                alive_player_ids: &alive_player_ids,
                retreat_commands: retreat_commands.clone(),
            });
            let decision_trace = controller
                .latest_decision_trace()
                .filter(|trace| trace.trace_tick == tick)
                .map(|trace| TraceRecord {
                    tick: trace.trace_tick,
                    lines: trace.lines,
                });
            pending_commands.extend(
                emitted_commands
                    .iter()
                    .cloned()
                    .map(|command| (player_id, command)),
            );
            controller_records.push(ControllerRecord {
                player_id,
                profile_id: controller.profile_id().to_string(),
                invocation: Invocation::Invoked,
                snapshot_fingerprint: Some(snapshot_fingerprint),
                retreat_commands,
                emitted_commands,
                decision_trace,
            });
        }
        for (player_id, command) in pending_commands {
            game.enqueue(player_id, command);
        }

        let recipient_events = game.tick();
        let command_log = game.command_log();
        let command_log_delta = command_log[command_log_cursor..].to_vec();
        command_log_cursor = command_log.len();
        let recipient_event_fingerprints = recipient_events
            .iter()
            .map(|(player_id, events)| RecipientFingerprint {
                player_id: *player_id,
                fingerprint: fingerprint(events),
            })
            .collect();
        let objective_alive_player_ids = game.primary_base_alive_players();
        let snapshot_fingerprints = players
            .iter()
            .map(|player| PlayerFingerprint {
                player_id: player.id,
                fingerprint: fingerprint(&game.snapshot_for(player.id)),
            })
            .collect();
        records.push(TranscriptRecord::Tick(TickRecord {
            tick,
            alive_player_ids,
            controllers: controller_records,
            post_tick: PostTickRecord {
                command_log_delta,
                recipient_event_fingerprints,
                objective_alive_player_ids,
                snapshot_fingerprints,
            },
        }));
    }
    records
}

fn compare_transcripts(
    expected: &[TranscriptRecord],
    actual: &[TranscriptRecord],
    horizon: u32,
) -> Result<(), String> {
    let (expected_manifest, expected_ticks) = split_transcript(expected)?;
    let (actual_manifest, actual_ticks) = split_transcript(actual)?;
    if expected_manifest != actual_manifest {
        return Err(format!(
            "classification=input_drift scenario={FIXTURE_ID} tick=manifest\nexpected metadata={}\nactual metadata={}",
            json(expected_manifest),
            json(actual_manifest)
        ));
    }
    if expected_ticks.len() < horizon as usize || actual_ticks.len() < horizon as usize {
        return Err(format!(
            "classification=post_tick_drift scenario={FIXTURE_ID} tick=length expected_records={} actual_records={} required_horizon={horizon}\nfixture metadata={}",
            expected_ticks.len(), actual_ticks.len(), json(expected_manifest)
        ));
    }
    for index in 0..horizon as usize {
        compare_tick(
            expected_manifest,
            expected_ticks[index],
            actual_ticks[index],
        )?;
    }
    Ok(())
}

fn compare_tick(
    manifest: &Manifest,
    expected: &TickRecord,
    actual: &TickRecord,
) -> Result<(), String> {
    if expected.tick != actual.tick || expected.alive_player_ids != actual.alive_player_ids {
        return Err(tick_failure(
            "input_drift",
            manifest,
            expected,
            actual,
            None,
            "pre-tick number or ordered alive IDs changed",
        ));
    }
    if expected.controllers.len() != actual.controllers.len() {
        return Err(tick_failure(
            "input_drift",
            manifest,
            expected,
            actual,
            None,
            "controller count changed",
        ));
    }
    for (expected_controller, actual_controller) in
        expected.controllers.iter().zip(&actual.controllers)
    {
        let player = Some(expected_controller.player_id);
        if expected_controller.player_id != actual_controller.player_id
            || expected_controller.profile_id != actual_controller.profile_id
            || expected_controller.invocation != actual_controller.invocation
            || expected_controller.snapshot_fingerprint != actual_controller.snapshot_fingerprint
            || expected_controller.retreat_commands != actual_controller.retreat_commands
        {
            return Err(tick_failure(
                "input_drift",
                manifest,
                expected,
                actual,
                player,
                "controller identity, invocation, snapshot, or retreat input changed",
            ));
        }
        if expected_controller.emitted_commands != actual_controller.emitted_commands {
            let command_index = first_difference(
                &expected_controller.emitted_commands,
                &actual_controller.emitted_commands,
            );
            let detail = format!(
                "command index={command_index} expected={} actual={}",
                expected_controller
                    .emitted_commands
                    .get(command_index)
                    .map(json)
                    .unwrap_or_else(|| "<missing>".to_string()),
                actual_controller
                    .emitted_commands
                    .get(command_index)
                    .map(json)
                    .unwrap_or_else(|| "<missing>".to_string())
            );
            return Err(tick_failure(
                "command_drift",
                manifest,
                expected,
                actual,
                player,
                &detail,
            ));
        }
        if expected_controller.decision_trace != actual_controller.decision_trace {
            return Err(tick_failure(
                "trace_drift",
                manifest,
                expected,
                actual,
                player,
                "decision trace changed",
            ));
        }
    }
    if expected.post_tick != actual.post_tick {
        return Err(tick_failure(
            "post_tick_drift",
            manifest,
            expected,
            actual,
            None,
            "command-log delta, recipient events, objective IDs, or post-tick snapshots changed",
        ));
    }
    Ok(())
}

fn tick_failure(
    classification: &str,
    manifest: &Manifest,
    expected: &TickRecord,
    actual: &TickRecord,
    player_id: Option<u32>,
    detail: &str,
) -> String {
    let player_label = player_id
        .map(|player| player.to_string())
        .unwrap_or_else(|| "all".to_string());
    let expected_controller = controller_summary(expected, player_id);
    let actual_controller = controller_summary(actual, player_id);
    format!(
        "classification={classification} scenario={} tick={} player={player_label}\n{detail}\nexpected invocation/input/commands/trace={}\nactual invocation/input/commands/trace={}\nexpected events/post-state={}\nactual events/post-state={}\nfixture metadata={}",
        manifest.fixture_id,
        expected.tick,
        expected_controller,
        actual_controller,
        json(&expected.post_tick),
        json(&actual.post_tick),
        json(manifest),
    )
}

fn controller_summary(tick: &TickRecord, player_id: Option<u32>) -> String {
    match player_id {
        Some(player_id) => tick
            .controllers
            .iter()
            .find(|controller| controller.player_id == player_id)
            .map(json)
            .unwrap_or_else(|| "<missing>".to_string()),
        None => json(&tick.controllers),
    }
}

fn split_transcript(records: &[TranscriptRecord]) -> Result<(&Manifest, Vec<&TickRecord>), String> {
    let Some(TranscriptRecord::Manifest(manifest)) = records.first() else {
        return Err("transcript does not start with one manifest record".to_string());
    };
    let mut ticks = Vec::with_capacity(records.len().saturating_sub(1));
    for record in &records[1..] {
        match record {
            TranscriptRecord::Manifest(_) => {
                return Err("transcript contains an extra manifest record".to_string())
            }
            TranscriptRecord::Tick(tick) => ticks.push(tick),
        }
    }
    Ok((manifest, ticks))
}

fn encode_jsonl(records: &[TranscriptRecord]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for record in records {
        serde_json::to_writer(&mut bytes, record).expect("serialize oracle record");
        bytes.push(b'\n');
    }
    bytes
}

fn read_jsonl(path: &Path) -> Result<Vec<TranscriptRecord>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    text.lines()
        .enumerate()
        .map(|(index, line)| {
            serde_json::from_str(line).map_err(|error| {
                format!(
                    "invalid oracle record {} in {}: {error}",
                    index + 1,
                    path.display()
                )
            })
        })
        .collect()
}

fn fingerprint<T: Serialize + ?Sized>(value: &T) -> String {
    let mut canonical = serde_json::to_value(value).expect("serialize fingerprint input");
    canonicalize_fingerprint_value(&mut canonical);
    let bytes = serde_json::to_vec(&canonical).expect("serialize canonical fingerprint input");
    format!("fnv1a64:{:016x}", fnv1a64(&bytes))
}

fn canonicalize_fingerprint_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                canonicalize_fingerprint_value(value);
            }
        }
        serde_json::Value::Object(fields) => {
            for value in fields.values_mut() {
                canonicalize_fingerprint_value(value);
            }
        }
        serde_json::Value::Number(number) if number.is_f64() => {
            let Some(value) = number.as_f64() else {
                return;
            };
            let quantized = (value * FINGERPRINT_FLOAT_STEPS_PER_UNIT).round()
                / FINGERPRINT_FLOAT_STEPS_PER_UNIT;
            let quantized = if quantized == 0.0 { 0.0 } else { quantized };
            *number = serde_json::Number::from_f64(quantized)
                .expect("finite simulation float in oracle fingerprint");
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

fn first_difference<T: PartialEq>(expected: &[T], actual: &[T]) -> usize {
    expected
        .iter()
        .zip(actual)
        .position(|(expected, actual)| expected != actual)
        .unwrap_or_else(|| expected.len().min(actual.len()))
}

fn json<T: Serialize + ?Sized>(value: &T) -> String {
    serde_json::to_string(value).expect("serialize diagnostic")
}

fn fixture_players() -> Vec<PlayerInit> {
    vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Jeff West".to_string(),
            color: "#4cc9f0".to_string(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Jeff East".to_string(),
            color: "#f72585".to_string(),
            is_ai: true,
        },
    ]
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH)
}

fn candidate_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("jeff-live-oracle")
        .join("candidate-v1.jsonl")
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn decode_round_trip<T: Serialize + DeserializeOwned>(value: &T) -> T {
    serde_json::from_slice(&serde_json::to_vec(value).expect("encode test value"))
        .expect("decode test value")
}

#[cfg(test)]
mod comparator_tests {
    use super::*;

    fn sample_transcript(commands: Vec<SimCommand>) -> Vec<TranscriptRecord> {
        let manifest = Manifest {
            schema_version: SCHEMA_VERSION,
            fixture_id: FIXTURE_ID.to_string(),
            canonical_encoding: "jsonl".to_string(),
            hash_algorithm: "fnv1a64".to_string(),
            seed: FIXTURE_SEED,
            full_horizon: 1,
            normal_horizon: 1,
            map_name: MAP_NAME.to_string(),
            map_schema_version: 1,
            map_content_hash: "map".to_string(),
            start_payload_fingerprint: "start".to_string(),
            players: Vec::new(),
        };
        let controller = ControllerRecord {
            player_id: 1,
            profile_id: JEFFS_AI_ID.to_string(),
            invocation: Invocation::Invoked,
            snapshot_fingerprint: Some("snapshot".to_string()),
            retreat_commands: Vec::new(),
            emitted_commands: commands,
            decision_trace: Some(TraceRecord {
                tick: 0,
                lines: vec!["trace".to_string()],
            }),
        };
        vec![
            TranscriptRecord::Manifest(manifest),
            TranscriptRecord::Tick(TickRecord {
                tick: 0,
                alive_player_ids: vec![1, 2],
                controllers: vec![controller],
                post_tick: PostTickRecord {
                    command_log_delta: Vec::new(),
                    recipient_event_fingerprints: Vec::new(),
                    objective_alive_player_ids: vec![1, 2],
                    snapshot_fingerprints: vec![PlayerFingerprint {
                        player_id: 1,
                        fingerprint: "post".to_string(),
                    }],
                },
            }),
        ]
    }

    fn moves() -> Vec<SimCommand> {
        vec![
            SimCommand::Move {
                units: vec![10],
                x: 100.0,
                y: 200.0,
                queued: false,
            },
            SimCommand::HoldPosition {
                units: vec![10],
                queued: true,
            },
        ]
    }

    #[test]
    fn jeff_live_oracle_fnv1a64_matches_the_published_test_vector() {
        assert_eq!(fnv1a64(b"hello"), 0xa430_d846_80aa_bd0b);
    }

    #[test]
    fn jeff_live_oracle_quantizes_subpixel_float_noise_only() {
        let baseline = serde_json::json!({
            "position": 100.125,
            "integer": 7,
            "nested": [20.5, { "facing": -0.0 }],
        });
        let subpixel_noise = serde_json::json!({
            "position": 100.125_2,
            "integer": 7,
            "nested": [20.500_2, { "facing": 0.0 }],
        });
        let meaningful_change = serde_json::json!({
            "position": 100.13,
            "integer": 7,
            "nested": [20.5, { "facing": 0.0 }],
        });

        assert_eq!(fingerprint(&baseline), fingerprint(&subpixel_noise));
        assert_ne!(fingerprint(&baseline), fingerprint(&meaningful_change));
    }

    #[test]
    fn jeff_live_oracle_reports_missing_extra_reordered_and_field_changed_commands() {
        let expected = sample_transcript(moves());
        let mut variants = Vec::new();
        variants.push(sample_transcript(vec![moves()[0].clone()]));
        let mut extra = moves();
        extra.push(SimCommand::Stop { units: vec![10] });
        variants.push(sample_transcript(extra));
        let mut reordered = moves();
        reordered.reverse();
        variants.push(sample_transcript(reordered));
        let mut field_changed = moves();
        if let SimCommand::Move { x, .. } = &mut field_changed[0] {
            *x = 101.0;
        }
        variants.push(sample_transcript(field_changed));

        for actual in variants {
            let failure = compare_transcripts(&expected, &actual, 1).unwrap_err();
            assert!(failure.contains("classification=command_drift"));
            assert!(failure.contains("command index="));
            assert!(failure.contains("expected="));
            assert!(failure.contains("actual="));
            assert!(failure.contains("snapshot"));
        }
    }

    #[test]
    fn jeff_live_oracle_classifies_input_trace_and_post_tick_drift() {
        let expected = sample_transcript(moves());

        let mut input = decode_round_trip(&expected);
        let TranscriptRecord::Tick(tick) = &mut input[1] else {
            unreachable!()
        };
        tick.controllers[0].snapshot_fingerprint = Some("changed".to_string());
        assert!(compare_transcripts(&expected, &input, 1)
            .unwrap_err()
            .contains("classification=input_drift"));

        let mut retreat = decode_round_trip(&expected);
        let TranscriptRecord::Tick(tick) = &mut retreat[1] else {
            unreachable!()
        };
        tick.controllers[0].retreat_commands = vec![moves()[0].clone()];
        assert!(compare_transcripts(&expected, &retreat, 1)
            .unwrap_err()
            .contains("classification=input_drift"));

        let mut trace = decode_round_trip(&expected);
        let TranscriptRecord::Tick(tick) = &mut trace[1] else {
            unreachable!()
        };
        tick.controllers[0].decision_trace.as_mut().unwrap().lines[0] = "changed".to_string();
        assert!(compare_transcripts(&expected, &trace, 1)
            .unwrap_err()
            .contains("classification=trace_drift"));

        let mut post = decode_round_trip(&expected);
        let TranscriptRecord::Tick(tick) = &mut post[1] else {
            unreachable!()
        };
        tick.post_tick.snapshot_fingerprints[0].fingerprint = "changed".to_string();
        assert!(compare_transcripts(&expected, &post, 1)
            .unwrap_err()
            .contains("classification=post_tick_drift"));
    }
}
