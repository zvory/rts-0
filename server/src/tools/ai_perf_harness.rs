//! Four-AI performance harness.
//!
//! This developer tool runs one local four-AI match with the same tick perf collector used by the
//! room task. It also builds, compacts, and serializes per-player snapshots so the logged tick
//! summaries include simulation and fanout work without requiring browser clients.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::process;
use std::time::{Duration, Instant};

use crate::lobby::compact_snapshot_for_wire;
use crate::protocol::{default_snapshot_codec, encode_snapshot_frame, Event, SnapshotFrame};
use crate::structured_log::SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES;
use rand::rngs::SmallRng;
use rand::SeedableRng;
use rts_ai::{AiController, AiThinkContext};
use rts_sim::game::{Game, PlayerInit};
use rts_sim::perf;
use tracing_subscriber::EnvFilter;

const DEFAULT_SEED: u32 = 0;
const DEFAULT_TICKS: u32 = 20_000;
const DEFAULT_PERF_MODE: &str = "sample";
const DEFAULT_SAMPLE_EVERY: u32 = 300;
const ROOM_NAME: &str = "ai-perf-harness";
const TOP_WORST_TICKS: usize = 10;

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
    perf_report: PerfReport,
}

#[derive(Debug, Default)]
struct PerfReport {
    ticks: MetricSeries,
    phases: BTreeMap<&'static str, MetricSeries>,
    snapshot_build: MetricSeries,
    snapshot_compact: MetricSeries,
    snapshot_serialize: MetricSeries,
    snapshot_total: MetricSeries,
    snapshot_entities: CountSeries,
    snapshot_resource_deltas: CountSeries,
    snapshot_events: CountSeries,
    snapshot_payload_bytes: CountSeries,
    worst_ticks: Vec<WorstTick>,
}

#[derive(Debug)]
struct WorstTick {
    tick: u32,
    total: Duration,
    slowest_phase: &'static str,
    slowest_phase_duration: Duration,
    entities: usize,
    units: usize,
    buildings: usize,
    resources: usize,
}

#[derive(Debug, Default)]
struct MetricSeries {
    samples: Vec<Duration>,
}

#[derive(Debug, Default)]
struct CountSeries {
    samples: Vec<usize>,
}

impl PerfReport {
    fn record_snapshot_serialize(&mut self, duration: Duration) {
        self.snapshot_serialize.record(duration);
    }

    fn record_snapshot_payload_bytes(&mut self, bytes: usize) {
        self.snapshot_payload_bytes.record(bytes);
    }

    fn record_total_only_tick(&mut self, tick: u32, total: Duration, counts: perf::EntityCounts) {
        self.ticks.record(total);
        self.record_worst_tick(WorstTick {
            tick,
            total,
            slowest_phase: "none",
            slowest_phase_duration: Duration::ZERO,
            entities: counts.entities,
            units: counts.units,
            buildings: counts.buildings,
            resources: counts.resources,
        });
    }

    fn record_tick(
        &mut self,
        tick: u32,
        total: Duration,
        counts: perf::EntityCounts,
        perf_tick: &perf::TickPerf,
    ) {
        self.ticks.record(total);

        let mut slowest_phase = "none";
        let mut slowest_phase_duration = Duration::ZERO;
        for (phase, duration) in perf_tick.phase_records() {
            self.phases.entry(phase).or_default().record(duration);
            if !matches!(phase, "game_tick" | "snapshot_fanout" | "outcome_checks")
                && duration > slowest_phase_duration
            {
                slowest_phase = phase;
                slowest_phase_duration = duration;
            }
        }

        for snapshot in perf_tick.snapshot_records() {
            self.snapshot_build.record(snapshot.snapshot);
            self.snapshot_compact.record(snapshot.compact);
            self.snapshot_total.record(snapshot.total);
            self.snapshot_entities.record(snapshot.entities);
            self.snapshot_resource_deltas
                .record(snapshot.resource_deltas);
            self.snapshot_events.record(snapshot.events);
        }

        self.record_worst_tick(WorstTick {
            tick,
            total,
            slowest_phase,
            slowest_phase_duration,
            entities: counts.entities,
            units: counts.units,
            buildings: counts.buildings,
            resources: counts.resources,
        });
    }

    fn record_worst_tick(&mut self, tick: WorstTick) {
        self.worst_ticks.push(tick);
        self.worst_ticks
            .sort_unstable_by(|a, b| b.total.cmp(&a.total).then_with(|| a.tick.cmp(&b.tick)));
        self.worst_ticks.truncate(TOP_WORST_TICKS);
    }
}

