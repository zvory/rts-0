//! Client-independent performance harness for the canonical Hellhole scenario.
//!
//! The harness drives the public simulation seam directly, then builds, compacts, and serializes
//! the same full-world projection consumed by the visual Lab lane. It never starts HTTP,
//! WebSockets, or a browser, so client frame rate cannot throttle the server measurement.

use std::process;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::lobby::compact_snapshot_for_wire;
use crate::protocol::{default_snapshot_codec, encode_snapshot_frame, Event, SnapshotFrame};
use rts_sim::perf;

use super::hellhole_snapshot_stream::{
    apply_hellhole_scenario_actions, build_hellhole_game, union_events, HellholeActionCounts,
    TICK_RATE_HZ,
};

const DEFAULT_TICKS: u32 = 900;
const MAX_TICKS: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CliConfig {
    ticks: u32,
    json: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HarnessSummary {
    mode: &'static str,
    client_connected: bool,
    network_transport: bool,
    ticks: u32,
    simulated_seconds: f64,
    elapsed_ms: u64,
    realtime_factor: f64,
    initial_entities: usize,
    final_entities: usize,
    serialized_snapshots: u32,
    snapshot_bytes: u64,
    attack_events: usize,
    projectile_events: usize,
    death_events: usize,
    shuttle_commands: usize,
    selected_units: usize,
    respawn_batches: usize,
    respawned_units: usize,
    minimum_snapshot_entities: usize,
    last_combat_tick: u32,
    tick: DurationSummary,
    snapshot_build: DurationSummary,
    snapshot_compact: DurationSummary,
    snapshot_serialize: DurationSummary,
    api_round_trip: DurationSummary,
    snapshot_payload: CountSummary,
}

#[derive(Debug, Default)]
struct DurationSeries {
    samples: Vec<Duration>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DurationSummary {
    samples: usize,
    total_us: u64,
    avg_us: u64,
    p95_us: u64,
    p99_us: u64,
    max_us: u64,
}

#[derive(Debug, Default)]
struct CountSeries {
    samples: Vec<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CountSummary {
    samples: usize,
    total: u64,
    avg: usize,
    p95: usize,
    max: usize,
}

impl DurationSeries {
    fn record(&mut self, value: Duration) {
        self.samples.push(value);
    }

    fn summarize(&self) -> DurationSummary {
        let total: Duration = self.samples.iter().copied().sum();
        DurationSummary {
            samples: self.samples.len(),
            total_us: duration_us(total),
            avg_us: duration_us(div_duration(total, self.samples.len())),
            p95_us: duration_us(percentile(&self.samples, 95, 100)),
            p99_us: duration_us(percentile(&self.samples, 99, 100)),
            max_us: duration_us(self.samples.iter().copied().max().unwrap_or_default()),
        }
    }
}

impl CountSeries {
    fn record(&mut self, value: usize) {
        self.samples.push(value);
    }

    fn summarize(&self) -> CountSummary {
        CountSummary {
            samples: self.samples.len(),
            total: self
                .samples
                .iter()
                .fold(0u64, |sum, value| sum.saturating_add(*value as u64)),
            avg: if self.samples.is_empty() {
                0
            } else {
                self.samples.iter().sum::<usize>() / self.samples.len()
            },
            p95: percentile(&self.samples, 95, 100),
            max: self.samples.iter().copied().max().unwrap_or_default(),
        }
    }
}

pub fn run_from_env() {
    let Some(config) = parse_args_or_exit() else {
        return;
    };
    perf::PerfConfig::global().enforce_release_build_for_server();

    match run_harness(config) {
        Ok(summary) if config.json => match serde_json::to_string_pretty(&summary) {
            Ok(json) => println!("{json}"),
            Err(err) => {
                eprintln!("hellhole-perf-harness failed to encode its summary: {err}");
                process::exit(1);
            }
        },
        Ok(summary) => print_summary(&summary),
        Err(err) => {
            eprintln!("hellhole-perf-harness failed: {err}");
            process::exit(1);
        }
    }
}

fn run_harness(config: CliConfig) -> Result<HarnessSummary, String> {
    let (mut game, mut driver) = build_hellhole_game()?;
    let initial_entities = game.perf_entity_counts().entities;
    let started = Instant::now();
    let mut tick_series = DurationSeries::default();
    let mut snapshot_build_series = DurationSeries::default();
    let mut snapshot_compact_series = DurationSeries::default();
    let mut snapshot_serialize_series = DurationSeries::default();
    let mut api_round_trip_series = DurationSeries::default();
    let mut payload_series = CountSeries::default();
    let mut snapshot_bytes = 0u64;
    let mut attack_events = 0usize;
    let mut projectile_events = 0usize;
    let mut last_combat_tick = 0u32;
    let mut death_events = 0usize;
    let mut action_counts = HellholeActionCounts::default();
    let mut minimum_snapshot_entities = initial_entities;

    while game.tick_count() < config.ticks {
        let round_trip_started = Instant::now();
        action_counts.add(apply_hellhole_scenario_actions(&mut game, &mut driver)?);
        let post_action_counts = game.perf_entity_counts();
        let post_action_entities = post_action_counts.entities;
        if post_action_entities != initial_entities {
            return Err(format!(
                "Hellhole pre-tick entity count changed at tick {}: {post_action_entities} != {initial_entities} (units={}, buildings={}, resources={}, respawn_batches={}, respawned_units={})",
                game.tick_count(),
                post_action_counts.units,
                post_action_counts.buildings,
                post_action_counts.resources,
                action_counts.respawn_batches,
                action_counts.respawned_units,
            ));
        }

        let tick_started = Instant::now();
        let event_sets = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.tick()))
            .map_err(|payload| {
                format!(
                    "Game::tick panicked at tick {}: {}",
                    game.tick_count(),
                    panic_payload_to_string(&payload)
                )
            })?;
        tick_series.record(tick_started.elapsed());

        let events = union_events(event_sets.iter().map(|(_, events)| events));
        let mut combat_active = false;
        for event in &events {
            let attack = matches!(event, Event::Attack { .. });
            let projectile = matches!(
                event,
                Event::MortarLaunch { .. }
                    | Event::ArtilleryTarget { .. }
                    | Event::PanzerfaustLaunch { .. }
            );
            attack_events += usize::from(attack);
            projectile_events += usize::from(projectile);
            death_events += usize::from(matches!(event, Event::Death { .. }));
            combat_active |= attack || projectile;
        }
        if combat_active {
            last_combat_tick = game.tick_count();
        }

        let snapshot_started = Instant::now();
        let mut snapshot = game.snapshot_full_for(1);
        snapshot.events = events;
        minimum_snapshot_entities = minimum_snapshot_entities.min(snapshot.entities.len());
        snapshot.net_status = Default::default();
        snapshot_build_series.record(snapshot_started.elapsed());

        let compact_started = Instant::now();
        compact_snapshot_for_wire(&mut snapshot);
        snapshot_compact_series.record(compact_started.elapsed());

        let serialize_started = Instant::now();
        let frame = encode_snapshot_frame(&snapshot, default_snapshot_codec())
            .map_err(|err| format!("failed to serialize snapshot tick {}: {err}", snapshot.tick))?;
        snapshot_serialize_series.record(serialize_started.elapsed());
        let payload_len = snapshot_frame_len(&frame);
        payload_series.record(payload_len);
        snapshot_bytes = snapshot_bytes.saturating_add(payload_len as u64);
        api_round_trip_series.record(round_trip_started.elapsed());
    }

    if game.tick_count() > 90 && last_combat_tick < game.tick_count() - 90 {
        return Err(format!(
            "Hellhole combat went quiet before tick {} (last combat tick {last_combat_tick})",
            game.tick_count()
        ));
    }

    let elapsed = started.elapsed();
    let simulated_seconds = f64::from(game.tick_count()) / f64::from(TICK_RATE_HZ);
    Ok(HarnessSummary {
        mode: "isolated-server-api",
        client_connected: false,
        network_transport: false,
        ticks: game.tick_count(),
        simulated_seconds,
        elapsed_ms: duration_ms(elapsed),
        realtime_factor: if elapsed.is_zero() {
            0.0
        } else {
            simulated_seconds / elapsed.as_secs_f64()
        },
        initial_entities,
        final_entities: game.perf_entity_counts().entities,
        serialized_snapshots: game.tick_count(),
        snapshot_bytes,
        attack_events,
        projectile_events,
        death_events,
        shuttle_commands: action_counts.shuttle_commands,
        selected_units: action_counts.selected_units,
        respawn_batches: action_counts.respawn_batches,
        respawned_units: action_counts.respawned_units,
        minimum_snapshot_entities,
        last_combat_tick,
        tick: tick_series.summarize(),
        snapshot_build: snapshot_build_series.summarize(),
        snapshot_compact: snapshot_compact_series.summarize(),
        snapshot_serialize: snapshot_serialize_series.summarize(),
        api_round_trip: api_round_trip_series.summarize(),
        snapshot_payload: payload_series.summarize(),
    })
}

fn snapshot_frame_len(frame: &SnapshotFrame) -> usize {
    match frame {
        SnapshotFrame::Text(text) => text.len(),
        SnapshotFrame::Binary(bytes) => bytes.len(),
    }
}

fn percentile<T: Ord + Copy + Default>(values: &[T], numerator: usize, denominator: usize) -> T {
    if values.is_empty() || denominator == 0 {
        return T::default();
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let rank = (sorted.len() * numerator).div_ceil(denominator);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn div_duration(duration: Duration, divisor: usize) -> Duration {
    u32::try_from(divisor)
        .ok()
        .filter(|divisor| *divisor > 0)
        .map(|divisor| duration / divisor)
        .unwrap_or_default()
}

fn duration_us(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn parse_args_or_exit() -> Option<CliConfig> {
    match parse_args(std::env::args().skip(1)) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}\n");
            print_usage();
            process::exit(2);
        }
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<CliConfig>, String> {
    let mut config = CliConfig {
        ticks: DEFAULT_TICKS,
        json: false,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                return Ok(None);
            }
            "--ticks" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--ticks requires a value".to_string())?;
                config.ticks = value
                    .parse()
                    .map_err(|_| format!("--ticks requires a u32 value, got {value:?}"))?;
            }
            "--json" => config.json = true,
            _ => return Err(format!("unknown flag: {arg}")),
        }
    }
    if config.ticks == 0 || config.ticks > MAX_TICKS {
        return Err(format!("--ticks must be between 1 and {MAX_TICKS}"));
    }
    Ok(Some(config))
}

