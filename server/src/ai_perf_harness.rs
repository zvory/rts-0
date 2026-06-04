//! Four-AI performance harness.
//!
//! This developer tool runs one local four-AI match with the same tick perf collector used by the
//! room task. It also builds, compacts, and serializes per-player snapshots so the logged tick
//! summaries include simulation and fanout work without requiring browser clients.
#![allow(dead_code)]

mod config;
mod game;
mod perf;
mod protocol;
mod rules;
#[path = "lobby/snapshots.rs"]
mod snapshots;

use std::collections::HashMap;
use std::process;
use std::time::{Duration, Instant};

use game::{Game, PlayerInit};
use protocol::{serialize_compact_snapshot, Event};
use snapshots::compact_snapshot_for_wire;
use tracing_subscriber::EnvFilter;

const DEFAULT_SEED: u32 = 0;
const DEFAULT_TICKS: u32 = 20_000;
const DEFAULT_PERF_MODE: &str = "sample";
const DEFAULT_SAMPLE_EVERY: u32 = 300;
const ROOM_NAME: &str = "ai-perf-harness";

#[derive(Debug)]
struct CliConfig {
    seed: u32,
    ticks: u32,
    perf_mode: Option<String>,
    sample_every: Option<u32>,
    log_snapshots: bool,
}

#[derive(Debug)]
struct HarnessSummary {
    seed: u32,
    max_ticks: u32,
    ticks: u32,
    completed_by_elimination: bool,
    alive_players: Vec<u32>,
    elapsed: Duration,
    snapshot_bytes: u64,
    serialized_snapshots: u64,
    attack_events: usize,
    death_events: usize,
    final_counts: perf::EntityCounts,
}

fn main() {
    let Some(config) = parse_args_or_exit() else {
        return;
    };

    if let Some(perf_mode) = &config.perf_mode {
        std::env::set_var("RTS_PERF", perf_mode);
    } else {
        install_default_env("RTS_PERF", DEFAULT_PERF_MODE);
    }
    if let Some(sample_every) = config.sample_every {
        std::env::set_var("RTS_PERF_SAMPLE_EVERY", sample_every.to_string());
    } else {
        install_default_env("RTS_PERF_SAMPLE_EVERY", &DEFAULT_SAMPLE_EVERY.to_string());
    }
    if config.log_snapshots {
        install_default_env("RTS_PERF_LOG_SNAPSHOTS", "1");
    }
    install_default_env("RUST_LOG", "info,server::perf=debug");

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    perf::PerfConfig::global().enforce_release_build_for_server();

    match run_harness(config) {
        Ok(summary) => print_summary(&summary),
        Err(err) => {
            eprintln!("ai-perf-harness failed: {err}");
            process::exit(1);
        }
    }
}

