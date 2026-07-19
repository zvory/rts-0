# Command-Density Jitter Investigation Plan

## Archive Status

Archived on 2026-07-18. Phase 1 diagnostics shipped, but the evidence-gathering and reproduction
phases were never completed and their assumptions are now stale. Do not resume this phase sequence
or treat it as a current diagnosis; begin any renewed command-jitter investigation from fresh beta,
runtime, protocol, and client evidence.

## Purpose

Diagnose and make repeatable the player-visible stutter where high-density command input appears to
increase HUD `jit`, snapshot burstiness, and short visual freezes even when server tick and scheduler
lag stay healthy. This plan is intentionally evidence-first: phase 1 adds targeted diagnostics,
phase 2 requires a fresh beta reproduction and neutral analysis, and phase 3 builds local
reproduction tooling so later fixing work does not depend on a human repeatedly recreating the issue.
Do not prescribe or implement transport, prediction, receipt-coalescing, worker-thread, or command
coalescing fixes in this plan; those belong in a follow-up plan after phase 2 and phase 3 produce
actionable evidence.

## Overall Constraints

- Treat this as a diagnostics and reproducibility plan, not a fix plan.
- Preserve the server-authoritative model. Client diagnostics must never affect command validation,
  simulation, fog, matchmaking, or outcomes.
- Keep normal telemetry bounded and low-cardinality. Prefer report-window counters, max/p95 values,
  stable enum labels, and bounded histograms over raw event streams.
- Do not upload raw command payloads, raw snapshots, entity ids beyond existing protocol data,
  browser stack traces, arbitrary labels, secrets, or local-only debug buffers from normal clients.
- Keep the meaning of HUD `jit` clear in docs and analysis: it is snapshot arrival jitter, not
  JavaScript compiler/JIT time.
- Phase 2 is a hard evidence gate. Do not continue to phase 3 from assumptions or previous memory;
  use fresh beta logs and replay/artifact evidence from a deliberately manufactured reproduction.
- Phase 2 analysis must describe what happened and what remains unknown without prescribing a fix.
- Phase 3 should let an agent reproduce the bad condition locally without the user manually opening
  the game and generating command bursts.
- If phase 1 touches protocol fields, update the Rust protocol crate, server protocol mirror, client
  protocol mirror, `docs/design/protocol.md`, log parser, and focused tests in the same phase.
- If phase 1 touches client module wiring, run the client architecture check and keep dependencies
  injected through `Match`/`App` as required by `docs/context/client-ui.md`.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite PR merge with the phase head reachable from
  `origin/main` before the next phase starts.
- When a phase is complete, mark its phase document done in the implementation commit and provide a
  handoff message describing what changed, what the next agent should do, and the core manual test
  focus.

## Diagnostic Questions

The completed investigation should let a future agent answer these questions from preserved
artifacts and automated reproduction output:

- Does high command density reliably correlate with snapshot jitter, snapshot gaps, snapshot bursts,
  command server-queue delay, browser frame gaps, prediction resets, or WASM replay budget pressure?
- Are accepted command receipts, rejected command receipts, other reliable messages, or inbound
  command handling temporally close to delayed snapshot sends?
- Does the client keep rendering frames while snapshots are late, or is the visual freeze primarily a
  main-thread/frame-loop stall?
- Does owned-unit prediction remain active, advance visual ticks, and apply predicted snapshots during
  the bad window?
- Does prediction lose coverage because of disabled reasons, replay-budget resets, state mismatch,
  correction thresholds, pending-command replay cost, or lack of local predicted state?
- Can the same symptom be reproduced locally with a deterministic harness, and does that harness
  produce logs that match the beta signature?

## Phase Summaries

### [Phase 1 - Command-Cadence Diagnostics](phase-1.md)

Add bounded diagnostics that directly connect command bursts, reliable-message pressure, snapshot
send timing, client frame pacing, and prediction health. This phase should make a future beta log
window answer whether command density preceded snapshot jitter or visual stutter, without changing
gameplay command semantics or prescribing a fix. The output is richer structured evidence, updated
parser support, and documentation for how to read the new fields.

### [Phase 2 - Beta Evidence Gate and Neutral Analysis](phase-2.md)

Stop after phase 1 is deployed and require the user to manufacture a fresh beta reproduction with
high-density commands, preserved logs, and replay/artifact evidence. Analyze the resulting evidence
against idle/normal-command/high-command windows and write down what is supported, contradicted, and
still unknown. This phase must not prescribe or implement a solution; it exists to prevent premature
optimization based on a single theory.

### [Phase 3 - Local Reproduction Harness](phase-3.md)

Build a local harness or automated browser/server workflow that reproduces the phase 2 signature
without requiring the user to manually open the game and spam commands. The harness should drive
controlled command densities, capture the same diagnostics as beta, and classify whether the local
run matches the preserved incident signature. Its purpose is to give later fixing agents a repeatable
loop for validating changes before any beta/manual repro.

## Phase Index

1. [Phase 1 - Command-Cadence Diagnostics](phase-1.md)
2. [Phase 2 - Beta Evidence Gate and Neutral Analysis](phase-2.md)
3. [Phase 3 - Local Reproduction Harness](phase-3.md)

## Non-Goals

- Do not implement accepted-receipt coalescing in this plan.
- Do not implement command input throttling or move-command coalescing in this plan.
- Do not move networking into a Web Worker in this plan.
- Do not change snapshot fanout priority, reliable-message behavior, prediction behavior, or command
  semantics in this plan.
- Do not add anti-cheat, rollback, lockstep, UDP, WebTransport, or a transport rewrite.
- Do not define success by server tick health alone.
- Do not rely on the user's manual repro as the long-term validation loop.

## Required Verification Themes

Each phase should run the smallest relevant subset of:

- `node scripts/parse-net-report-logs.mjs` on captured evidence.
- `node tests/protocol_parity.mjs` for any protocol/report shape change.
- `node scripts/check-client-architecture.mjs` for client module or wiring changes.
- Focused JS tests for report aggregation reset behavior and parser output.
- Focused Rust tests for `ClientNetReport` serde defaults and structured-log classification.
- Targeted live Node integration or browser smoke only when the phase changes live client/server
  behavior or adds a harness that needs a running server.
- `git diff --check`.

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait gate
and confirm the phase head is reachable from `origin/main`. For unattended executor passes after the
plan is approved, use:

```bash
scripts/phase-runner.sh --plan cmdjitter phase-1 --pr --wait
scripts/phase-runner.sh --plan cmdjitter phase-2 --pr --wait
scripts/phase-runner.sh --plan cmdjitter phase-3 --pr --wait
```
