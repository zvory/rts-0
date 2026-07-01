# Lag Diagnostics Plan

## Purpose

Develop diagnostics that let future agents explain large-game lag incidents from preserved artifacts
without guessing or rereading raw logs line by line. The goal is not to fix command latency, pathing,
packet size, rendering, or transport behavior yet; the goal is to collect and summarize the right
signals so later fixes can be chosen with evidence. This plan builds on `plans/cmdjitter/`,
`plans/lag/`, `docs/perf-tracing.md`, and the
`2026-06-30-beta-soupman-alex-lag` incident.

## Agent-Readable Diagnostic Contract

Every new diagnostic stream in this plan must produce high-value, low-cardinality summaries that are
easy for an agent to interpret:

- Prefer windowed counts, max/p95 buckets, stable enum labels, top-N exemplars, correlation blocks,
  source-coverage sections, and explicit unknowns over raw event arrays.
- Every field must document its unit, owner, reset/window behavior, privacy boundary, and the
  interpretation caveat that prevents overclaiming.
- Parser output should answer "what pressure was indicated, contradicted, or unknown" before showing
  raw rows.
- Optional verbose traces may exist for local or explicitly enabled perf runs, but normal beta net
  reports should stay bounded and safe to preserve in incident directories.
- Do not upload raw command payloads, raw snapshots, raw timestamp arrays, raw frame records, entity
  ids, target ids, player-entered text, stack traces, secrets, or browser-local traces from normal
  clients.

## Overall Constraints

- Treat this as diagnostics and evidence packaging only; do not implement gameplay, transport,
  prediction, pathing, render, or balance fixes in these phases.
- Preserve the server-authoritative model and current fog/privacy guarantees.
- Keep new fields backwards-compatible with existing logs and parser inputs.
- Keep diagnostics bounded by report windows, stable histograms, or explicit opt-in perf modes.
- Prefer structured server-side summaries and parser-derived context over large unstructured log
  messages.
- If a phase changes protocol or net-report shape, update Rust protocol DTOs, server/client mirrors,
  `docs/design/protocol.md`, parser support, and focused parity/default tests in that phase.
- If a phase changes client module wiring, keep dependencies injected through `Match`/`App` and run
  the client architecture check.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite merge with the phase head reachable from `origin/main`
  before the next phase starts.
- When a phase is complete, mark its phase document done in the implementation commit and provide a
  handoff message describing what changed, what the next agent should do, and the core manual test
  focus.

## Diagnostic Questions

The completed plan should let future agents answer these questions from one incident directory:

- Was the main pressure server tick/scheduler, pathing, snapshot projection/serialization, outbound
  writer/backpressure, network delivery, browser parse/decode/apply, render/frame work, prediction
  replay, command density, or command lifecycle delay?
- For command lag, which part of the lifecycle dominated: browser issue/send, server ingress,
  room-event queue, room handling, receipt delivery, sim consumption, snapshot delivery, or client
  apply?
- For snapshot lag, did the server generate snapshots late, enqueue them behind reliable messages,
  serialize/send slowly, deliver them in bursts, or merely produce large payloads that correlated
  with delivery gaps?
- For pathing hitches, what kind of path requests dominated and whether the hitch came from request
  count, path complexity, cache behavior, budget exhaustion, or a specific command family.
- For client-visible frame issues, which stable phase/counter group was responsible and whether
  that local work coincided with command bursts, late snapshots, prediction replay, or payload size.
- Which conclusions are supported, contradicted, unknown, or blocked by missing diagnostics.

## Phase Summaries

### [Phase 1 - Incident Package and Agent Digest](phase-1.md)

Create an agent-first incident package and digest format on top of the existing parser before adding
new telemetry. The package should include an evidence index, key metrics, parser markdown/JSON/TSV,
filtered client and tick rows, provenance, top bad windows, timeline bands, confidence-tagged
classifications, and explicit unknowns from existing logs. This gives future investigations a better
reading surface immediately and establishes the output contract that later phases must feed.

### [Phase 2 - Command Lifecycle Diagnostics](phase-2.md)

Add bounded command lifecycle diagnostics that split command delay into client, transport ingress,
room actor, receipt delivery, simulation acknowledgement, and snapshot/apply stages. The normal
report output should remain aggregate and contextualized, using per-window histograms and top-N
exemplars instead of raw command traces. This phase should make `command_upload_delay` and
`command_server_queue` interpretable without changing command semantics.

