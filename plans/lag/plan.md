# Adaptive Command Cadence Plan

## Purpose

Eliminate player-visible command-response lag by making live commands run through a short,
predictable command cadence instead of variable remote-echo timing. The first rollout target is a
two-tick command lead at 30 Hz, with per-player adaptive increases when network jitter makes two
ticks too aggressive. The server remains authoritative; the browser uses owner-safe prediction and
replay so local owned-world response starts on the same effective tick the server is expected to
use.

This plan builds on the existing movement prediction setting and prediction debug surfaces. Do not
add a second player-facing toggle for this effort: the existing "Movement prediction" control and
`rts.prediction.enabled` preference remain the master switch for scheduled-command prediction,
local owned-intent overlays, WASM prediction, and the debug surfaces introduced here. When the
setting is off, the live match must keep the current authoritative-only command path.

## Core Model

- Every sequenced gameplay command carries an intended `executeTick`.
- The client initially schedules commands two ticks ahead of its current server-tick estimate.
- The server queues commands for the requested effective tick when they arrive in time.
- A command that arrives after its requested effective tick is applied at the next legal
  authoritative tick, marked late in owner-only result metadata, and used to raise that player's
  future command lead.
- The client predicts from the intended effective tick, then imports authoritative snapshots and
  replays pending commands forward to the current display tick instead of visually rewinding to an
  old server pose.
- Server authority still wins for validation, combat, fog, resource income, production completion,
  spawns, upgrades, death, and match outcome.

## Cross-Phase Constraints

- Keep the server-authoritative model. Client prediction is display and responsiveness only.
- Keep prediction owner-safe. Do not send hidden enemy ids, hidden positions, hidden orders, target
  ids, enemy economy, or full-world state to support prediction.
- Keep the existing movement prediction setting as the rollout/debug gate. Prediction disabled
  must clear local overlays and preserve monotonic `clientSeq` allocation.
- Start with `commandLeadTicks = 2`; adapt upward only from measured late arrivals, excessive
  correction, or repeated jitter. Decay downward slowly after stable windows.
- Treat late commands as expected under bad networks, not as fatal desyncs. They must be
  measurable, visible in diagnostics, and bounded by reconciliation tests.
- Prefer compact owner-only command result metadata over broad snapshots when the client cannot
  prove acceptance from existing owner-visible fields.
- Use tri-state scenarios for every new command family before enabling its local owned-world
  response by default.
- Keep existing latest-only snapshot coalescing; prediction must tolerate dropped, skipped,
  duplicated, and burst-delivered snapshots.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite merge with the phase head reachable from `origin/main`
  before the next phase starts.
- When a phase is complete, mark its phase document done in the implementation commit and provide a
  handoff message describing what changed, what the next agent should do, and the core manual test
  focus.

## Phase Summaries

### [Phase 1 - Effective-Tick Protocol and Diagnostics](phase-1.md)

Add the wire and snapshot contract for scheduled command execution without changing live behavior
yet. Commands gain an intended `executeTick`, and owner-only command result diagnostics record
requested, accepted, applied, and late ticks. This phase proves the protocol and logs can explain
the command cadence before local prediction depends on it.

### [Phase 2 - Client Command Cadence Controller](phase-2.md)

Build the browser-side command cadence controller behind the existing Movement prediction setting.
The client estimates server tick from snapshots, starts at a two-tick lead, stamps commands with
effective ticks, and records local command timeline diagnostics while still tolerating the old
server path. This phase must prove prediction-off and prediction-on command sequencing stay
monotonic and debuggable.

### [Phase 3 - Server Scheduled Command Queue](phase-3.md)

Make the room task execute queued player commands on their accepted effective ticks. Commands that
arrive late execute at the next legal authoritative tick, emit owner-only late metadata, and update
that player's future lead recommendation. This phase creates the authoritative cadence contract
that prevents local two-tick prediction from becoming guaranteed rubberbanding.

### [Phase 4 - Movement Prediction on Effective Ticks](phase-4.md)

