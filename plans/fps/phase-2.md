# Phase 2 - Permanent Client Perf Reports

## Phase Status

- [x] Done.

## Objective

Send bounded client frame/render performance aggregates through the existing `ClientNetReport` path so
future lag incidents leave actionable Fly logs automatically. This phase should make server logs answer
which client-side phase was slow, not just that FPS was low.

## Work

- Extend the `ClientNetReport` schema with stable, low-cardinality performance fields derived from
  Phase 1. Prefer bounded numeric aggregates such as:
  - `frameWorkMaxMs`
  - `frameWorkP95Ms`
  - `slowFrameCount`
  - `worstFramePhase`
  - `worstFramePhaseMs`
  - `rendererMaxMs`
  - `rendererP95Ms`
  - `longTaskCount` and `longTaskMaxMs` if browser support is reliable
  - entity/selected/visible-tile/viewport/device-pixel-ratio context fields
- Keep field names compact and stable. Avoid raw arrays, stack traces, dynamic labels, player names,
  entity ids, command payloads, or replay data.
- Update the JavaScript report builder in `Match.sendNetReport()` to include the new aggregates and
  reset only the report-window counters that should reset.
- Update the Rust `ClientNetReport` DTO and structured server logging so notable reports include the
  new fields in Fly logs.
- Update `is_notable_net_report` and issue classification so severe local frame/paint cost can be
  separated from network RTT, snapshot jitter, server tick lag, websocket backlog, prediction
  correction, and WASM prediction budget pressure.
- Update `docs/design/protocol.md` and `docs/perf-tracing.md` to document the new report fields and
  how to interpret them during incidents like the Matt/Alex match.
- Add or update tests for protocol deserialization defaults, JS report shape, structured log
  classification, and docs/contract parity.
- Do not add a new upload endpoint or a second telemetry service.

## Expected Touch Points

- `client/src/match.js`
- `client/src/match_health.js`
- Phase 1 profiler module/debug surface
- `client/src/protocol.js`
- `server/crates/protocol/src/lib.rs`
- `server/src/structured_log.rs`
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- `tests/client_contracts.mjs`
- Rust tests near `server/src/structured_log.rs` and protocol tests as needed

## Implementation Checklist

- [x] Select the stable Phase 1 aggregate fields to upload.
- [x] Extend JS report generation with bounded fields.
- [x] Extend the Rust protocol DTO with backwards-compatible defaults where appropriate.
- [x] Extend structured logging and issue classification.
- [x] Update protocol/perf docs.
- [x] Add focused JS/Rust tests.
- [x] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- `cargo test --manifest-path server/Cargo.toml -p rts-server structured_log`
- `cargo test --manifest-path server/Cargo.toml -p rts-protocol client_net_report`
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If the exact Rust test filter matches zero tests, use the concrete test names added or updated in this
phase before counting verification as passed.

## Manual Test Focus

Run a local match long enough for at least one report window. Confirm normal gameplay is unchanged,
the browser sends reports without console errors, and the server log emits a notable client report when
you induce a slow frame or lower the local threshold during development.

## Handoff Expectations

List every new `ClientNetReport` field, the thresholds/classification rules, and an example Fly-log
query or local log pattern that can diagnose a slow client. Note any aggregate that was intentionally
kept local-only because it was too noisy, too high-cardinality, or too expensive.