impl MetricSeries {
    fn record(&mut self, duration: Duration) {
        self.samples.push(duration);
    }

    fn len(&self) -> usize {
        self.samples.len()
    }

    fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    fn total(&self) -> Duration {
        self.samples.iter().copied().sum()
    }

    fn max(&self) -> Duration {
        self.samples.iter().copied().max().unwrap_or(Duration::ZERO)
    }

    fn avg(&self) -> Duration {
        if self.samples.is_empty() {
            Duration::ZERO
        } else {
            duration_div(self.total(), self.samples.len() as u32)
        }
    }

    fn percentile(&self, numerator: usize, denominator: usize) -> Duration {
        if self.samples.is_empty() || denominator == 0 {
            return Duration::ZERO;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        let rank = (sorted.len() * numerator).div_ceil(denominator);
        let index = rank.saturating_sub(1).min(sorted.len() - 1);
        sorted[index]
    }

    fn count_at_least(&self, threshold: Duration) -> usize {
        self.samples
            .iter()
            .filter(|&&duration| duration >= threshold)
            .count()
    }
}

impl CountSeries {
    fn record(&mut self, count: usize) {
        self.samples.push(count);
    }

    fn len(&self) -> usize {
        self.samples.len()
    }

    fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    fn total(&self) -> u64 {
        self.samples.iter().map(|&value| value as u64).sum()
    }

    fn max(&self) -> usize {
        self.samples.iter().copied().max().unwrap_or(0)
    }

    fn avg(&self) -> usize {
        if self.samples.is_empty() {
            0
        } else {
            self.samples.iter().sum::<usize>() / self.samples.len()
        }
    }