Rework owned-unit movement prediction so local motion starts on the accepted command cadence and
reconciles by replaying forward from authoritative snapshots. Existing move, attack-move, stop,
hold, and queued movement scenarios must be rerun under two-tick, delayed, late, bursty, and
coalesced snapshot profiles. The player-facing goal is that healthy connections get stable
two-tick response, while bad connections correct once and adapt rather than repeatedly snapping.

### [Phase 5 - Unit Intent Surfaces](phase-5.md)

Expand local owned-world response for non-movement unit orders: attack target, gather, setup,
teardown, and ability intent posture. These predictions should show accepted owned-unit intent,
pathing/posture, target markers, windups, and queue changes without predicting hidden enemies,
damage, resource income, deaths, or ability outcomes. Each command family needs tri-state accepted,
late, rejected, no-op, and correction coverage before default enablement.

### [Phase 6 - Building, Rally, Queue, and Build Intent](phase-6.md)

Expand provisional owned-world response for building-facing commands: rally, train, research,
cancel, build intent, and safe progress displays. Existing train/rally optimism should move onto
the same scheduled-command result model, while build intent gets a reversible owner-only local
ghost and only becomes an authoritative scaffold after the server confirms it. This phase must not
predict resource spending, supply changes, spawned units, completed upgrades, or completed
buildings before server snapshots confirm them.

### [Phase 7 - Prediction Worker and Frame-Pacing Budget](phase-7.md)

Move or isolate expensive prediction/replay work so the lag fix does not create client frame lag on
weaker machines. The target is a worker-backed or otherwise budgeted prediction runner with a
graceful degradation mode that keeps accepted-intent overlays while shrinking the prediction
horizon if CPU pressure is high. This phase uses the existing frame profiler, net reports, and a
repeatable browser perf harness to prove the command loop paints promptly.

### [Phase 8 - Rollout, Tuning, and Regression Matrix](phase-8.md)

Turn the adaptive cadence into the default behavior under the Movement prediction setting after
the correctness and performance gates pass. Lock in thresholds for two-tick floor, upward lead
adjustment, decay, late-command correction budgets, and fallback modes. This phase updates docs,
operator playbooks, and tri-state/perf suites so future gameplay work cannot quietly break command
responsiveness.

## Phase Index

1. [Phase 1 - Effective-Tick Protocol and Diagnostics](phase-1.md)
2. [Phase 2 - Client Command Cadence Controller](phase-2.md)
3. [Phase 3 - Server Scheduled Command Queue](phase-3.md)
4. [Phase 4 - Movement Prediction on Effective Ticks](phase-4.md)
5. [Phase 5 - Unit Intent Surfaces](phase-5.md)
6. [Phase 6 - Building, Rally, Queue, and Build Intent](phase-6.md)
7. [Phase 7 - Prediction Worker and Frame-Pacing Budget](phase-7.md)
8. [Phase 8 - Rollout, Tuning, and Regression Matrix](phase-8.md)

## Non-Goals

- Do not implement lockstep networking.
- Do not make the client authoritative for validation or gameplay outcomes.
- Do not predict hidden enemy state, fog reveal, damage, kills, resource income, production
  completion, spawned units, completed upgrades, or match outcome.
- Do not add a separate product toggle for the cadence work.
- Do not define success by server tick health alone.
- Do not rely on manual QA as the main correctness check.

## Required Verification Themes

Every phase must add or run the relevant subset of:

- `node tests/prediction_controller.mjs`
- `node tests/sim_wasm_smoke.mjs` when generated WASM assets are present
- `node tests/tri_state/self_test.mjs`
- `node tests/tri_state/run.mjs --scenario <new-or-touched-scenario>`
- `node tests/protocol_parity.mjs` for any protocol changes
- `node scripts/check-prediction-guardrails.mjs`
- `node scripts/check-client-architecture.mjs` for client module changes
- focused Rust tests for room scheduling, protocol DTOs, sim-wasm, and command services
- browser perf harness checks once Phase 7 adds or updates them

Tri-state coverage should prefer scenario artifacts over visual judgment. For each predicted command
family, include at least one healthy two-tick case, one late-arrival case, one rejected/no-op case,
one coalesced or skipped snapshot case, and one prediction-disabled authoritative-only case.
