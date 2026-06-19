# Phase 2.6 - Compression Rollout

## Phase Status

- [ ] Ready for implementation after Phase 2.5 proves a selected compression path improves realistic
      delivery pressure and names the rollout mode.

## Objective

Ship the Phase 2.5 compression decision before any delta snapshot work begins. This phase should turn
the verified compression path into an operational runtime mode with a rollback switch, clear
diagnostics, compact JSON fallback, and enough local/beta evidence to decide whether compression is
the default for normal live matches.

## Background

Phase 2.5 is intentionally evidence-first: it may prove real `permessage-deflate`, find that the
current stack needs a small transport change, or recommend a different compression route. Phase 2.6
applies that result. It should not re-open protobuf, MessagePack, CBOR, or custom binary unless
Phase 2.5 explicitly rejects WebSocket compression and names one of those as the next best shipping
candidate.

## Decision Inputs

Start from the Phase 2.5 handoff. It must include:

- the selected compression route;
- whether local and beta/Fly WebSocket negotiation were confirmed;
- before/after metrics for Matt/Alex, stress, and AI/server workloads;
- server CPU/compression timing, writer backlog, snapshot gap/jitter, command acknowledgement, and
  packet-budget evidence;
- the recommended rollout mode: default-on, beta-only, opt-in, implementation-route follow-up, or
  defer encoding changes.

If Phase 2.5 did not prove a production-relevant improvement, do not force a default rollout. Keep
compact JSON as the default and explicitly tell Phase 3 whether delta work is the recommended next
step.

## Work

- Implement the selected compression runtime path:
  - enable the chosen WebSocket compression support or documented transport change;
  - keep reliable non-snapshot messages on the existing JSON semantics unless Phase 2.5 proved they
    need special handling;
  - keep semantic `Snapshot` and `GameState.applySnapshot` unchanged.
- Add operational controls:
  - add a server-side runtime flag or env var to disable compression quickly;
  - document default, beta, and rollback behavior;
  - ensure clients that do not negotiate compression continue receiving compact JSON text.
- Finalize diagnostics:
  - report active compression state and codec/version in client reports, server logs, parser output,
    and harness summaries;
  - label byte metrics clearly as application payload bytes, negotiated compressed bytes, or proxy
    estimates;
  - include enough fields to answer whether a laggy session actually used compression.
- Verify compatibility surfaces:
  - live match;
  - replay entry and seek;
  - spectator/observer view;
  - lab and dev-watch paths;
  - reconnect and unsupported-client behavior;
  - rollback-disabled compact JSON path.
- Clean up or preserve experiment code deliberately:
  - keep useful bake-off and harness tools;
  - remove or mark stale experiment-only candidates that would confuse later phases;
  - update Phase 3 gate text only to point at this phase's final decision.

## Expected Touch Points

- `server/Cargo.toml` and `server/Cargo.lock` if the selected route changes dependencies
- `server/src/main.rs`
- `server/src/structured_log.rs`
- `server/src/perf.rs` or writer timing logs
- `client/src/net.js`
- `client/src/client_perf_report.js`
- `scripts/client-perf-harness.mjs`
- `scripts/parse-net-report-logs.mjs`
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- `tests/client_contracts.mjs`
- `tests/client_net_report_fields.mjs`
- `tests/net_report_log_parser.mjs`
- focused live Node or browser tests if connection negotiation changes

## Implementation Checklist

- [ ] Confirm Phase 2.5 is merged and names the selected compression rollout mode.
- [ ] Implement the selected compression path without changing snapshot semantics.
- [ ] Add a quick rollback/disable switch and document it.
- [ ] Keep compact JSON text as the fallback when compression is disabled or not negotiated.
- [ ] Finalize report/log/parser/harness fields for active compression and byte interpretation.
- [ ] Verify live, replay, spectator, lab/dev-watch, reconnect, and rollback-disabled paths.
- [ ] Run local realistic benchmarks and beta verification for the final rollout mode.
- [ ] Update protocol/perf docs and focused tests.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/client_net_report_fields.mjs`
- `node tests/net_report_log_parser.mjs`
- focused Rust/server tests for compression selection, rollback flag, and logging if those seams are
  added
- focused live Node or browser test for WebSocket negotiation if practical
- `node scripts/client-perf-harness.mjs --workload matt-alex-replay --seconds 6`
- high-entity stress harness run with compression enabled and disabled
- `scripts/ai-perf-harness.sh --ticks 5000 --perf full --no-log-snapshots` or the Phase 2.5
  documented server-side compression benchmark
- bounded beta verification:
  - `/version` matches the candidate build;
  - negotiated compression state appears in reports/logs;
  - packet-budget pressure, writer backlog, snapshot gaps/jitter, and command acknowledgement health
    improve or remain acceptable;
  - rollback-disabled compact JSON path still works.
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If beta verification or rollback verification cannot run, keep the rollout opt-in or beta-only and
document the missing evidence.

## Manual Test Focus

Play a normal match with compression enabled and again with the rollback switch disabling it. Confirm
that gameplay is unchanged, snapshots continue flowing, commands acknowledge, replay and lab/dev-watch
paths render, and diagnostics make the active compression state obvious. On beta, compare a
representative match or replay against the Phase 2.5 baseline and watch for lower packet-budget
pressure without new writer backlog or command-latency regressions.

## Handoff Expectations

State the final default, beta, and rollback settings. Include the local and beta before/after
measurements used for the rollout decision. State whether encoding/compression work is now complete
and whether Phase 3 delta work is recommended next, still deferred, or needs explicit user approval.
