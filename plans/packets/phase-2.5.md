# Phase 2.5 - Selected Encoding Rollout

## Phase Status

- [ ] Ready for implementation after Phase 2 is merged and its decision artifact names the encoding
      or compression recommendation to apply.

## Objective

Apply the Phase 2 encoding/compression recommendation before any delta work begins. This phase turns
the bake-off result into a clear runtime decision: ship a selected codec or compression path safely,
keep it opt-in with documented blockers, or explicitly defer format changes and preserve compact JSON
as the default.

## Background

Phase 2 compares compact JSON, WebSocket compression, protobuf-style schema binary, MessagePack,
CBOR, and custom positional binary. That comparison may leave experiment code, feature flags,
benchmark harnesses, and docs describing multiple candidates. The project needs a narrow follow-up
that applies the recommendation instead of making Phase 3 guess whether encoding work shipped,
stayed experimental, or was rejected in favor of deltas.

## Decision Inputs

Start from the Phase 2 decision artifact. It must include:

- the candidate or default/fallback policy Phase 2 recommends;
- the exact baseline and candidate measurements behind that recommendation;
- dependency, browser, deployment, and maintenance risks;
- whether the recommendation is default rollout, opt-in/beta rollout, one more focused hardening
  pass, or deferral in favor of delta work.

If Phase 2 did not produce enough evidence to choose one of those outcomes, keep compact JSON as the
default, document the missing evidence, and do not start delta phases from ambiguous assumptions.

## Work

- Apply the Phase 2 recommendation:
  - if Phase 2 recommends shipping a candidate, implement or harden the selected runtime path;
  - if Phase 2 recommends opt-in only, keep the flag/capability explicit and document the blocker for
    default rollout;
  - if Phase 2 recommends deferring format changes, disable or remove experiment-only paths that
    would confuse later work while keeping useful measurement tooling;
  - if Phase 2 recommends one more focused hardening pass, keep the scope limited to the named
    candidate and produce the final rollout/defer decision in this phase.
- Keep compact JSON as the compatibility fallback:
  - unsupported clients must fall back or fail clearly according to the documented negotiation
    contract;
  - rollback must be a runtime flag, negotiated capability, or similarly quick operational switch;
  - no selected codec may strand replay, spectator, lab, branch, or dev-watch paths.
- Finalize runtime selection:
  - name the default snapshot codec for normal live matches;
  - document whether replay, spectator, lab, branch, and dev-watch use the same codec or force
    compact JSON;
  - surface active codec/version in client reports, server logs, and perf harness summaries;
  - preserve existing reliable non-snapshot message behavior unless Phase 2 showed it matters.
- Harden the selected path:
  - keep semantic `Snapshot` as the boundary above the codec unless Phase 2 explicitly proved a safe
    exception;
  - keep `GameState.applySnapshot` receiving the same semantic shape;
  - enforce codec version constants, bounds checks, malformed-frame rejection, and fallback behavior;
  - keep byte metrics explicit about application payload bytes versus compressed-on-wire bytes.
- Update docs and cleanup:
  - update `docs/design/protocol.md` with the final codec/default/fallback contract;
  - update `docs/perf-tracing.md` with the selected measurement and rollout controls;
  - remove stale experiment-only code for rejected candidates when it is safe to do so;
  - leave Phase 3 delta docs untouched except for the gate that points to this phase.

## Expected Touch Points

- `server/crates/protocol/src/lib.rs`
- `server/crates/protocol/Cargo.toml` if the selected codec needs a retained dependency
- `server/src/main.rs`
- `server/src/lobby/connection.rs`
- `server/src/perf.rs` or structured snapshot-send logs
- `client/src/net.js`
- `client/src/protocol.js`
- `client/src/client_perf_report.js`
- `scripts/client-perf-harness.mjs`
- `scripts/parse-net-report-logs.mjs`
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- `tests/client_contracts.mjs`
- `tests/protocol_parity.mjs`
- focused Rust protocol tests for the selected/default/fallback path
- focused client decoder tests for selected codec, fallback, and malformed frames

## Implementation Checklist

- [ ] Confirm Phase 2 is merged and its decision artifact is available.
- [ ] State which Phase 2 recommendation this phase is applying.
- [ ] Choose default-on, opt-in only, focused hardening, or deferred/no-op for encoding changes.
- [ ] Implement the selected runtime selection, fallback, and rollback behavior.
- [ ] Remove or disable experiment-only paths that should not survive the decision.
- [ ] Update report/log/parser/harness output so the active codec and version are visible.
- [ ] Update protocol/perf docs with the final encoding default and fallback contract.
- [ ] Add or keep focused tests for codec constants, negotiation, fallback, and malformed input.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- focused Rust protocol tests for retained codec encode/decode, fallback, and malformed rejection
- focused client decoder tests for the selected codec and compact JSON fallback
- `node scripts/client-perf-harness.mjs --workload matt-alex-replay --seconds 6` for the selected
  default or opt-in path when practical
- `scripts/ai-perf-harness.sh --ticks 5000 --perf full --no-log-snapshots` or the Phase 2
  documented server-side payload benchmark for the selected path when practical
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If the selected candidate cannot be verified locally across representative workloads, keep it opt-in
or disabled by default and document the exact missing evidence.

## Manual Test Focus

Run a normal local match in the selected default mode and in the compact JSON fallback mode. Confirm
snapshots apply, commands acknowledge, fog remains correct, replay entry/seek works, spectator view
renders, lab/dev-watch paths still start, and diagnostics clearly name the active codec/version.

## Handoff Expectations

State the Phase 2 recommendation applied, the final default/opt-in/deferred decision, the rollback
switch, the retained fallback path, and the exact verification run. If encoding changes were
deferred, explicitly tell Phase 3 whether delta work is now the recommended next step and whether
the user has approved moving beyond encoding/compression.