fn print_summary(summary: &HarnessSummary) {
    println!("Hellhole server perf harness (isolated)");
    println!("client: disconnected  transport: none  projection: full-world MessagePack");
    println!(
        "ticks: {}  simulated_seconds: {:.2}  elapsed_ms: {}  realtime_factor: {:.2}x",
        summary.ticks, summary.simulated_seconds, summary.elapsed_ms, summary.realtime_factor
    );
    println!(
        "entities: {} -> {}  snapshots: {}  snapshot_bytes: {}",
        summary.initial_entities,
        summary.final_entities,
        summary.serialized_snapshots,
        summary.snapshot_bytes
    );
    println!(
        "events: attacks={} projectiles={} deaths={} last_combat_tick={}",
        summary.attack_events,
        summary.projectile_events,
        summary.death_events,
        summary.last_combat_tick
    );
    println!(
        "churn: shuttle_commands={} selected_units={} respawn_batches={} respawned_units={} minimum_snapshot_entities={}",
        summary.shuttle_commands,
        summary.selected_units,
        summary.respawn_batches,
        summary.respawned_units,
        summary.minimum_snapshot_entities
    );
    println!();
    println!(
        "{:<22} {:>8} {:>10} {:>9} {:>9} {:>9} {:>9}",
        "part", "samples", "total_us", "avg_us", "p95_us", "p99_us", "max_us"
    );
    for (name, metrics) in [
        ("game.tick", &summary.tick),
        ("snapshot.build", &summary.snapshot_build),
        ("snapshot.compact", &summary.snapshot_compact),
        ("snapshot.serialize", &summary.snapshot_serialize),
        ("API in/out total", &summary.api_round_trip),
    ] {
        println!(
            "{name:<22} {:>8} {:>10} {:>9} {:>9} {:>9} {:>9}",
            metrics.samples,
            metrics.total_us,
            metrics.avg_us,
            metrics.p95_us,
            metrics.p99_us,
            metrics.max_us
        );
    }
    println!(
        "snapshot_payload_bytes: samples={} total={} avg={} p95={} max={}",
        summary.snapshot_payload.samples,
        summary.snapshot_payload.total,
        summary.snapshot_payload.avg,
        summary.snapshot_payload.p95,
        summary.snapshot_payload.max
    );
}

