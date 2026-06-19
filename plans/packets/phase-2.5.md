# Phase 2.5 - Real Compression Viability

## Phase Status

- [ ] Ready for implementation after Phase 2 is merged and its decision artifact recommends a
      compression follow-up.

## Objective

Prove whether compact JSON over real WebSocket compression can improve packet-budget pressure in the
actual server/browser/deployment path. This phase should answer the production-relevant question:
can Chrome and the Rust/Fly WebSocket stack negotiate `permessage-deflate`, and does that reduce
snapshot delivery pressure on realistic workloads without creating server writer backlog or CPU
pressure? It should not change the default live snapshot path yet.

## Background

Phase 2 showed that offline `deflateRaw` compressed deterministic compact snapshot fixtures from
17,533 p95 bytes to 4,466 p95 bytes. That is the strongest size result by far, but it was not a real
WebSocket measurement: it did not prove that the current `axum`/`tokio-tungstenite` server supports
`permessage-deflate`, that Chrome negotiates it on `/ws`, or that beta/Fly preserves the extension.

This phase turns that offline result into runtime evidence. The expected fallback remains the current
compact JSON text snapshot path; if compression is unsupported or too risky in the current stack, the
phase should document the blocker and the smallest implementation route for Phase 2.6 rather than
starting delta work from ambiguity.

## Work

- Verify WebSocket compression support in the current stack:
  - inspect the current `axum`/`tokio-tungstenite`/`tungstenite` feature surface and determine
    whether `permessage-deflate` can be enabled directly;
  - if direct support is missing, identify the smallest viable route: dependency feature, transport
    wrapper, alternate WebSocket crate, or explicit application-level compression as a separate
    choice;
  - keep this decision documented in the phase handoff and docs.
- Add real runtime negotiation diagnostics:
  - expose whether the browser sees `permessage-deflate` through `WebSocket.extensions`;
  - surface negotiated compression state in bounded client reports, server structured logs, and
    browser harness summaries;
  - distinguish application payload bytes from negotiated/compressed wire evidence so logs do not
    claim post-compression bytes when they only measured JSON text length.
- Build a production-like compression benchmark path:
  - run the Matt/Alex replay workload through the browser harness with compression diagnostics;
  - run a stress workload such as vehicle-wall stress or an equivalent high-entity local/dev path;
  - run an AI/server perf workload that captures snapshot serialization cost, writer send/backlog
    signals, and packet-budget p95/rate before and after the compression candidate;
  - when practical, deploy or use beta and verify the same negotiation/report fields through `/ws`,
    `/version`, server logs, and incident parser output.
- Compare against the Phase 1/2 baselines:
  - report p50/p95/p99/max application payload bytes;
  - report negotiated compression state and, if measurable, compressed bytes or a clearly labeled
    compressed-byte proxy;
  - report snapshot parse/decode/apply p95, server serialization/compression p95, writer backlog,
    snapshot gaps/jitter, and command acknowledgement health;
  - call out whether the observed gain is large enough to spend CPU on compression before delta work.
- Keep default behavior unchanged:
  - compact JSON without required compression remains the compatibility path;
  - no client should require a new decoder in this phase;
  - if a compression experiment is added, keep it opt-in or beta-only with a clear rollback switch.
- Update docs and tests:
  - update `docs/perf-tracing.md` with the real-compression benchmark commands and interpretation;
  - update `docs/design/protocol.md` only if runtime negotiation/report fields change the protocol
    contract;
  - add focused tests for new report/log/parser/harness fields and fallback behavior.

## Expected Touch Points

- `server/Cargo.toml` and `server/Cargo.lock` if dependency features or a WebSocket crate change
- `server/src/main.rs`
- `server/src/structured_log.rs`
- `server/src/perf.rs` or writer timing logs
- `client/src/net.js`
- `client/src/client_perf_report.js`
- `scripts/client-perf-harness.mjs`
- `scripts/parse-net-report-logs.mjs`
- `scripts/fly-logs.sh` only for bounded beta verification, not for token handling changes
- `docs/design/protocol.md` if report/protocol fields change
- `docs/perf-tracing.md`
- `tests/client_contracts.mjs`
- `tests/client_net_report_fields.mjs`
- `tests/net_report_log_parser.mjs`

## Implementation Checklist

- [ ] Confirm Phase 2 is merged and `phase-2-bakeoff.md` recommends WebSocket compression follow-up.
- [ ] Determine whether the current Rust WebSocket stack can negotiate `permessage-deflate`.
- [ ] Add bounded diagnostics for negotiated compression state without changing the default path.
- [ ] Add or extend harnesses to capture Matt/Alex, stress, and AI/server compression evidence.
- [ ] Run local realistic benchmarks and record before/after payload, timing, and backlog results.
- [ ] Verify beta/Fly negotiation and log/parser output when credentials and deployment access are
      available.
- [ ] Decide whether Phase 2.6 should ship compression by default, ship opt-in/beta-only, or defer.
- [ ] Update protocol/perf docs and focused tests.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/client_net_report_fields.mjs`
- `node tests/net_report_log_parser.mjs`
- `node scripts/client-perf-harness.mjs --workload matt-alex-replay --seconds 6 --snapshot-codec-bakeoff`
- a high-entity stress browser harness workload, preferably `vehicle-wall-stress` if available
- `scripts/ai-perf-harness.sh --ticks 5000 --perf full --no-log-snapshots` or an equivalent
  documented server-side compression benchmark
- bounded beta verification when practical:
  - confirm `/version` identifies the candidate build;
  - confirm the browser reports negotiated compression state;
  - confirm server logs and parser summaries show compression state and delivery/backlog effects.
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If beta verification cannot run in the implementation environment, leave compression default-off and
state the exact missing production evidence in the handoff.

## Manual Test Focus

Run a normal local match with compression disabled and with the compression candidate enabled. Confirm
that snapshots apply, commands acknowledge, replay/lab entry still works, and diagnostics visibly
state whether compression was negotiated. On beta, run a representative match or replay long enough
to produce client network reports and compare packet-budget pressure, writer backlog, snapshot gaps,
and command acknowledgement health against the pre-compression baseline.

## Handoff Expectations

State whether real `permessage-deflate` negotiated locally and on beta. Include the measured
before/after table for Matt/Alex, stress, and AI/server workloads. State the exact recommendation for
Phase 2.6: default-on rollout, opt-in/beta rollout, implementation-route follow-up, or deferral
before delta work.
