# Network Lag Report Plan

## Purpose

Make future player-reported lag incidents diagnosable from preserved server logs and bounded browser
aggregates. The current `ClientNetReport` path can separate server health, RTT/snapshot jitter, local
frame pacing, and prediction state, but it cannot yet answer whether lag came from payload size,
browser parse/decode/apply cost, command upload delay, downstream snapshot delivery, or transport
head-of-line behavior. This plan strengthens that report path without adding raw snapshot uploads,
raw command logs, per-frame spam, or a separate telemetry service.

## Overall Constraints

- Keep the normal-match upload shape bounded, low-cardinality, and report-window based. Prefer
  10-second aggregates, max/p95 bucket summaries, counters, stable enum labels, and byte totals over
  raw arrays or event streams.
- Treat client reports as advisory and untrusted. They are for diagnosis and Fly-log evidence only;
  they must never affect simulation authority, command validation, fog, matchmaking, or outcomes.
- Do not upload raw commands, raw snapshots, entity ids, player names, replay contents, stack traces,
  or arbitrary browser labels from normal clients.
- Prefer the existing `ClientNetReport` WebSocket message and structured server logging path. Add a
  new wire message only when a diagnostic milestone cannot be reconstructed from report aggregates
  or snapshots.
- Coordinate any protocol field with the Rust DTO, JavaScript report builder/decoder, structured
  logging, `docs/design/protocol.md`, `docs/perf-tracing.md`, and focused tests in the same phase.
- Keep report fields stable enough that preserved incident examples and future log parsers can parse
  them across releases. If a field is experimental, name it as such in docs and keep defaults
  backwards-compatible.
- Do not optimize payloads, renderer code, prediction code, WebSocket behavior, or protocol transport
  in this plan. If the new diagnostics identify an optimization target, record it in the handoff for
  a follow-up optimization plan.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with auto-merge
  armed, then waited on until GitHub reports the PR merged and the phase head is reachable from
  `origin/main`.
- After each phase, the implementing agent must provide a handoff message naming exact verification,
  behavior affected, remaining risks, next-phase guidance, and the core features that should be
  manually tested.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Diagnostic Questions

The complete plan should let a future investigator answer these questions from logs and preserved
artifacts:

- Was the server late, overloaded, or coalescing snapshots for this player?
- Was the player's network path high-latency, jittery, bursty, or dropping snapshot cadence?
- Were outbound snapshots unusually large for that player or map state?
- Did browser JSON parse, compact decode, snapshot apply, frame work, or a specific frame phase consume
  the time?
- Did commands wait on client upload/server receipt, room/sim consumption, downstream snapshot
  delivery, or browser apply/render?
- Is there evidence consistent with WebSocket/TCP head-of-line behavior, or is the problem better
  explained by client frame stalls, Wi-Fi RTT/jitter, or oversized payloads?

## Phase Summaries

### [Phase 1 - Permanent Report Foundation](phase-1.md)

Extend `ClientNetReport` with bounded payload, browser processing, frame phase, and snapshot cadence
aggregates. This phase should make Fly logs answer whether the client was slow because frames were
late, payloads were large, parse/decode/apply work was expensive, or snapshots arrived in bursts. It
absorbs the permanent-upload portion of the active FPS measurement work, so the handoff must state how
`plans/fps/phase-2.md` should be updated or considered satisfied.

### [Phase 2 - Command Timing And Correlation](phase-2.md)

Add enough correlation to split command response delay into issue, server receipt, sim consumption,
snapshot receipt, and browser application. This phase should add `matchRunId`-level correlation for
client reports and choose the smallest wire change needed for server-receipt diagnostics. It should
produce report-window command latency max/p95/oldest-pending evidence without logging raw command
payloads.

### [Phase 3 - Incident Parser And Playbook](phase-3.md)

Add an operator-facing parser and documentation path that turns Fly logs into a compact incident table
like the Matt/Alex example. This phase should combine `client_net_report`, match start/end, optional
server perf rows, and writer timing rows into a repeatable summary. The output should classify likely
lag sources without pretending to prove causes that the collected fields still cannot prove.

## Phase Index

1. [Phase 1 - Permanent Report Foundation](phase-1.md)
2. [Phase 2 - Command Timing And Correlation](phase-2.md)
3. [Phase 3 - Incident Parser And Playbook](phase-3.md)

## Non-Goals

- Do not implement WebTransport, UDP, rollback, or a transport rewrite.
- Do not change gameplay command semantics or prediction authority.
- Do not add a second browser-to-server telemetry endpoint.
- Do not add hard CI gates on absolute FPS, browser timing, network latency, or machine-specific
  performance numbers.
- Do not upload Chrome traces or local-only debug buffers from normal clients.
- Do not reduce snapshot fidelity, fog correctness, or server authority to improve report numbers.

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait gate
and confirm the phase head is reachable from `origin/main`. For unattended executor passes, use:

```bash
scripts/phase-runner.sh --plan net-report phase-1 --pr --wait
scripts/phase-runner.sh --plan net-report phase-1 phase-2 phase-3 --pr --wait
```
