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

Four-AI local harness:

```bash
scripts/ai-perf-harness.sh --ticks 20000
```

The harness runs four AI players in one local match, enables sample perf tracing by default, and
exercises simulation, per-player snapshot fanout, snapshot compaction, and compact JSON
serialization without requiring browser clients. Override the usual `RTS_PERF*` and `RUST_LOG`
environment variables when you need a different trace shape, or pass `--perf full` for every tick.

The Fly production deploy enables the low-noise spike mode in `fly.toml`:

```toml
RTS_PERF = "spikes"
RTS_PERF_SLOW_TICK_MS = "40"
```

That emits one `performance tick summary` log line when a server tick takes at least 40 ms. Phase,
snapshot, and WebSocket writer detail remains at `debug`, so the production default keeps basic
slow-tick visibility without logging every tick.

Perf tracing intentionally exits at startup if `RTS_PERF` is enabled in a debug build. Debug builds
include overflow checks and different optimization behavior, so their timings are not useful for
production lag diagnosis.

Tick summaries separate simulation, snapshot fanout, scheduler lag, room shape, entity counts,
snapshot coalescing, and the slowest concrete simulation phase. Snapshot and writer lines identify
whether lag is in per-player projection/compaction, JSON serialization, or socket writes.

## Structured server logging

Server logs in `server/src` must go through `server/src/structured_log.rs`. Use the helper macros
for ordinary logs:

```rust
crate::log_info!(room = %room, "room created");
crate::log_warn!(room = %room, error = %err, "room task failed");
```

Use a named helper function in `structured_log.rs` when a log needs stable fields, correlation, or
issue classification. Current high-signal helpers cover:

- `client_net_report` with `build_id` and `primary_issue`.
- `performance tick summary` rows include `match_run_id` when emitted by a live room.
- `match_started` with `match_run_id`, map, seed, participants, build, and player counts.
- `match_ended` with `match_run_id`, duration, tick count, slow-tick count, head-of-line max, and
  replay/history context.

`scripts/check-structured-logging.sh` fails if new direct `tracing::{info,warn,error,debug,trace}`
calls are added under `server` outside the helper. The only exception is
`server/crates/sim/src/perf.rs`, which is the centralized simulation performance logging surface and
cannot depend back on the server crate. `tests/run-all.sh` runs that check as part of the
architecture policy gate.