    fn percentile(&self, numerator: usize, denominator: usize) -> usize {
        if self.samples.is_empty() || denominator == 0 {
            return 0;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        let rank = (sorted.len() * numerator).div_ceil(denominator);
        let index = rank.saturating_sub(1).min(sorted.len() - 1);
        sorted[index]
    }

    fn count_over(&self, threshold: usize) -> usize {
        self.samples
            .iter()
            .filter(|&&count| count > threshold)
            .count()
    }
}

pub fn run_from_env() {
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
    let mut ai_controllers = live_ai_controllers(&players, config.seed);
    let started = Instant::now();
    let mut snapshot_bytes = 0u64;
    let mut serialized_snapshots = 0u64;
    let mut attack_events = 0usize;
    let mut death_events = 0usize;
    let mut perf_report = PerfReport::default();

    while game.tick_count() < config.ticks {
        let alive = game.alive_players();
        if alive.len() <= 1 {
            break;
        }

        let tick_start = Instant::now();
        let mut perf_tick = perf::TickPerf::maybe_new();
        let game_tick_start = Instant::now();
        let tick_events = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            enqueue_live_ai_commands(&mut game, &mut ai_controllers);
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
                    _ => {}
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
            let codec = default_snapshot_codec();
            let payload = encode_snapshot_frame(&snapshot, codec)
                .map_err(|err| format!("failed to serialize snapshot: {err}"))?;
            let serialize_duration = serialize_start.elapsed();
            let payload_len = snapshot_frame_len(&payload);
            snapshot_bytes = snapshot_bytes.saturating_add(payload_len as u64);
            serialized_snapshots = serialized_snapshots.saturating_add(1);
            perf_report.record_snapshot_serialize(serialize_duration);
            perf_report.record_snapshot_payload_bytes(payload_len);
            perf::log_writer_message(perf::WriterMessageTiming {
                player_id: player.id,
                message_kind: "snapshot",
                snapshot_codec: codec.name(),
                snapshot_codec_version: codec.version(),
                frame_kind: payload.frame_kind(),
                serialize: serialize_duration,
                send: Duration::ZERO,
                bytes: payload_len,
            });

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
        let total = tick_start.elapsed();
        let counts = game.perf_entity_counts();
        if let Some(perf_tick) = perf_tick.as_mut() {
            perf_tick.record_phase("outcome_checks", outcome_start.elapsed());
            perf_report.record_tick(game.current_tick(), total, counts, perf_tick);
            perf_tick.finish(perf::TickContext {
                room: ROOM_NAME,
                match_run_id: "",
                tick: game.current_tick(),
                scheduler_lag: Duration::ZERO,
                total,
                players: players.len(),
                spectators: 0,
                ai_players: players.len(),
                counts,
            });
        } else {
            perf_report.record_total_only_tick(game.current_tick(), total, counts);
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
        perf_report,
    })
}

fn snapshot_frame_len(frame: &SnapshotFrame) -> usize {
    match frame {
        SnapshotFrame::Text(text) => text.len(),
        SnapshotFrame::Binary(bytes) => bytes.len(),
    }
}

fn live_ai_controllers(players: &[PlayerInit], seed: u32) -> Vec<AiController> {
    let mut rng = SmallRng::seed_from_u64((seed as u64) ^ 0xA17E_5EED);
    players
        .iter()
        .filter(|player| player.is_ai)
        .map(|player| {
            let profile_id = rts_ai::random_live_profile_id(&mut rng);
            let profile_id = rts_ai::resolve_live_profile_id_for_match(profile_id);
            AiController::with_profile_id(player.id, profile_id)
        })
        .collect()
}

fn enqueue_live_ai_commands(game: &mut Game, controllers: &mut [AiController]) {
    let start = game.start_payload();
    let alive = game.alive_players();
    let mut commands = Vec::new();
    for controller in controllers {
        let player_id = controller.player_id();
        if !alive.contains(&player_id) {
            continue;
        }
        let snapshot = game.snapshot_for(player_id);
        commands.extend(
            controller
                .think(AiThinkContext {
                    start: &start,
                    snapshot: &snapshot,
                    alive_player_ids: &alive,
                    retreat_commands: game.worker_retreat_commands_for(player_id),
                })
                .into_iter()
                .map(|command| (player_id, command)),
        );
    }
    for (player_id, command) in commands {
        game.enqueue(player_id, command);
    }
}

fn four_ai_players() -> Vec<PlayerInit> {
    vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Perf AI 1".to_string(),
            color: "#4878c8".to_string(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Perf AI 2".to_string(),
            color: "#c84848".to_string(),
            is_ai: true,
        },
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: "kriegsia".to_string(),
            name: "Perf AI 3".to_string(),
            color: "#30a090".to_string(),
            is_ai: true,
        },
        PlayerInit {
            id: 4,
            team_id: 4,
            faction_id: "kriegsia".to_string(),
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
    print_snapshot_payload_bytes(&summary.perf_report.snapshot_payload_bytes);
    println!(
        "events: attacks={} deaths={}  final_entities={} units={} buildings={} resources={}",
        summary.attack_events,
        summary.death_events,
        summary.final_counts.entities,
        summary.final_counts.units,
        summary.final_counts.buildings,
        summary.final_counts.resources
    );
    print_perf_report(&summary.perf_report);
}

fn print_snapshot_payload_bytes(bytes: &CountSeries) {
    if bytes.is_empty() {
        return;
    }
    let budget = SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES as usize;
    let over_budget = bytes.count_over(budget);
    println!(
        "snapshot_payload_bytes: samples={} total={} avg={} p95={} max={} segment_budget={} over_segment_budget={} over_segment_budget_pct_x100={}",
        bytes.len(),
        bytes.total(),
        bytes.avg(),
        bytes.percentile(95, 100),
        bytes.max(),
        budget,
        over_budget,
        pct_x100(over_budget, bytes.len())
    );
}

fn print_perf_report(report: &PerfReport) {
    if report.ticks.is_empty() {
        return;
    }

    let config = perf::PerfConfig::global();
    println!();
    println!("Perf aggregate report");
    println!(
        "tick_total: samples={} total_ms={} avg_us={} p95_us={} p99_us={} max_us={} slow_ticks>={}ms={}",
        report.ticks.len(),
        report.ticks.total().as_millis(),
        report.ticks.avg().as_micros(),
        report.ticks.percentile(95, 100).as_micros(),
        report.ticks.percentile(99, 100).as_micros(),
        report.ticks.max().as_micros(),
        config.slow_tick().as_millis(),
        report.ticks.count_at_least(config.slow_tick())
    );

    print_phase_table(
        "top-level phases by total",
        phase_rows(report, |phase| {
            matches!(phase, "game_tick" | "snapshot_fanout" | "outcome_checks")
        }),
        report.ticks.total(),
        config.slow_phase(),
    );
    print_phase_table(
        "simulation phases by total",
        phase_rows(report, |phase| {
            !matches!(phase, "game_tick" | "snapshot_fanout" | "outcome_checks")
        }),
        report.ticks.total(),
        config.slow_phase(),
    );
    print_snapshot_report(report, config.slow_snapshot());
    print_worst_ticks(report);
}

fn phase_rows(
    report: &PerfReport,
    keep: impl Fn(&'static str) -> bool,
) -> Vec<(&'static str, &MetricSeries)> {
    let mut rows: Vec<_> = report
        .phases
        .iter()
        .filter(|(phase, _)| keep(phase))
        .map(|(&phase, metrics)| (phase, metrics))
        .collect();
    rows.sort_unstable_by(|a, b| {
        b.1.total()
            .cmp(&a.1.total())
            .then_with(|| b.1.max().cmp(&a.1.max()))
            .then_with(|| a.0.cmp(b.0))
    });
    rows
}

fn print_phase_table(
    title: &str,
    rows: Vec<(&'static str, &MetricSeries)>,
    tick_total: Duration,
    spike_threshold: Duration,
) {
    if rows.is_empty() {
        return;
    }
    println!();
    println!("{title}");
    println!(
        "{:<24} {:>8} {:>10} {:>7} {:>9} {:>9} {:>9} {:>9} {:>9}",
        "phase", "samples", "total_ms", "share", "avg_us", "p95_us", "p99_us", "max_us", "spikes"
    );
    for (phase, metrics) in rows {
        println!(
            "{:<24} {:>8} {:>10} {:>6.1}% {:>9} {:>9} {:>9} {:>9} {:>9}",
            phase,
            metrics.len(),
            metrics.total().as_millis(),
            percent_of(metrics.total(), tick_total),
            metrics.avg().as_micros(),
            metrics.percentile(95, 100).as_micros(),
            metrics.percentile(99, 100).as_micros(),
            metrics.max().as_micros(),
            metrics.count_at_least(spike_threshold)
        );
    }
}

fn print_snapshot_report(report: &PerfReport, spike_threshold: Duration) {
    if report.snapshot_total.is_empty() && report.snapshot_serialize.is_empty() {
        return;
    }

    println!();
    println!("snapshot detail");
    println!(
        "{:<24} {:>8} {:>10} {:>9} {:>9} {:>9} {:>9} {:>9}",
        "part", "samples", "total_ms", "avg_us", "p95_us", "p99_us", "max_us", "spikes"
    );
    for (name, metrics) in [
        ("snapshot_build", &report.snapshot_build),
        ("snapshot_compact", &report.snapshot_compact),
        ("snapshot_build_compact", &report.snapshot_total),
        ("snapshot_serialize", &report.snapshot_serialize),
    ] {
        if metrics.is_empty() {
            continue;
        }
        println!(
            "{:<24} {:>8} {:>10} {:>9} {:>9} {:>9} {:>9} {:>9}",
            name,
            metrics.len(),
            metrics.total().as_millis(),
            metrics.avg().as_micros(),
            metrics.percentile(95, 100).as_micros(),
            metrics.percentile(99, 100).as_micros(),
            metrics.max().as_micros(),
            metrics.count_at_least(spike_threshold)
        );
    }
    println!(
        "snapshot_payload_shape: avg_entities={} max_entities={} avg_resource_deltas={} max_resource_deltas={} avg_events={} max_events={}",
        report.snapshot_entities.avg(),
        report.snapshot_entities.max(),
        report.snapshot_resource_deltas.avg(),
        report.snapshot_resource_deltas.max(),
        report.snapshot_events.avg(),
        report.snapshot_events.max()
    );
}

fn print_worst_ticks(report: &PerfReport) {
    if report.worst_ticks.is_empty() {
        return;
    }

    println!();
    println!("worst ticks");
    println!(
        "{:>8} {:>10} {:<24} {:>12} {:>9} {:>7} {:>10} {:>10}",
        "tick",
        "total_us",
        "slowest_phase",
        "phase_us",
        "entities",
        "units",
        "buildings",
        "resources"
    );
    for tick in &report.worst_ticks {
        println!(
            "{:>8} {:>10} {:<24} {:>12} {:>9} {:>7} {:>10} {:>10}",
            tick.tick,
            tick.total.as_micros(),
            tick.slowest_phase,
            tick.slowest_phase_duration.as_micros(),
            tick.entities,
            tick.units,
            tick.buildings,
            tick.resources
        );
    }
}

fn percent_of(part: Duration, whole: Duration) -> f64 {
    if whole.is_zero() {
        0.0
    } else {
        part.as_secs_f64() * 100.0 / whole.as_secs_f64()
    }
}

fn pct_x100(part: usize, whole: usize) -> usize {
    (part * 10_000 + (whole / 2))
        .checked_div(whole)
        .unwrap_or(0)
}

fn duration_div(duration: Duration, divisor: u32) -> Duration {
    if divisor == 0 {
        Duration::ZERO
    } else {
        duration / divisor
    }
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