fn run_harness(config: CliConfig) -> Result<HarnessSummary, String> {
    if config.ticks == 0 {
        return Err("tick cap must be greater than zero".to_string());
    }

    let players = four_ai_players();
    let mut game = Game::new_with_random_ai_profiles(&players, config.seed);
    let started = Instant::now();
    let mut snapshot_bytes = 0u64;
    let mut serialized_snapshots = 0u64;
    let mut attack_events = 0usize;
    let mut death_events = 0usize;

    while game.tick_count() < config.ticks {
        let alive = game.alive_players();
        if alive.len() <= 1 {
            break;
        }

        let tick_start = Instant::now();
        let mut perf_tick = perf::TickPerf::maybe_new();
        let game_tick_start = Instant::now();
        let tick_events = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            game.tick_with_perf(perf_tick.as_mut())
        }))
        .map_err(|payload| {
            format!(
                "Game::tick panicked during perf harness: {}",
                panic_payload_to_string(&payload)
            )
        })?;
        if let Some(perf_tick) = perf_tick.as_mut() {
            perf_tick.record_phase("game_tick", game_tick_start.elapsed());
        }

        let mut per_player_events: HashMap<u32, Vec<Event>> = tick_events.into_iter().collect();
        for events in per_player_events.values() {
            for event in events {
                match event {
                    Event::Attack { .. } => attack_events += 1,
                    Event::Death { .. } => death_events += 1,
                    Event::Build { .. } | Event::Notice { .. } => {}
                }
            }
        }

        let fanout_start = Instant::now();
        for player in &players {
            let snapshot_start = Instant::now();
            let mut snapshot = game.snapshot_for(player.id);
            if let Some(mut events) = per_player_events.remove(&player.id) {
                snapshot.events.append(&mut events);
            }
            let snapshot_duration = snapshot_start.elapsed();
            let entity_count = snapshot.entities.len();
            let resource_delta_count = snapshot.resource_deltas.len();
            let event_count = snapshot.events.len();

            let compact_start = Instant::now();
            compact_snapshot_for_wire(&mut snapshot);
            let compact_duration = compact_start.elapsed();

            let serialize_start = Instant::now();
            let payload = serialize_compact_snapshot(&snapshot)
                .map_err(|err| format!("failed to serialize snapshot: {err}"))?;
            let serialize_duration = serialize_start.elapsed();
            snapshot_bytes = snapshot_bytes.saturating_add(payload.len() as u64);
            serialized_snapshots = serialized_snapshots.saturating_add(1);
            perf::log_writer_message(
                player.id,
                "snapshot",
                serialize_duration,
                Duration::ZERO,
                payload.len(),
            );

            if let Some(perf_tick) = perf_tick.as_mut() {
                perf_tick.record_snapshot(perf::SnapshotRecord {
                    player_id: player.id,
                    spectator: false,
                    snapshot: snapshot_duration,
                    compact: compact_duration,
                    entities: entity_count,
                    resource_deltas: resource_delta_count,
                    events: event_count,
                });
                perf_tick.record_enqueue(perf::SnapshotEnqueue::Stored);
            }
        }
        if let Some(perf_tick) = perf_tick.as_mut() {
            perf_tick.record_phase("snapshot_fanout", fanout_start.elapsed());
        }

        let outcome_start = Instant::now();
        let alive = game.alive_players();
        if let Some(perf_tick) = perf_tick.as_mut() {
            perf_tick.record_phase("outcome_checks", outcome_start.elapsed());
            perf_tick.finish(perf::TickContext {
                room: ROOM_NAME,
                tick: game.current_tick(),
                scheduler_lag: Duration::ZERO,
                total: tick_start.elapsed(),
                players: players.len(),
                spectators: 0,
                ai_players: players.len(),
                counts: game.perf_entity_counts(),
            });
        }
        if alive.len() <= 1 {
            break;
        }
    }

    let alive_players = game.alive_players();
    Ok(HarnessSummary {
        seed: config.seed,
        max_ticks: config.ticks,
        ticks: game.tick_count(),
        completed_by_elimination: alive_players.len() <= 1,
        alive_players,
        elapsed: started.elapsed(),
        snapshot_bytes,
        serialized_snapshots,
        attack_events,
        death_events,
        final_counts: game.perf_entity_counts(),
    })
}

