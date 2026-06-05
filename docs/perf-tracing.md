# Server performance tracing

Optional server-side performance tracing is controlled by environment variables. It is off by
default and emits structured `tracing` logs under the `server::perf` target when enabled.

## Modes

```bash
RTS_PERF=off      # default
RTS_PERF=spikes   # log ticks slower than RTS_PERF_SLOW_TICK_MS
RTS_PERF=sample   # log every RTS_PERF_SAMPLE_EVERY ticks and all slow ticks
RTS_PERF=full     # log every tick, phase, snapshot, and writer send
```

Useful thresholds:

```bash
RTS_PERF_SLOW_TICK_MS=33       # default is the configured tick interval
RTS_PERF_SLOW_PHASE_MS=8       # phase detail threshold on logged ticks
RTS_PERF_SLOW_SNAPSHOT_MS=8    # per-recipient snapshot detail threshold
RTS_PERF_SLOW_SEND_MS=10       # writer serialization/send threshold
RTS_PERF_SAMPLE_EVERY=300      # sample mode interval
RTS_PERF_LOG_SNAPSHOTS=1       # include every snapshot detail on logged ticks
```

Example local run:

```bash
cd server && RUST_LOG=info,server::perf=debug RTS_PERF=spikes cargo run --release
```

Perf tracing intentionally exits at startup if `RTS_PERF` is enabled in a debug build. Debug builds
include overflow checks and different optimization behavior, so their timings are not useful for
production lag diagnosis.

Tick summaries separate simulation, snapshot fanout, scheduler lag, room shape, entity counts,
snapshot coalescing, and the slowest concrete simulation phase. Snapshot and writer lines identify
whether lag is in per-player projection/compaction, JSON serialization, or socket writes.