### [Phase 3 - Snapshot Lifecycle and Payload Diagnostics](phase-3.md)

Add per-window snapshot lifecycle and payload-composition diagnostics that explain whether large or
late snapshots came from projection, compaction, serialization, writer send, delivery cadence, or
payload shape. The output should summarize bytes and counts by stable snapshot section and entity
kind, plus lifecycle timing by recipient, without preserving raw snapshot data. This phase should
make packet-budget pressure actionable without jumping straight to packet or codec fixes.

### [Phase 4 - Pathing Slow-Tick Diagnostics](phase-4.md)

Add slow-tick pathing diagnostics for `awaiting_paths`, `promoted_awaiting_paths`, and order
promotion so pathing hitches have internal explanations. The logs should summarize pending and
processed request counts, request source families, path complexity buckets, cache/budget signals,
and worst request timing only when useful. This phase should turn "awaiting_paths took 297ms" into a
bounded root-cause summary.

### [Phase 5 - Client Frame and Prediction Context](phase-5.md)

Extend uploaded client diagnostics so frame, renderer, and prediction context is useful when lag is
partly local. The net report should include stable top phase/counter groups, prediction replay
coverage, and late-snapshot visual coverage in a bounded form that mirrors the local
`window.__rtsPerf` view. This phase should help agents decide whether render work, RAF dispatch,
prediction replay, or browser scheduling materially contributed to a bad window.

### [Phase 6 - Reproduction, Capture, and Regression](phase-6.md)

Build the capture, reproduction, and regression loop that proves the diagnostics are useful on fresh
incidents. The phase should add or update local/beta collection scripts, harness output, incident
directory templates, and parser checks so a large-game lag report produces one complete evidence
package. This phase is the gate before any later fixing plan starts from the new diagnostics.

## Phase Index

1. [Phase 1 - Incident Package and Agent Digest](phase-1.md)
2. [Phase 2 - Command Lifecycle Diagnostics](phase-2.md)
3. [Phase 3 - Snapshot Lifecycle and Payload Diagnostics](phase-3.md)
4. [Phase 4 - Pathing Slow-Tick Diagnostics](phase-4.md)
5. [Phase 5 - Client Frame and Prediction Context](phase-5.md)
6. [Phase 6 - Reproduction, Capture, and Regression](phase-6.md)

## Non-Goals

- Do not implement command scheduling, rollback, command coalescing, throttling, transport changes,
  compression, pathing optimization, render optimization, or prediction behavior changes.
- Do not define success by server tick health alone.
- Do not make normal beta logs depend on `RTS_PERF=full`.
- Do not rely on raw log volume as a substitute for parser-derived summaries.
- Do not add unbounded labels or user/device fingerprinting fields.
- Do not make incident analysis depend on private agent memory.

## Required Verification Themes

Each phase should run the smallest relevant subset of:

- `node scripts/parse-net-report-logs.mjs` on preserved incident logs and new fixture logs.
- Focused parser tests for digest sections, source coverage, classifications, and unknowns.
- `node tests/protocol_parity.mjs` for any protocol or net-report shape change.
- Focused Rust tests for `ClientNetReport` serde defaults and structured-log classification.
- Focused JS tests for report aggregation reset behavior and client-side field clamping.
- `node scripts/check-client-architecture.mjs` for client module or wiring changes.
- Focused Rust tests around `TickPerf`, snapshot fanout, connection counters, and movement/pathing
  diagnostics when those areas change.
- A targeted live Node or browser harness only when a phase changes live client/server behavior or
  evidence capture tooling that needs a running server.
- `git diff --check`.

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait gate
and confirm the phase head is reachable from `origin/main`. For unattended executor passes after the
plan is approved, use:

```bash
scripts/phase-runner.sh --plan lagdiagnostics phase-1 --pr --wait
scripts/phase-runner.sh --plan lagdiagnostics phase-2 --pr --wait
scripts/phase-runner.sh --plan lagdiagnostics phase-3 --pr --wait
scripts/phase-runner.sh --plan lagdiagnostics phase-4 --pr --wait
scripts/phase-runner.sh --plan lagdiagnostics phase-5 --pr --wait
scripts/phase-runner.sh --plan lagdiagnostics phase-6 --pr --wait
```
