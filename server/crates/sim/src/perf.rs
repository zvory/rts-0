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
pub struct PerfConfig {
    mode: PerfMode,
    slow_tick: Duration,
    slow_phase: Duration,
    slow_snapshot: Duration,
    slow_send: Duration,
    sample_every: u32,
    log_snapshots: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EntityCounts {
    pub entities: usize,
    pub units: usize,
    pub buildings: usize,
    pub resources: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PathingRequestSource {
    Move,
    AttackMove,
    DirectAttack,
    Gather,
    Build,
    Deconstruct,
    Ability,
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PathingSourceCounts {
    pub(crate) move_orders: u32,
    pub(crate) attack_move: u32,
    pub(crate) direct_attack: u32,
    pub(crate) gather: u32,
    pub(crate) build: u32,
    pub(crate) deconstruct: u32,
    pub(crate) ability: u32,
    pub(crate) other: u32,
}

impl PathingSourceCounts {
    pub(crate) fn record(&mut self, source: PathingRequestSource, count: u32) {
        match source {
            PathingRequestSource::Move => self.move_orders = self.move_orders.saturating_add(count),
            PathingRequestSource::AttackMove => {
                self.attack_move = self.attack_move.saturating_add(count);
            }
            PathingRequestSource::DirectAttack => {
                self.direct_attack = self.direct_attack.saturating_add(count);
            }
            PathingRequestSource::Gather => self.gather = self.gather.saturating_add(count),
            PathingRequestSource::Build => self.build = self.build.saturating_add(count),
            PathingRequestSource::Deconstruct => {
                self.deconstruct = self.deconstruct.saturating_add(count);
            }
            PathingRequestSource::Ability => self.ability = self.ability.saturating_add(count),
            PathingRequestSource::Other => self.other = self.other.saturating_add(count),
        }
    }

    fn add(&mut self, other: Self) {
        self.move_orders = self.move_orders.saturating_add(other.move_orders);
        self.attack_move = self.attack_move.saturating_add(other.attack_move);
        self.direct_attack = self.direct_attack.saturating_add(other.direct_attack);
        self.gather = self.gather.saturating_add(other.gather);
        self.build = self.build.saturating_add(other.build);
        self.deconstruct = self.deconstruct.saturating_add(other.deconstruct);
        self.ability = self.ability.saturating_add(other.ability);
        self.other = self.other.saturating_add(other.other);
    }

    fn top(self) -> (&'static str, u32) {
        let (label, count) = [
            ("move", self.move_orders),
            ("attackMove", self.attack_move),
            ("attack", self.direct_attack),
            ("gather", self.gather),
            ("build", self.build),
            ("deconstruct", self.deconstruct),
            ("ability", self.ability),
            ("other", self.other),
        ]
        .into_iter()
        .max_by_key(|(label, count)| (*count, std::cmp::Reverse(*label)))
        .unwrap_or(("none", 0));
        if count == 0 {
            ("none", 0)
        } else {
            (label, count)
        }
    }

    fn compact(self) -> String {
        compact_counts([
            ("move", self.move_orders),
            ("attackMove", self.attack_move),
            ("attack", self.direct_attack),
            ("gather", self.gather),
            ("build", self.build),
            ("deconstruct", self.deconstruct),
            ("ability", self.ability),
            ("other", self.other),
        ])
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct UnitCountBuckets {
    pub(crate) one: u32,
    pub(crate) two_to_four: u32,
    pub(crate) five_to_sixteen: u32,
    pub(crate) seventeen_to_sixty_four: u32,
    pub(crate) over_sixty_four: u32,
}

impl UnitCountBuckets {
    pub(crate) fn record(&mut self, count: usize) {
        match count {
            0 => {}
            1 => self.one = self.one.saturating_add(1),
            2..=4 => self.two_to_four = self.two_to_four.saturating_add(1),
            5..=16 => self.five_to_sixteen = self.five_to_sixteen.saturating_add(1),
            17..=64 => {
                self.seventeen_to_sixty_four = self.seventeen_to_sixty_four.saturating_add(1);
            }
            _ => self.over_sixty_four = self.over_sixty_four.saturating_add(1),
        }
    }

    fn compact(self) -> String {
        compact_counts([
            ("1", self.one),
            ("2-4", self.two_to_four),
            ("5-16", self.five_to_sixteen),
            ("17-64", self.seventeen_to_sixty_four),
            ("65+", self.over_sixty_four),
        ])
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PathLengthBuckets {
    pub(crate) zero: u32,
    pub(crate) one_to_eight: u32,
    pub(crate) nine_to_thirty_two: u32,
    pub(crate) thirty_three_to_128: u32,
    pub(crate) over_128: u32,
}

impl PathLengthBuckets {
    pub(crate) fn record(&mut self, count: usize) {
        match count {
            0 => self.zero = self.zero.saturating_add(1),
            1..=8 => self.one_to_eight = self.one_to_eight.saturating_add(1),
            9..=32 => self.nine_to_thirty_two = self.nine_to_thirty_two.saturating_add(1),
            33..=128 => self.thirty_three_to_128 = self.thirty_three_to_128.saturating_add(1),
            _ => self.over_128 = self.over_128.saturating_add(1),
        }
    }

    fn compact(self) -> String {
        compact_counts([
            ("0", self.zero),
            ("1-8", self.one_to_eight),
            ("9-32", self.nine_to_thirty_two),
            ("33-128", self.thirty_three_to_128),
            ("129+", self.over_128),
        ])
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExploredNodeBuckets {
    pub(crate) zero: u32,
    pub(crate) one_to_512: u32,
    pub(crate) five_thirteen_to_2048: u32,
    pub(crate) two_049_to_8192: u32,
    pub(crate) over_8192: u32,
}

impl ExploredNodeBuckets {
    pub(crate) fn record(&mut self, count: usize) {
        match count {
            0 => self.zero = self.zero.saturating_add(1),
            1..=512 => self.one_to_512 = self.one_to_512.saturating_add(1),
            513..=2048 => {
                self.five_thirteen_to_2048 = self.five_thirteen_to_2048.saturating_add(1);
            }
            2049..=8192 => self.two_049_to_8192 = self.two_049_to_8192.saturating_add(1),
            _ => self.over_8192 = self.over_8192.saturating_add(1),
        }
    }

    fn compact(self) -> String {
        compact_counts([
            ("0", self.zero),
            ("1-512", self.one_to_512),
            ("513-2048", self.five_thirteen_to_2048),
            ("2049-8192", self.two_049_to_8192),
            ("8193+", self.over_8192),
        ])
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PathingPassDiagnostics {
    pub(crate) pass: &'static str,
    pub(crate) awaiting_start: usize,
    pub(crate) queued_for_path: usize,
    pub(crate) requests_processed: usize,
    pub(crate) requests_deferred: usize,
    pub(crate) still_awaiting: usize,
    pub(crate) path_success: usize,
    pub(crate) path_failed: usize,
    pub(crate) same_tile: usize,
    pub(crate) cache_hits: usize,
    pub(crate) cache_misses: usize,
    pub(crate) path_budget_exhausted: usize,
    pub(crate) coordinator_budget_exhausted: bool,
    pub(crate) total_request_duration: Duration,
    pub(crate) worst_request: Duration,
    pub(crate) explored_nodes_max: usize,
    pub(crate) path_len_max: usize,
    pub(crate) source_counts: PathingSourceCounts,
    pub(crate) queued_source_counts: PathingSourceCounts,
    pub(crate) group_size_buckets: UnitCountBuckets,
    pub(crate) path_len_buckets: PathLengthBuckets,
    pub(crate) explored_node_buckets: ExploredNodeBuckets,
}

pub(crate) struct PathingRequestSample {
    pub(crate) source: PathingRequestSource,
    pub(crate) path_ok: bool,
    pub(crate) same_tile: bool,
    pub(crate) cache_hit: Option<bool>,
    pub(crate) budget_exhausted: bool,
    pub(crate) expanded_nodes: usize,
    pub(crate) tile_path_len: usize,
    pub(crate) duration: Duration,
}

impl PathingPassDiagnostics {
    pub(crate) fn new(pass: &'static str, awaiting_start: usize) -> Self {
        PathingPassDiagnostics {
            pass,
            awaiting_start,
            queued_for_path: 0,
            requests_processed: 0,
            requests_deferred: 0,
            still_awaiting: 0,
            path_success: 0,
            path_failed: 0,
            same_tile: 0,
            cache_hits: 0,
            cache_misses: 0,
            path_budget_exhausted: 0,
            coordinator_budget_exhausted: false,
            total_request_duration: Duration::ZERO,
            worst_request: Duration::ZERO,
            explored_nodes_max: 0,
            path_len_max: 0,
            source_counts: PathingSourceCounts::default(),
            queued_source_counts: PathingSourceCounts::default(),
            group_size_buckets: UnitCountBuckets::default(),
            path_len_buckets: PathLengthBuckets::default(),
            explored_node_buckets: ExploredNodeBuckets::default(),
        }
    }

    pub(crate) fn record_group_queued_for_path(
        &mut self,
        source: PathingRequestSource,
        count: usize,
    ) {
        self.queued_for_path = self.queued_for_path.saturating_add(count);
        self.queued_source_counts
            .record(source, count.min(u32::MAX as usize) as u32);
        self.group_size_buckets.record(count);
    }

    pub(crate) fn record_path_request(&mut self, request: PathingRequestSample) {
        self.requests_processed = self.requests_processed.saturating_add(1);
        self.source_counts.record(request.source, 1);
        self.total_request_duration = self.total_request_duration.saturating_add(request.duration);
        self.worst_request = self.worst_request.max(request.duration);
        if request.path_ok {
            self.path_success = self.path_success.saturating_add(1);
        } else {
            self.path_failed = self.path_failed.saturating_add(1);
        }
        if request.same_tile {
            self.same_tile = self.same_tile.saturating_add(1);
        }
        match request.cache_hit {
            Some(true) => self.cache_hits = self.cache_hits.saturating_add(1),
            Some(false) => self.cache_misses = self.cache_misses.saturating_add(1),
            None => {}
        }
        if request.budget_exhausted {
            self.path_budget_exhausted = self.path_budget_exhausted.saturating_add(1);
        }
        self.explored_nodes_max = self.explored_nodes_max.max(request.expanded_nodes);
        self.path_len_max = self.path_len_max.max(request.tile_path_len);
        self.path_len_buckets.record(request.tile_path_len);
        self.explored_node_buckets.record(request.expanded_nodes);
    }
}

fn compact_counts<const N: usize>(entries: [(&'static str, u32); N]) -> String {
    let mut out = String::new();
    for (label, count) in entries {
        if count == 0 {
            continue;
        }
        if !out.is_empty() {
            out.push(',');
        }
        out.push_str(label);
        out.push('=');
        out.push_str(&count.to_string());
    }
    if out.is_empty() {
        "none".to_string()
    } else {
        out
    }
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
pub struct TickPerf {
    phases: Vec<PhaseTiming>,
    snapshots: Vec<SnapshotTiming>,
    pathing: Vec<PathingPassDiagnostics>,
    snapshot_stored: u32,
    snapshot_replaced: u32,
    snapshot_closed: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct TickContext<'a> {
    pub room: &'a str,
    pub match_run_id: &'a str,
    pub tick: u32,
    pub scheduler_lag: Duration,
    pub total: Duration,
    pub players: usize,
    pub spectators: usize,
    pub ai_players: usize,
    pub counts: EntityCounts,
}

#[derive(Clone, Copy, Debug)]
pub struct SnapshotRecord {
    pub player_id: u32,
    pub spectator: bool,
    pub snapshot: Duration,
    pub compact: Duration,
    pub entities: usize,
    pub resource_deltas: usize,
    pub events: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct SnapshotPerfSample {
    pub snapshot: Duration,
    pub compact: Duration,
    pub total: Duration,
    pub entities: usize,
    pub resource_deltas: usize,
    pub events: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum SnapshotEnqueue {
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

    pub fn global() -> &'static Self {
        static CONFIG: OnceLock<PerfConfig> = OnceLock::new();
        CONFIG.get_or_init(PerfConfig::from_env)
    }

    pub fn enabled(&self) -> bool {
        self.mode != PerfMode::Off
    }

    #[allow(dead_code)]
    pub fn slow_tick(&self) -> Duration {
        self.slow_tick
    }

    #[allow(dead_code)]
    pub fn slow_phase(&self) -> Duration {
        self.slow_phase
    }

    #[allow(dead_code)]
    pub fn slow_snapshot(&self) -> Duration {
        self.slow_snapshot
    }

    pub fn enforce_release_build_for_server(&self) {
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
    pub fn maybe_new() -> Option<Self> {
        if PerfConfig::global().enabled() {
            Some(TickPerf {
                phases: Vec::with_capacity(16),
                snapshots: Vec::with_capacity(4),
                pathing: Vec::with_capacity(3),
                snapshot_stored: 0,
                snapshot_replaced: 0,
                snapshot_closed: 0,
            })
        } else {
            None
        }
    }

    pub fn record_phase(&mut self, phase: &'static str, duration: Duration) {
        self.phases.push(PhaseTiming { phase, duration });
    }

    pub fn record_snapshot(&mut self, record: SnapshotRecord) {
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

    pub(crate) fn record_pathing(&mut self, record: PathingPassDiagnostics) {
        self.pathing.push(record);
    }

    pub fn record_enqueue(&mut self, status: SnapshotEnqueue) {
        match status {
            SnapshotEnqueue::Stored => self.snapshot_stored += 1,
            SnapshotEnqueue::Replaced => self.snapshot_replaced += 1,
            SnapshotEnqueue::Closed => self.snapshot_closed += 1,
        }
    }

    #[allow(dead_code)]
    pub fn phase_records(&self) -> impl Iterator<Item = (&'static str, Duration)> + '_ {
        self.phases
            .iter()
            .map(|phase| (phase.phase, phase.duration))
    }

    #[allow(dead_code)]
    pub fn snapshot_records(&self) -> impl Iterator<Item = SnapshotPerfSample> + '_ {
        self.snapshots.iter().map(|snapshot| SnapshotPerfSample {
            snapshot: snapshot.snapshot,
            compact: snapshot.compact,
            total: snapshot.snapshot + snapshot.compact,
            entities: snapshot.entities,
            resource_deltas: snapshot.resource_deltas,
            events: snapshot.events,
        })
    }

    pub fn finish(&self, context: TickContext<'_>) {
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
        let pathing = PathingTickSummary::from_records(&self.pathing);

        info!(
            target: PERF_TARGET,
            event = "tick",
            room = %context.room,
            match_run_id = %context.match_run_id,
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
            pathing_passes = self.pathing.len(),
            pathing_awaiting_start = pathing.awaiting_start,
            pathing_promoted_awaiting_start = pathing.promoted_awaiting_start,
            pathing_promote_queued_for_path = pathing.promote_queued_for_path,
            pathing_requests = pathing.requests,
            pathing_processed = pathing.processed,
            pathing_deferred = pathing.deferred,
            pathing_still_awaiting = pathing.still_awaiting,
            pathing_success = pathing.success,
            pathing_failed = pathing.failed,
            pathing_cache_hits = pathing.cache_hits,
            pathing_cache_misses = pathing.cache_misses,
            pathing_budget_exhausted = pathing.budget_exhausted,
            pathing_worst_request_ms = millis(pathing.worst_request),
            pathing_explored_nodes_max = pathing.explored_nodes_max,
            pathing_path_len_max = pathing.path_len_max,
            pathing_top_source = pathing.top_source,
            pathing_top_source_count = pathing.top_source_count,
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

        for record in &self.pathing {
            debug!(
                target: PERF_TARGET,
                event = "pathing",
                room = %context.room,
                match_run_id = %context.match_run_id,
                tick = context.tick,
                pass = record.pass,
                awaiting_start = record.awaiting_start,
                queued_for_path = record.queued_for_path,
                requests_processed = record.requests_processed,
                requests_deferred = record.requests_deferred,
                still_awaiting = record.still_awaiting,
                path_success = record.path_success,
                path_failed = record.path_failed,
                same_tile = record.same_tile,
                cache_hits = record.cache_hits,
                cache_misses = record.cache_misses,
                path_budget_exhausted = record.path_budget_exhausted,
                coordinator_budget_exhausted = record.coordinator_budget_exhausted,
                total_request_ms = millis(record.total_request_duration),
                worst_request_ms = millis(record.worst_request),
                worst_request_bucket = request_duration_bucket(record.worst_request),
                explored_nodes_max = record.explored_nodes_max,
                path_len_max = record.path_len_max,
                source_counts = %record.source_counts.compact(),
                queued_source_counts = %record.queued_source_counts.compact(),
                group_size_buckets = %record.group_size_buckets.compact(),
                path_len_buckets = %record.path_len_buckets.compact(),
                explored_node_buckets = %record.explored_node_buckets.compact(),
                cache_available = true,
                complexity_available = true,
                fuse_triggered = false,
                "performance pathing diagnostics"
            );
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

struct PathingTickSummary {
    awaiting_start: usize,
    promoted_awaiting_start: usize,
    promote_queued_for_path: usize,
    requests: usize,
    processed: usize,
    deferred: usize,
    still_awaiting: usize,
    success: usize,
    failed: usize,
    cache_hits: usize,
    cache_misses: usize,
    budget_exhausted: usize,
    worst_request: Duration,
    explored_nodes_max: usize,
    path_len_max: usize,
    top_source: &'static str,
    top_source_count: u32,
}

impl PathingTickSummary {
    fn from_records(records: &[PathingPassDiagnostics]) -> Self {
        let mut request_sources = PathingSourceCounts::default();
        let mut queued_sources = PathingSourceCounts::default();
        let mut out = PathingTickSummary {
            awaiting_start: 0,
            promoted_awaiting_start: 0,
            promote_queued_for_path: 0,
            requests: 0,
            processed: 0,
            deferred: 0,
            still_awaiting: 0,
            success: 0,
            failed: 0,
            cache_hits: 0,
            cache_misses: 0,
            budget_exhausted: 0,
            worst_request: Duration::ZERO,
            explored_nodes_max: 0,
            path_len_max: 0,
            top_source: "none",
            top_source_count: 0,
        };
        for record in records {
            match record.pass {
                "awaiting_paths" => out.awaiting_start = record.awaiting_start,
                "promoted_awaiting_paths" => {
                    out.promoted_awaiting_start = record.awaiting_start;
                }
                "promote_queued_orders" => {
                    out.promote_queued_for_path = record.queued_for_path;
                }
                _ => {}
            }
            out.requests = out.requests.saturating_add(record.requests_processed);
            out.processed = out.processed.saturating_add(record.requests_processed);
            if matches!(record.pass, "awaiting_paths" | "promoted_awaiting_paths") {
                out.deferred = record.requests_deferred;
            }
            out.still_awaiting = record.still_awaiting;
            out.success = out.success.saturating_add(record.path_success);
            out.failed = out.failed.saturating_add(record.path_failed);
            out.cache_hits = out.cache_hits.saturating_add(record.cache_hits);
            out.cache_misses = out.cache_misses.saturating_add(record.cache_misses);
            out.budget_exhausted = out
                .budget_exhausted
                .saturating_add(record.path_budget_exhausted)
                .saturating_add(usize::from(record.coordinator_budget_exhausted));
            out.worst_request = out.worst_request.max(record.worst_request);
            out.explored_nodes_max = out.explored_nodes_max.max(record.explored_nodes_max);
            out.path_len_max = out.path_len_max.max(record.path_len_max);
            request_sources.add(record.source_counts);
            queued_sources.add(record.queued_source_counts);
        }
        let (request_label, request_count) = request_sources.top();
        let (label, count) = if request_count == 0 {
            queued_sources.top()
        } else {
            (request_label, request_count)
        };
        out.top_source = label;
        out.top_source_count = count;
        out
    }
}

pub struct WriterMessageTiming {
    pub player_id: u32,
    pub message_kind: &'static str,
    pub snapshot_codec: &'static str,
    pub snapshot_codec_version: u16,
    pub frame_kind: &'static str,
    pub serialize: Duration,
    pub send: Duration,
    pub bytes: usize,
}

pub fn log_writer_message(record: WriterMessageTiming) {
    let config = PerfConfig::global();
    if !config.should_log_writer(record.serialize, record.send) {
        return;
    }
    debug!(
        target: PERF_TARGET,
        event = "writer_send",
        player_id = record.player_id,
        message_kind = record.message_kind,
        snapshot_codec = record.snapshot_codec,
        snapshot_codec_version = record.snapshot_codec_version,
        frame_kind = record.frame_kind,
        serialize_ms = millis(record.serialize),
        send_ms = millis(record.send),
        bytes = record.bytes,
        "performance writer timing"
    );
}

pub fn timed<T>(perf: Option<&mut TickPerf>, phase: &'static str, f: impl FnOnce() -> T) -> T {
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

fn request_duration_bucket(duration: Duration) -> &'static str {
    match duration.as_millis() {
        0 => "0ms",
        1..=2 => "1-2ms",
        3..=8 => "3-8ms",
        9..=16 => "9-16ms",
        17..=33 => "17-33ms",
        _ => "34ms+",
    }
}

fn millis(duration: Duration) -> u128 {
    duration.as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pathing_top_source_reports_none_when_all_sources_are_zero() {
        assert_eq!(PathingSourceCounts::default().top(), ("none", 0));

        let summary = PathingTickSummary::from_records(&[]);
        assert_eq!(summary.top_source, "none");
        assert_eq!(summary.top_source_count, 0);
    }

    #[test]
    fn pathing_tick_summary_uses_final_awaiting_deferred_count() {
        let mut initial = PathingPassDiagnostics::new("awaiting_paths", 96);
        initial.requests_processed = 64;
        initial.requests_deferred = 32;
        initial.still_awaiting = 32;
        initial.source_counts.record(PathingRequestSource::Move, 64);

        let mut promotion = PathingPassDiagnostics::new("promote_queued_orders", 32);
        promotion.queued_for_path = 40;
        promotion.requests_deferred = 0;
        promotion.still_awaiting = 72;
        promotion
            .source_counts
            .record(PathingRequestSource::AttackMove, 40);

        let mut promoted = PathingPassDiagnostics::new("promoted_awaiting_paths", 72);
        promoted.requests_processed = 40;
        promoted.requests_deferred = 32;
        promoted.still_awaiting = 32;
        promoted
            .source_counts
            .record(PathingRequestSource::AttackMove, 40);

        let summary = PathingTickSummary::from_records(&[initial, promotion, promoted]);

        assert_eq!(summary.processed, 104);
        assert_eq!(summary.deferred, 32);
        assert_eq!(summary.still_awaiting, 32);
        assert_eq!(summary.promote_queued_for_path, 40);
    }
}
