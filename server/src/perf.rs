use std::env;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tracing::{debug, info};

use crate::config;

const PERF_TARGET: &str = "server::perf";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PerfMode {
    Off,
    Spikes,
    Sample,
    Full,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PerfConfig {
    mode: PerfMode,
    slow_tick: Duration,
    slow_phase: Duration,
    slow_snapshot: Duration,
    slow_send: Duration,
    sample_every: u32,
    log_snapshots: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EntityCounts {
    pub(crate) entities: usize,
    pub(crate) units: usize,
    pub(crate) buildings: usize,
    pub(crate) resources: usize,
}

#[derive(Clone, Copy, Debug)]
struct PhaseTiming {
    phase: &'static str,
    duration: Duration,
}

#[derive(Clone, Copy, Debug)]
struct SnapshotTiming {
    player_id: u32,
    spectator: bool,
    snapshot: Duration,
    compact: Duration,
    entities: usize,
    resource_deltas: usize,
    events: usize,
}

#[derive(Debug)]
pub(crate) struct TickPerf {
    phases: Vec<PhaseTiming>,
    snapshots: Vec<SnapshotTiming>,
    snapshot_stored: u32,
    snapshot_replaced: u32,
    snapshot_closed: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TickContext<'a> {
    pub(crate) room: &'a str,
    pub(crate) tick: u32,
    pub(crate) scheduler_lag: Duration,
    pub(crate) total: Duration,
    pub(crate) players: usize,
    pub(crate) spectators: usize,
    pub(crate) ai_players: usize,
    pub(crate) counts: EntityCounts,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SnapshotRecord {
    pub(crate) player_id: u32,
    pub(crate) spectator: bool,
    pub(crate) snapshot: Duration,
    pub(crate) compact: Duration,
    pub(crate) entities: usize,
    pub(crate) resource_deltas: usize,
    pub(crate) events: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct SnapshotPerfSample {
    pub(crate) snapshot: Duration,
    pub(crate) compact: Duration,
    pub(crate) total: Duration,
    pub(crate) entities: usize,
    pub(crate) resource_deltas: usize,
    pub(crate) events: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum SnapshotEnqueue {
    Stored,
    Replaced,
    Closed,
}

impl PerfConfig {
    fn from_env() -> Self {
        let mode = match env::var("RTS_PERF")
            .unwrap_or_else(|_| "off".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "1" | "on" | "spike" | "spikes" => PerfMode::Spikes,
            "sample" | "samples" => PerfMode::Sample,
            "full" | "trace" => PerfMode::Full,
            _ => PerfMode::Off,
        };
        PerfConfig {
            mode,
            slow_tick: duration_env("RTS_PERF_SLOW_TICK_MS", config::TICK_MS),
            slow_phase: duration_env("RTS_PERF_SLOW_PHASE_MS", 8),
            slow_snapshot: duration_env("RTS_PERF_SLOW_SNAPSHOT_MS", 8),
            slow_send: duration_env("RTS_PERF_SLOW_SEND_MS", 10),
            sample_every: u32_env("RTS_PERF_SAMPLE_EVERY", 300).max(1),
            log_snapshots: bool_env("RTS_PERF_LOG_SNAPSHOTS"),
        }
    }

    pub(crate) fn global() -> &'static Self {
        static CONFIG: OnceLock<PerfConfig> = OnceLock::new();
        CONFIG.get_or_init(PerfConfig::from_env)
    }

    pub(crate) fn enabled(&self) -> bool {
        self.mode != PerfMode::Off
    }

    #[allow(dead_code)]
    pub(crate) fn slow_tick(&self) -> Duration {
        self.slow_tick
    }

    #[allow(dead_code)]
    pub(crate) fn slow_phase(&self) -> Duration {
        self.slow_phase
    }

    #[allow(dead_code)]
    pub(crate) fn slow_snapshot(&self) -> Duration {
        self.slow_snapshot
    }

    pub(crate) fn enforce_release_build_for_server(&self) {
        #[cfg(debug_assertions)]
        if self.enabled() {
            tracing::error!(
                target: PERF_TARGET,
                event = "invalid_perf_build",
                "RTS_PERF requires a release build; run the server with `cargo run --release` or an optimized production binary"
            );
            std::process::exit(2);
        }
    }

    fn should_log_tick(&self, tick: u32, total: Duration) -> bool {
        match self.mode {
            PerfMode::Off => false,
            PerfMode::Spikes => total >= self.slow_tick,
            PerfMode::Sample => tick.is_multiple_of(self.sample_every) || total >= self.slow_tick,
            PerfMode::Full => true,
        }
    }

    fn should_log_snapshot(
        &self,
        tick_loggable: bool,
        snapshot: Duration,
        compact: Duration,
    ) -> bool {
        self.mode == PerfMode::Full
            || self.log_snapshots
            || (tick_loggable && (snapshot >= self.slow_snapshot || compact >= self.slow_snapshot))
    }

    fn should_log_writer(&self, serialize: Duration, send: Duration) -> bool {
        match self.mode {
            PerfMode::Off => false,
            PerfMode::Full => true,
            PerfMode::Sample | PerfMode::Spikes => {
                serialize >= self.slow_send || send >= self.slow_send
            }
        }
    }
}

impl TickPerf {
    pub(crate) fn maybe_new() -> Option<Self> {
        if PerfConfig::global().enabled() {
            Some(TickPerf {
                phases: Vec::with_capacity(16),
                snapshots: Vec::with_capacity(4),
                snapshot_stored: 0,
                snapshot_replaced: 0,
                snapshot_closed: 0,
            })
        } else {
            None
        }
    }

    pub(crate) fn record_phase(&mut self, phase: &'static str, duration: Duration) {
        self.phases.push(PhaseTiming { phase, duration });
    }

    pub(crate) fn record_snapshot(&mut self, record: SnapshotRecord) {
        self.snapshots.push(SnapshotTiming {
            player_id: record.player_id,
            spectator: record.spectator,
            snapshot: record.snapshot,
            compact: record.compact,
            entities: record.entities,
            resource_deltas: record.resource_deltas,
            events: record.events,
        });
    }

    pub(crate) fn record_enqueue(&mut self, status: SnapshotEnqueue) {
        match status {
            SnapshotEnqueue::Stored => self.snapshot_stored += 1,
            SnapshotEnqueue::Replaced => self.snapshot_replaced += 1,
            SnapshotEnqueue::Closed => self.snapshot_closed += 1,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn phase_records(&self) -> impl Iterator<Item = (&'static str, Duration)> + '_ {
        self.phases
            .iter()
            .map(|phase| (phase.phase, phase.duration))
    }

    #[allow(dead_code)]
    pub(crate) fn snapshot_records(&self) -> impl Iterator<Item = SnapshotPerfSample> + '_ {
        self.snapshots.iter().map(|snapshot| SnapshotPerfSample {
            snapshot: snapshot.snapshot,
            compact: snapshot.compact,
            total: snapshot.snapshot + snapshot.compact,
            entities: snapshot.entities,
            resource_deltas: snapshot.resource_deltas,
            events: snapshot.events,
        })
    }

    pub(crate) fn finish(&self, context: TickContext<'_>) {
        let config = PerfConfig::global();
        if !config.enabled() {
            return;
        }
        let log_tick = config.should_log_tick(context.tick, context.total);
        if !log_tick {
            return;
        }

        let sim = self.phase_duration("game_tick");
        let fanout = self.phase_duration("snapshot_fanout");
        let outcome = self.phase_duration("outcome_checks");
        let (slowest_phase, slowest_phase_ms) = self
            .phases
            .iter()
            .filter(|p| !matches!(p.phase, "game_tick" | "snapshot_fanout" | "outcome_checks"))
            .max_by_key(|p| p.duration)
            .map(|p| (p.phase, millis(p.duration)))
            .unwrap_or(("none", 0));
        let max_snapshot = self.snapshots.iter().max_by_key(|s| s.snapshot + s.compact);

        info!(
            target: PERF_TARGET,
            event = "tick",
            room = %context.room,
            tick = context.tick,
            tick_ms = millis(context.total),
            scheduler_lag_ms = millis(context.scheduler_lag),
            sim_ms = millis(sim),
            fanout_ms = millis(fanout),
            outcome_ms = millis(outcome),
            players = context.players,
            spectators = context.spectators,
            ai_players = context.ai_players,
            entities = context.counts.entities,
            units = context.counts.units,
            buildings = context.counts.buildings,
            resources = context.counts.resources,
            snapshots = self.snapshots.len(),
            snapshot_stored = self.snapshot_stored,
            snapshot_replaced = self.snapshot_replaced,
            snapshot_closed = self.snapshot_closed,
            max_snapshot_ms = max_snapshot.map(|s| millis(s.snapshot + s.compact)).unwrap_or(0),
            max_snapshot_entities = max_snapshot.map(|s| s.entities).unwrap_or(0),
            slowest_phase,
            slowest_phase_ms,
            "performance tick summary"
        );

        for phase in &self.phases {
            if config.mode == PerfMode::Full || phase.duration >= config.slow_phase {
                debug!(
                    target: PERF_TARGET,
                    event = "tick_phase",
                    room = %context.room,
                    tick = context.tick,
                    phase = phase.phase,
                    ms = millis(phase.duration),
                    "performance phase timing"
                );
            }
        }

        for snapshot in &self.snapshots {
            if config.should_log_snapshot(log_tick, snapshot.snapshot, snapshot.compact) {
                debug!(
                    target: PERF_TARGET,
                    event = "snapshot",
                    room = %context.room,
                    tick = context.tick,
                    player_id = snapshot.player_id,
                    spectator = snapshot.spectator,
                    snapshot_ms = millis(snapshot.snapshot),
                    compact_ms = millis(snapshot.compact),
                    total_ms = millis(snapshot.snapshot + snapshot.compact),
                    entities = snapshot.entities,
                    resource_deltas = snapshot.resource_deltas,
                    events = snapshot.events,
                    "performance snapshot timing"
                );
            }
        }
    }

    fn phase_duration(&self, phase: &'static str) -> Duration {
        self.phases
            .iter()
            .filter(|p| p.phase == phase)
            .map(|p| p.duration)
            .sum()
    }
}

pub(crate) fn log_writer_message(
    player_id: u32,
    message_kind: &'static str,
    serialize: Duration,
    send: Duration,
    bytes: usize,
) {
    let config = PerfConfig::global();
    if !config.should_log_writer(serialize, send) {
        return;
    }
    debug!(
        target: PERF_TARGET,
        event = "writer_send",
        player_id,
        message_kind,
        serialize_ms = millis(serialize),
        send_ms = millis(send),
        bytes,
        "performance writer timing"
    );
}

pub(crate) fn timed<T>(
    perf: Option<&mut TickPerf>,
    phase: &'static str,
    f: impl FnOnce() -> T,
) -> T {
    match perf {
        Some(perf) => {
            let start = Instant::now();
            let out = f();
            perf.record_phase(phase, start.elapsed());
            out
        }
        None => f(),
    }
}

fn duration_env(name: &str, default_ms: u64) -> Duration {
    Duration::from_millis(u64_env(name, default_ms))
}

fn u64_env(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn u32_env(name: &str, default: u32) -> u32 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn bool_env(name: &str) -> bool {
    matches!(
        env::var(name)
            .unwrap_or_else(|_| "0".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn millis(duration: Duration) -> u128 {
    duration.as_millis()
}