fn panic_payload_to_string(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        message.to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "panic without string payload".to_string()
    }
}

fn print_usage() {
    println!(
        "Usage:
  cargo run --release --bin hellhole-perf-harness -- [options]

Options:
  --ticks <u32>  Authoritative ticks to run (default: {DEFAULT_TICKS}, max: {MAX_TICKS})
  --json         Print the aggregate summary as JSON
  -h, --help     Print this help

This mode starts no HTTP server, WebSocket, or browser. It drives Game directly and includes one
full-world snapshot projection, compaction pass, and production MessagePack encoding per tick."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_defaults_to_the_isolated_thirty_second_run() {
        assert_eq!(
            parse_args(Vec::<String>::new()).unwrap(),
            Some(CliConfig {
                ticks: DEFAULT_TICKS,
                json: false,
            })
        );
        assert!(parse_args(["--ticks".to_string(), "0".to_string()]).is_err());
        assert!(parse_args(["--integrated".to_string()]).is_err());
    }

    #[test]
    fn smoke_run_drives_one_snapshot_out_for_each_tick_in() {
        let summary = run_harness(CliConfig {
            ticks: 2,
            json: false,
        })
        .unwrap();
        assert_eq!(summary.mode, "isolated-server-api");
        assert!(!summary.client_connected);
        assert!(!summary.network_transport);
        assert_eq!(summary.ticks, 2);
        assert_eq!(summary.serialized_snapshots, 2);
        assert_eq!(summary.initial_entities, 380);
        assert_eq!(summary.final_entities, 380);
        assert!(summary.snapshot_bytes > 0);
    }
}
