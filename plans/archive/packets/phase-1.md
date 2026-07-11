# Phase 1 - Packet Budget Measurement

## Phase Status

- [x] Done.

## Objective

Make snapshot single-segment pressure directly visible in client reports, server structured logs, the
incident parser, and local harness output. This phase should answer whether p95 snapshot payloads and
over-budget rates fit a documented payload-byte budget without changing the snapshot format,
transport, gameplay behavior, or fog projection.

## Background

Current `ClientNetReport` fields include `snapshotBytesTotal`, `snapshotBytesMax`,
`snapshotBytesAvg`, and `snapshotMessageCount`, plus parse/decode/apply timing. Server classification
currently treats payload pressure as a very large-frame issue, with thresholds far above a practical
single-segment target. The next compression/encoding phase needs better evidence: p95 byte size,
over-budget counts, and repeatable harness output that compares branches on the same workloads.

## Work

- Choose and document the canonical payload-byte budget:
  - define a constant such as `SNAPSHOT_SINGLE_SEGMENT_BUDGET_BYTES`;
  - prefer a conservative value in the 1200-1350 byte range unless the implementation produces a
    stronger repo-local justification;
  - document that measured payload bytes exclude WebSocket, TLS, TCP, and IP overhead, so 1460 bytes
    of JSON text is not a safe single-segment budget.
- Extend browser-side snapshot byte aggregation:
  - add p95 bytes using the existing bounded report-window aggregate pattern or an equivalent bounded
    histogram;
  - add over-budget snapshot count for the selected budget;
  - add over-budget percentage or fixed-point rate if it can be represented without ambiguity;
  - keep existing total/max/avg/count fields for backwards-compatible logs and parser output.
- Extend `ClientNetReport` with backwards-compatible defaulted fields, using stable names such as:
  - `snapshotBytesP95`;
  - `snapshotSegmentBudgetBytes`;
  - `snapshotOverSegmentBudgetCount`;
  - `snapshotOverSegmentBudgetPctX100` if percentage is added.
- Update server structured logging and classification:
  - log the new fields on `client_net_report`;
  - add a packet-budget classification distinct from the existing large-payload `payload_pressure`
    classification;
  - decide deliberately whether ordinary over-budget reports should always be notable, require a
    minimum sample count, or use a higher p95/rate threshold to avoid excessive beta log volume;
  - keep the old large-payload thresholds for true pathological frames.
- Update `scripts/parse-net-report-logs.mjs`:
  - summarize per-player p95 payload bytes and over-budget rate when fields exist;
  - keep older incident files readable when fields are absent;
  - update missing-data language so packet-loss and retransmit limits remain clear, but packet-budget
    evidence is reported when available.
- Update harness and artifact reporting:
  - include p95 bytes, budget, and over-budget counts in `scripts/client-perf-harness.mjs`
    `summary.json` when available from `ClientNetReport`;
  - ensure the existing AI/server perf path can still report writer payload byte p95 from log parsing
    or add a lightweight helper if there is already a suitable script seam;
  - do not add a brittle CI failure on the absolute byte budget.
- Update docs:
  - `docs/design/protocol.md` field definitions for the new `ClientNetReport` fields;
  - `docs/perf-tracing.md` interpretation notes for packet-budget pressure versus large-payload
    pressure;
  - any relevant incident-example notes only if the parser output changes committed artifacts.
- Add focused tests for:
  - JavaScript aggregation reset behavior and p95/over-budget calculation;
  - Rust serde defaults for older reports missing the new fields;
  - structured log notable/classification behavior;
  - protocol parity and docs coverage;
  - incident parser behavior with both old and new report rows.

## Expected Touch Points

- `client/src/net.js`
- `client/src/client_perf_report.js`
- `client/src/report_window_aggregate.js` if the existing aggregate cannot represent byte p95 cleanly
- `server/crates/protocol/src/lib.rs`
- `server/src/structured_log.rs`
- `scripts/parse-net-report-logs.mjs`
- `scripts/client-perf-harness.mjs`
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- `tests/client_contracts.mjs`
- `tests/client_net_report_fields.mjs`
- `tests/protocol_parity.mjs`
- `tests/net_report_log_parser.mjs`
- Rust tests near `server/src/structured_log.rs` and `server/crates/protocol/src/lib.rs`

## Implementation Checklist

- [x] Pick and document the payload-byte budget constant.
- [x] Add bounded browser aggregation for byte p95 and over-budget count/rate.
- [x] Extend `ClientNetReport` Rust and JavaScript mirrors with serde/default compatibility.
- [x] Update structured logging, notable-report thresholds, and primary-issue classification.
- [x] Update parser summaries and old-log compatibility tests.
- [x] Update client perf harness summaries without adding a hard byte gate.
- [x] Update protocol/perf docs.
- [x] Add focused JS/Rust/parser/protocol tests.
- [x] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- `node tests/net_report_log_parser.mjs`
- `cargo test --manifest-path server/Cargo.toml -p rts-server structured_log`
- `cargo test --manifest-path server/Cargo.toml -p rts-protocol client_net_report`
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If a Rust filter matches zero tests, run the concrete test names added or updated in this phase before
counting verification as passed. If the browser harness cannot run on the implementing machine, state
that explicitly in the handoff and include the lower-level test evidence instead.

## Manual Test Focus

Run a local match long enough for at least one `ClientNetReport` window. Confirm normal gameplay is
unchanged, reports include the new p95/budget fields, and the structured log distinguishes
single-segment budget pressure from existing large-payload pressure. Run the Matt/Alex replay workload
through the browser harness when practical and record p95 bytes plus over-budget rate in the handoff.

## Handoff Expectations

List the selected budget value and why it was chosen. List every new report/log/parser field and the
classification threshold used. Include one current-code baseline from either the Matt/Alex replay,
the browser harness, or the AI/server perf path so Phase 2 has a concrete comparison target.
