# Phase 3 - Snapshot Lifecycle and Payload Diagnostics

## Phase Status

- [x] Done.

## Objective

Explain snapshot pressure without preserving raw snapshots or assuming payload size alone is the
root cause. This phase should show whether bad windows came from snapshot projection, compaction,
serialization, writer send, delivery cadence, payload composition, or client processing.

## Work

- Add bounded per-recipient snapshot lifecycle summaries:
  - snapshot projected
  - compacted for wire
  - queued in latest-only slot
  - replaced or stored
  - taken by writer
  - serialized
  - sent
  - observed by client
  - parsed/decoded/applied by client
- Make snapshot and writer timing rows useful in `spikes` or `sample` perf modes without requiring
  full raw logging for every snapshot.
- Add payload composition summaries by stable section:
  - entities
  - visible or explored tiles
  - resource deltas
  - events
  - smokes
  - ability objects
  - trenches
  - player resources/status
  - net status
  - other
- Add entity count and approximate bytes by stable entity kind or kind family where feasible.
- Preserve section counts/bytes as buckets or top-N summaries; do not log raw snapshot bodies,
  entity ids, positions, target ids, or hidden fog data.
- Extend parser output so packet-budget pressure is tied to payload composition and lifecycle
  timing.

## Expected Touch Points

- `server/src/lobby/snapshot_fanout.rs`
- `server/src/lobby/snapshots.rs`
- `server/src/lobby/connection.rs`
- `server/src/main.rs`
- `server/crates/sim/src/perf.rs`
- `server/crates/protocol/src/compact_snapshot.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/sim/src/rules/projection.rs`
- `server/src/structured_log.rs`
- `client/src/net.js`
- `client/src/client_perf_report.js`
- `client/src/match_health.js`
- `scripts/parse-net-report-logs.mjs`
- `docs/perf-tracing.md`
- `docs/design/protocol.md` if report fields change

## Agent-Readable Output Requirements

- The digest should distinguish payload size, server snapshot work, writer send age, network arrival
  gap, and client parse/decode/apply cost.
- Payload composition should be summarized as percentages and top contributors, not as raw section
  dumps.
- If WebSocket compression is absent or present, the digest should state that payload bytes are
  application payload bytes and not full wire bytes.
- Missing writer/snapshot timing rows must remain explicit unknowns unless a new aggregate field
  covers the gap.
- Parser output should avoid implying that crossing the 1280-byte single-segment budget proves packet
  loss.

## Implementation Checklist

- [x] Define snapshot lifecycle and payload composition field names.
- [x] Add bounded server-side aggregation for projection, compaction, enqueue, serialize, and send
      timing.
- [x] Add payload section counts/bytes or stable approximations.
- [x] Add client-side arrival/processing aggregation only where existing fields are insufficient.
- [x] Extend structured logs, parser output, and docs.
- [x] Add focused tests for compact snapshot instrumentation and parser interpretation.
- [x] Mark this phase as done in this file in the implementation commit.

## Verification

- focused Rust tests for snapshot diagnostics and defaults
- `cargo test --manifest-path server/Cargo.toml -p rts-server connection_sink`
- `node tests/net_report_log_parser.mjs`
- focused JS tests for client snapshot report aggregation if changed
- focused parser fixture tests
- `node tests/protocol_parity.mjs` if report shape changes
- `node scripts/check-client-architecture.mjs` if client wiring changes
- `git diff --check`

## Manual Test Focus

Run a local match or replay that emits snapshots with enough entities to exceed the packet budget.
Confirm the digest names the largest snapshot sections and separates server lifecycle timing from
client arrival gaps. Confirm no raw snapshot payloads or hidden entity details appear in logs.

## Handoff Expectations

List the new snapshot lifecycle fields and payload section labels. Include before/after parser
examples showing how the Soupman/Alex style packet-budget pressure would now be explained. Tell the
next phase what slow-tick/pathing questions remain unrelated to snapshot payloads.