fn four_ai_players() -> Vec<PlayerInit> {
    vec![
        PlayerInit {
            id: 1,
            name: "Perf AI 1".to_string(),
            color: "#4878c8".to_string(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            name: "Perf AI 2".to_string(),
            color: "#c84848".to_string(),
            is_ai: true,
        },
        PlayerInit {
            id: 3,
            name: "Perf AI 3".to_string(),
            color: "#30a090".to_string(),
            is_ai: true,
        },
        PlayerInit {
            id: 4,
            name: "Perf AI 4".to_string(),
            color: "#8040c8".to_string(),
            is_ai: true,
        },
    ]
}

fn install_default_env(name: &str, value: &str) {
    if std::env::var_os(name).is_none() {
        std::env::set_var(name, value);
    }
}

fn parse_args_or_exit() -> Option<CliConfig> {
    match parse_args(std::env::args().skip(1)) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            eprintln!();
            print_usage();
            process::exit(2);
        }
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<CliConfig>, String> {
    let mut seed = DEFAULT_SEED;
    let mut ticks = DEFAULT_TICKS;
    let mut perf_mode = None;
    let mut sample_every = None;
    let mut log_snapshots = true;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                return Ok(None);
            }
            "--seed" => {
                seed = parse_u32_flag(&arg, &mut args)?;
            }
            "--ticks" => {
                ticks = parse_u32_flag(&arg, &mut args)?;
            }
            "--perf" => {
                perf_mode = Some(parse_perf_mode(&required_value(&arg, &mut args)?)?);
            }
            "--sample-every" => {
                sample_every = Some(parse_u32_flag(&arg, &mut args)?);
            }
            "--no-log-snapshots" => {
                log_snapshots = false;
            }
            _ => return Err(format!("unknown flag: {arg}")),
        }
    }

    if ticks == 0 {
        return Err("--ticks must be greater than zero".to_string());
    }
    if sample_every == Some(0) {
        return Err("--sample-every must be greater than zero".to_string());
    }

    Ok(Some(CliConfig {
        seed,
        ticks,
        perf_mode,
        sample_every,
        log_snapshots,
    }))
}

fn parse_perf_mode(value: &str) -> Result<String, String> {
    match value {
        "spikes" | "sample" | "full" => Ok(value.to_string()),
        _ => Err(format!(
            "--perf must be spikes, sample, or full; got {value:?}"
        )),
    }
}

fn required_value(flag: &str, args: &mut impl Iterator<Item = String>) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_u32_flag(flag: &str, args: &mut impl Iterator<Item = String>) -> Result<u32, String> {
    let value = required_value(flag, args)?;
    value
        .parse()
        .map_err(|_| format!("{flag} requires a u32 value, got {value:?}"))
}

fn print_summary(summary: &HarnessSummary) {
    println!();
    println!("AI perf harness summary");
    println!(
        "seed: {}  ticks: {}/{}  result: {}",
        summary.seed,
        summary.ticks,
        summary.max_ticks,
        if summary.completed_by_elimination {
            winner_text(&summary.alive_players)
        } else {
            "tick cap reached".to_string()
        }
    );
    println!(
        "elapsed_ms: {}  snapshots: {}  snapshot_bytes: {}",
        summary.elapsed.as_millis(),
        summary.serialized_snapshots,
        summary.snapshot_bytes
    );
    println!(
        "events: attacks={} deaths={}  final_entities={} units={} buildings={} resources={}",
        summary.attack_events,
        summary.death_events,
        summary.final_counts.entities,
        summary.final_counts.units,
        summary.final_counts.buildings,
        summary.final_counts.resources
    );
}

fn winner_text(alive_players: &[u32]) -> String {
    match alive_players {
        [winner] => format!("player {winner} won"),
        [] => "no players alive".to_string(),
        _ => "multiple players alive".to_string(),
    }
}

fn panic_payload_to_string(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic without string payload".to_string()
    }
}

fn print_usage() {
    println!(
        "Usage:
  cargo run --release --bin ai-perf-harness -- [options]

Options:
  --seed <u32>           Match seed (default: {DEFAULT_SEED})
  --ticks <u32>          Tick cap (default: {DEFAULT_TICKS})
  --perf <mode>          Perf mode: spikes, sample, full (default: {DEFAULT_PERF_MODE})
  --sample-every <u32>   Sample interval for sample mode (default: {DEFAULT_SAMPLE_EVERY})
  --no-log-snapshots     Do not default RTS_PERF_LOG_SNAPSHOTS=1
  -h, --help             Print this help

The harness defaults RTS_PERF, RTS_PERF_SAMPLE_EVERY, RTS_PERF_LOG_SNAPSHOTS, and RUST_LOG when
they are not already set. Because perf tracing is enabled by default, run it with --release.
"
    );
}
