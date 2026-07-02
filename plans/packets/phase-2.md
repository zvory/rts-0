# Phase 2 - Encoding And Compression Bake-off

## Phase Status

- [x] Done.

## Objective

Compare practical snapshot encoding and compression options against the compact JSON baseline, then
recommend the smallest safe shipping path. This phase should produce reproducible evidence for payload
bytes and CPU cost without making an unproven encoding the default.

## Background

Compact JSON already removes most field-name overhead by using short keys, positional arrays, code
tables, and omitted optional sections. It does not remove JSON number punctuation, repeated structural
characters, or full-state repetition, and the current browser/server code handles incoming and
outgoing WebSocket frames as JSON text only. Binary encodings may reduce payload bytes, while
`permessage-deflate` may reduce repeated JSON structure, but both need explicit compatibility,
performance, and failure-mode checks.

## Work

- Define the experiment boundary:
  - JSON compact snapshots remain the default and the fallback;
  - experiments may run behind an environment variable, dev query flag, or negotiated start payload
    capability, but must not silently strand older clients;
  - live reliable non-snapshot messages may stay JSON unless the experiment proves they matter;
  - measurement must capture encoded bytes, browser parse/decode/apply cost, server serialization
    cost, and any send/backlog effects already available through existing perf logs.
- Add a snapshot codec seam:
  - keep the semantic `Snapshot` DTO as the boundary above the codec;
  - keep `GameState.applySnapshot` receiving the same semantic shape unless a sub-experiment has a
    clearly isolated reason to bypass it;
  - support both text and binary WebSocket frames on the client and server where needed;
  - reject unsupported codec/version combinations with clear diagnostics and a safe fallback or
    connection error.
- Compare these candidates:
  - compact JSON baseline;
  - compact JSON with WebSocket `permessage-deflate`, only if the current axum/tungstenite/browser
    stack supports it without a risky transport fork;
  - protobuf-style schema binary over WebSocket, using generated or carefully maintained schema code;
  - MessagePack as a schema-less or mostly schema-less binary baseline;
  - CBOR as a schema-less binary baseline;
  - custom positional binary for the current compact snapshot schema, with explicit versioning and
    bounds checks.
- For protobuf or custom binary:
  - decide how compact code tables and optional trailing entity fields map to the binary shape;
  - keep numeric widths explicit where possible, especially ids, ticks, hp, resource counts, and
    enum/code values;
  - define how floating point positions/facing are encoded, including whether quantization is part of
    the experiment or deferred.
- For MessagePack and CBOR:
  - measure both a direct semantic object encoding and a compact positional encoding if feasible;
  - do not mistake schema-less convenience for wire stability if the compact positional shape becomes
    the real contract.
- For WebSocket compression:
  - verify whether server support exists in the current dependency stack before implementing;
  - record browser behavior and whether payload byte metrics measure pre-compression or
    post-compression bytes;
  - do not report compressed-on-wire bytes as equivalent to application payload bytes unless the
    measurement path is explicit about the distinction.
- Add reproducible benchmark output:
  - Matt/Alex replay workload;
  - four-AI or AI/server perf harness workload;
  - at least one current live/dev stress path such as the vehicle wall stress workload if available;
  - per-candidate p50/p95/p99/max bytes, over-budget rate, parse/decode/apply p95, server serialize
    p95, and implementation notes.
- Update docs and tests:
  - document the codec capability/selection behavior in `docs/design/protocol.md`;
  - document how to run the bake-off and interpret results in `docs/perf-tracing.md`;
  - add parity/tests for codec version constants, fallback behavior, and malformed binary rejection;
  - keep existing compact JSON protocol tests intact.
- End the phase with a decision artifact:
  - recommend one candidate to ship by default, recommend a follow-up phase, or recommend deferring
    format changes in favor of deltas;
  - include the exact baseline and candidate numbers used for the recommendation;
  - note any dependency, browser, or deployment risk that blocks a candidate;
  - state the exact recommendation Phase 2.5 should apply.

## Expected Touch Points

- `server/crates/protocol/src/lib.rs`
- `server/crates/protocol/Cargo.toml`
- `server/src/main.rs`
- `server/src/lobby/connection.rs` if writer negotiation or per-connection codec state is needed
- `server/src/perf.rs` or writer timing logs if byte/serialize measurement needs labels per codec
- `client/src/net.js`
- `client/src/protocol.js`
- `client/src/client_perf_report.js`
- `scripts/client-perf-harness.mjs`
- `scripts/parse-net-report-logs.mjs` if codec labels appear in reports/logs
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- `tests/client_contracts.mjs`
- `tests/protocol_parity.mjs`
- focused Rust protocol tests for each candidate kept in code
- focused live Node or server integration tests if WebSocket binary negotiation changes connection
  behavior

## Implementation Checklist

- [x] Confirm Phase 1 packet-budget fields are merged and available for comparison.
- [x] Define snapshot codec names, versioning, negotiation/fallback behavior, and defaults.
- [x] Add a codec seam while preserving compact JSON baseline behavior.
- [x] Implement only candidates that can be kept maintainable during the phase; document skipped
      candidates with evidence instead of forcing risky dependencies.
- [x] Add benchmark collection for baseline and each implemented candidate.
- [x] Compare p95 bytes, over-budget rate, parse/decode/apply cost, server serialize cost, and code
      complexity.
- [x] Update protocol/perf docs and focused tests.
- [x] Produce a decision artifact in the phase handoff or a small committed doc if the implementation
      needs one.
- [x] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- focused codec unit tests added under the Rust protocol crate
- focused client decoder tests added or updated for binary/malformed frame behavior
- `node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 6` for each
  candidate when the harness is stable on the implementing machine; use a fresh schema 3 replay
  workload if replay playback evidence is required
- `scripts/ai-perf-harness.sh --ticks 5000 --perf full --no-log-snapshots` or an equivalent
  documented server-side payload benchmark
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If candidate benchmarks cannot all run locally, keep the code defaulting to compact JSON and document
which candidate lacks evidence. Do not ship a new default codec based only on theory.

## Manual Test Focus

Run a normal local match using the default compact JSON path and confirm behavior is unchanged. Then
run each experimental codec path long enough to receive snapshots, issue commands, enter replay or lab
paths if the codec is enabled there, and verify that unsupported clients fail clearly or fall back as
designed. Inspect report/log output to confirm codec labels, p95 bytes, over-budget rate, and
parse/decode/apply timing are present and understandable.

## Handoff Expectations

Provide a comparison table for every candidate attempted: encoded p95 bytes, max bytes, over-budget
rate, browser parse/decode/apply p95, server serialize p95, dependency risk, browser support risk, and
maintenance cost. State the recommendation plainly: ship a candidate, run one more focused phase, or
skip format work and move to delta design. This recommendation is the input to Phase 2.5. If a
candidate was not implemented, explain the concrete blocker or why it was not worth the scope.
