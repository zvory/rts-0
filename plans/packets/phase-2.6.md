# Phase 2.6 - MessagePack Beta Smoke And Delta Gate

## Phase Status

- [ ] Ready for implementation after Phase 2.5 merges MessagePack binary snapshots and the candidate
      build is available for local or beta verification.

## Objective

Do the short operational closeout for the MessagePack snapshot rollout. This phase should verify the
default MessagePack path on beta once or twice, decide whether to keep or revert it, clean up stale
compression-rollout wording, and state whether packet work should move next to delta snapshots.

## Background

MessagePack is now the chosen encoding route. The old `permessage-deflate` rollout plan is
intentionally dropped because it required WebSocket-stack or browser-negotiation work that is too
risky for the current payoff. Compact JSON compatibility fallback is also intentionally dropped:
this is a pre-alpha game, and stale clients can refresh or fail.

The rollback policy is simple. If MessagePack breaks normal play, produces unacceptable
parse/decode/serialize cost, or fails beta smoke in a way that cannot be fixed quickly, revert the
MessagePack change. Do not turn Phase 2.6 into a long migration, opt-in rollout, or fallback-mode
project.

## Decision Inputs

Start from the Phase 2.5 handoff. It must include:

- the final `messagepack-compact` codec/version/header shape;
- the MessagePack Rust/browser dependency or in-repo decoder choice;
- local compact JSON baseline vs MessagePack payload and timing numbers;
- local smoke status for normal match snapshots, commands, and any replay/spectator/lab/dev-watch
  surfaces checked;
- known gaps that can be accepted for one beta smoke or require immediate revert/fix.

## Work

- Verify the candidate build:
  - confirm `/version` identifies the deployed beta build when beta is used;
  - run one or two representative beta sessions or replay/harness checks, enough to produce client
    reports and server logs;
  - compare packet-budget p95/rate, writer send timing/backlog signals, snapshot gaps/jitter, command
    acknowledgement health, and parse/decode/apply p95 against the Phase 1/2 compact JSON baseline
    and the Phase 2.5 local numbers.
- Make the keep/revert decision:
  - keep MessagePack if normal play works and diagnostics show meaningful payload reduction without
    obvious command/snapshot delivery regression;
  - revert or hand off a concrete blocker if beta or local smoke exposes a serious failure;
  - do not add runtime fallback, opt-in gating, or stale-client compatibility as the response.
- Finalize docs and phase gates:
  - update `docs/perf-tracing.md` if parser/harness language still refers to compression or compact
    JSON defaults;
  - update `docs/design/protocol.md` if the final beta-verified behavior differs from Phase 2.5;
  - update Phase 3 gate text only to say whether MessagePack is the accepted full-snapshot baseline
    for delta work.

## Expected Touch Points

- `plans/packets/phase-2.6.md`
- `plans/packets/phase-3.md` if the delta gate wording needs a final update
- `plans/packets/plan.md` if the phase index or summary needs a final update
- `docs/perf-tracing.md`
- `docs/design/protocol.md`
- `scripts/parse-net-report-logs.mjs` if parser labels need cleanup
- `tests/net_report_log_parser.mjs` if parser labels change
- `tests/client_net_report_fields.mjs` if report fields change

## Implementation Checklist

- [ ] Confirm Phase 2.5 is merged and MessagePack is the active snapshot path.
- [ ] Confirm local smoke results and focused tests from Phase 2.5 are available.
- [ ] Deploy or use beta when practical and confirm `/version` matches the candidate.
- [ ] Run one or two beta sessions/replay checks and collect bounded reports/logs.
- [ ] Decide keep vs revert based on gameplay health and packet-budget evidence.
- [ ] Clean up stale compression/default/fallback wording in docs, parser help, and phase gates.
- [ ] State whether Phase 3 delta work is recommended next and still needs explicit user approval.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/client_net_report_fields.mjs` if report fields changed
- `node tests/net_report_log_parser.mjs` if parser output changed
- `node scripts/check-docs-health.mjs`
- `git diff --check`
- bounded beta smoke when practical:
  - `/version` matches the candidate build;
  - normal match snapshots render and commands acknowledge;
  - reports/logs show `messagepack-compact`;
  - packet-budget pressure improves or remains clearly explainable;
  - no obvious writer backlog, snapshot-gap, or command-ack regression appears.

If beta verification cannot run from the implementation environment, do not invent a fallback plan.
Leave a short handoff with the exact local evidence and the exact beta check still needed.

## Manual Test Focus

On beta, play or observe one representative match long enough for client network reports. Confirm the
game remains playable, commands acknowledge, snapshots keep flowing, and diagnostics identify
MessagePack as the snapshot codec. If it fails, revert the MessagePack rollout instead of designing
a migration.

## Handoff Expectations

State the keep/revert decision, the beta build id checked, the local and beta packet/timing evidence,
and whether Phase 3 delta work is now recommended next, deferred, or blocked pending a revert/fix.
