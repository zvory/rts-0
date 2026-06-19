# Phase 1 - Permanent Report Foundation

## Phase Status

- [x] Done.

## Objective

Extend the existing `ClientNetReport` path so future lag incidents leave bounded evidence about
payload size, browser parse/decode/apply cost, frame phase cost, and snapshot cadence. This phase
should keep the upload low-noise and should not add a new telemetry service.

## Work

- Extend browser-side aggregation with report-window fields for incoming snapshot payloads:
  - `snapshotBytesTotal`
  - `snapshotBytesMax`
  - `snapshotBytesAvg` or an equivalent derived/logged value
  - `snapshotMessageCount` if this differs from the existing `snapshots` count
- Measure and aggregate browser processing costs around the incoming snapshot path:
  - JSON parse time
  - compact protocol decode time
  - `GameState.applySnapshot` and immediate prediction overlay/apply work
  - max and p95 bucket values where practical
- Promote stable Phase 1 frame-profiler aggregates into `ClientNetReport`:
  - `frameWorkMaxMs`
  - `frameWorkP95Ms`
  - `slowFrameCount`
  - `worstFramePhase`
  - `worstFramePhaseMs`
  - `rendererMaxMs`
  - `rendererP95Ms`
  - bounded shape context such as visible tile count, selected count, viewport size, canvas size,
    device pixel ratio, and prediction mode
- Add snapshot cadence diagnostics that are cheap to compute:
  - max tick gap between received snapshots
  - skipped/duplicate/stale snapshot counters if already tracked by prediction/client state
  - receive burst count or max snapshots received within one frame, if this can be measured without
    raw timestamp arrays
- Extend the Rust `ClientNetReport` DTO with defaults for new fields and update structured logging.
- Update `is_notable_net_report` and classification so local browser work, payload pressure, and
  snapshot burst/cadence issues can be separated from network RTT, server tick lag, WebSocket backlog,
  prediction correction, and WASM prediction budget pressure.
- Update `docs/design/protocol.md` and `docs/perf-tracing.md` with field definitions and incident
  interpretation notes.
- Add focused tests for JavaScript report shape, aggregation reset behavior, Rust serde defaults,
  structured log classification, and protocol parity.
- Decide how this phase relates to `plans/fps/phase-2.md`. Prefer either updating that phase status
  in the implementation PR or explicitly noting in the handoff that this phase satisfies its permanent
  upload scope.

## Expected Touch Points

- `client/src/net.js`
- `client/src/match.js`
- `client/src/match_health.js`
- `client/src/frame_profiler.js`
- `client/src/protocol.js`
- `server/crates/protocol/src/lib.rs`
- `server/src/structured_log.rs`
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- `plans/fps/phase-2.md` if the implementation satisfies or supersedes that phase
- `tests/client_contracts.mjs`
- `tests/protocol_parity.mjs`
- Rust tests near `server/src/structured_log.rs` and `server/crates/protocol/src/lib.rs`

## Implementation Checklist

- [x] Select the exact stable report fields and document any rejected/noisy candidates.
- [x] Add browser aggregation for payload bytes, parse/decode/apply work, and snapshot cadence.
- [x] Add bounded frame-profiler aggregates to the report builder.
- [x] Extend Rust DTOs, structured logging, notable-report thresholds, and classification.
- [x] Update protocol and perf documentation.
- [x] Add focused JS/Rust/protocol tests.
- [x] Resolve or document overlap with `plans/fps/phase-2.md`.
- [x] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- `cargo test --manifest-path server/Cargo.toml -p rts-server structured_log`
- `cargo test --manifest-path server/Cargo.toml -p rts-protocol client_net_report`
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If a Rust filter matches zero tests, run the concrete test names added or updated in the phase before
counting verification as passed.

## Manual Test Focus

Run a local match long enough for at least one report window. Confirm normal gameplay is unchanged,
the browser console has no report errors, and a deliberately induced slow frame or lowered threshold
produces a structured server log with payload, parse/decode/apply, frame phase, and snapshot cadence
fields. Inspect `window.__rtsPerf.summary()` during the same session and confirm uploaded aggregate
labels line up with local-only profiler labels.

## FPS Plan Overlap

`plans/fps/phase-2.md` was already marked done before this phase. This phase preserves that permanent
frame-profiler upload scope and adds the snapshot payload, browser processing, and cadence fields
needed by the network lag report plan.

## Handoff Expectations

List every new `ClientNetReport` field, threshold, and classification rule. Include an example local
or Fly-log query pattern that distinguishes large payloads, parse/decode/apply cost, slow frame phase,
and snapshot cadence problems. Name any fields intentionally kept local-only because they were too
high-cardinality, too expensive, or too privacy-sensitive.
