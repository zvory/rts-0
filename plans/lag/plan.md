# Hybrid Command Cadence and Rollback Plan

## Purpose

Eliminate player-visible command-response lag by making live commands run through a short,
predictable command cadence instead of variable remote-echo timing, then repairing late delivery
with bounded server rollback. The first rollout target is a two-tick command lead at 30 Hz and a
six-tick rollback window, roughly 200 ms. If a command arrives late but still within that window, the
server should enter a non-reentrant catch-up replay, insert the command at its intended tick when the
replay cursor has not passed it, greedily replay to present, and broadcast corrected authority. If a
command is too old for exact rollback but can still be safely applied at the oldest replayable tick,
the server should prefer that clamped rollback over live-only late execution. The server remains
authoritative; the browser uses owner-safe prediction and replay so local owned-world response starts
on the same effective tick the server is expected to honor.

This plan builds on the existing movement prediction setting and prediction debug surfaces. Do not
add a second player-facing toggle for this effort: the existing "Movement prediction" control and
`rts.prediction.enabled` preference remain the master switch for scheduled-command prediction,
local owned-intent overlays, WASM prediction, and the debug surfaces introduced here. When the
setting is off, the live match must keep the current authoritative-only command path.

## Core Model

- Every sequenced gameplay command carries an intended `executeTick`.
- The client initially schedules commands two ticks ahead of its current server-tick estimate.
- The server queues commands for the requested effective tick when they arrive in time.
- A command that arrives after its requested effective tick but within `ROLLBACK_WINDOW_TICKS = 6`
  is inserted at the intended tick by restoring a recent authoritative state and replaying forward.
- A command that arrives too old for exact rollback first tries clamped rollback: accept the oldest
  safe replayable tick, validate the command against that tick's restored state, mark the result
  `rollbackClamped` or equivalent, replay forward, and raise that player's future command lead.
- If clamped rollback is unsupported for the command family, validation fails at the clamped state,
  required history is missing, or the replay command-count cap is hit, apply at the earliest legal
  authoritative tick, mark late fallback metadata, and raise that player's future command lead. During
  catch-up, earliest legal means the earliest replay tick whose command list has not yet been drained;
  after catch-up exits, ordinary live scheduling applies.
- Rollback is a single catch-up section, not a nested operation. While replay is active, new commands
  are greedily folded into the replay command stream if their accepted tick has not passed the replay
  cursor.
- If a command arrives during replay after its accepted tick has already passed, it is applied at the
  earliest replay tick whose command list has not yet been drained, marked `lateDuringReplay` or
  equivalent in owner-only result metadata, and used to raise that player's future command lead. If
  catch-up has no remaining replay tick, ordinary live scheduling applies.
- The client predicts from the intended effective tick, then imports authoritative snapshots and
  replays pending commands forward to the current display tick instead of visually rewinding to an
  old server pose.
- Server authority still wins for validation, combat, fog, resource income, production completion,
  spawns, upgrades, death, and match outcome.
- Anti-cheat protection against malicious backdating is not a design goal for this plan. The
  priority is that normal games feel responsive for trusted players.

## Cross-Phase Constraints

- Keep the server-authoritative model. Client prediction is display and responsiveness only.
- Keep prediction owner-safe. Do not send hidden enemy ids, hidden positions, hidden orders, target
  ids, enemy economy, or full-world state to support prediction.
- Keep transport scheduling metadata out of `SimCommand`. `clientSeq`, `executeTick`,
  receipt/result status, command lead, and rollback diagnostics belong to the live room/protocol
  layer; the simulation should keep receiving ordinary player-owned `SimCommand`s through the
  `Game` API.
- Keep live rollback mechanics out of `ReplaySession`. Reuse replay ideas by extracting shared
  keyframe/fast-forward helpers where useful, but live match code should own a purpose-built
  rollback history instead of coupling live rooms to replay playback UI state.
- Keep the existing movement prediction setting as the rollout/debug gate. Prediction disabled
  must clear local overlays and preserve monotonic `clientSeq` allocation.
- Start with `commandLeadTicks = 2`; adapt upward only from measured late arrivals, excessive
  correction, or repeated jitter. Decay downward slowly after stable windows.
- Treat late commands as expected under bad networks, not as fatal desyncs. Commands inside the
  rollback window should be retroactively honored when the replay cursor has not passed their
  accepted tick; commands outside the exact rollback window should first try clamped rollback to the
  oldest safe replayable tick. Commands behind the active replay cursor, unsafe for clamped rollback,
  missing deterministic history, or over the replay command-count fuse fall forward to the earliest
  undrained replay tick during catch-up, or to ordinary live scheduling after catch-up exits, and
  adjust future lead.
- Keep the rollback window bounded. The product target is 6 ticks, exactly 200 ms at 30 Hz. This caps
  the authority rewind distance, not wall-clock CPU time. Trust the current server tick speed for the
  initial rollout, record slow catch-up replay timings, and treat optimization as follow-up evidence
  rather than a phase-blocking rollout gate.
- During catch-up replay, the room should not stream intermediate replay frames to clients. It should
  repair authority internally, then resume normal latest-snapshot fanout from the corrected state.
  Prediction-enabled clients may see an authoritative snapshot gap, but should keep locally predicting
  owned-world response from pending effective-tick commands.
- The initial metronome policy is that the replay target is the live tick captured when catch-up
  begins. Wall-clock ticks that elapse while catch-up runs are measured as room tick delay; the first
  rollout does not chase those elapsed ticks in the same catch-up pass. If this creates visible room
  stalls, use diagnostics to decide between optimization, higher lead, reduced rollback support, or a
  smaller window.
- Rollback must be non-reentrant. Once a room enters catch-up replay, it cannot start another
  rollback until it exits catch-up and resumes live ticking.
- Catch-up replay should greedily drain newly arrived commands between replay ticks. If the accepted
  tick is still ahead of or equal to the replay cursor, insert the command into that replay tick's
  deterministic command list. If the accepted tick is behind the cursor, apply it at the earliest
  replay tick whose command list has not yet been drained.
- If catch-up has already replayed through present and there is no remaining replay tick, new
  commands are no longer part of that catch-up pass. They follow ordinary live scheduling: future
  accepted ticks remain queued for that future tick, while already-late commands apply on the next
  live tick.
- Structure catch-up so the room can observe command arrivals between replay ticks; if an
  implementation can only run one opaque synchronous replay block, the phase handoff must document
  that narrower behavior before later phases depend on absorbed-during-replay semantics.
- Add a hard replay command-count fuse, initially `MAX_REPLAY_COMMANDS = 1000`, so command bursts
  cannot grow a catch-up pass without bound. Hitting the fuse falls back to late execution and lead
  adjustment; it should also produce structured diagnostics.
- Rollback may invalidate snapshots, events, audio cues, and visible remote-unit positions already
  observed by clients. Corrected snapshots remain authoritative; clients should smooth corrected
  remote state where possible instead of trying to undo every old visual effect.
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

## Required Architecture Boundaries

The rollback phases should introduce small, named collaborators rather than expanding the room task
or prediction controller into catch-all modules:

- `LiveCommandScheduler` owns live command envelopes, accepted effective ticks, deterministic
  ordering, and draining due commands into `Game::enqueue`.
- `RollbackHistory` owns authoritative keyframes, the applied command stream by tick, catch-up replay
  measurements, replay command-count accounting, and history expiry. A restore for tick `T` starts
  from the post-tick `T - 1` keyframe, with an explicit tick-0 keyframe for commands applied on tick
  1.
- `CommandResultTracker` owns owner-only command result metadata keyed by `(connection_id,
  clientSeq)`, including requested, accepted, applied, late, rollback, fallback, and reason data.
  Snapshot fanout may attach a bounded result list, but gameplay code should not scrape debug
  strings.
- `CommandLeadController` owns per-player lead recommendations, upward adjustment, slow decay, and
  the owner-only lead value exposed to the client.
- Browser cadence logic should be a named prediction-path collaborator, injected through `Match`.
  `PredictionController` may compose it, but cadence estimation, command result ingestion, and
  WASM replay should remain separable so tests can cover each one without a full browser match.

If an implementation cannot keep one of these boundaries, the phase handoff must call that out as a
blocker or deliberately document the narrower substitute API before later phases build on it.

## Phase Summaries

### [Phase 1 - Effective-Tick and Rollback Protocol](phase-1.md)

Add the wire and snapshot contract for scheduled command execution without changing live behavior
yet. Commands gain an intended `executeTick`, and owner-only command result diagnostics record
requested, accepted, applied, late, exact rollback, clamped rollback, and fallback execution ticks.
This phase proves the protocol and logs can explain the command cadence and rollback path before
local prediction depends on it.

### [Phase 2 - Client Command Cadence Controller](phase-2.md)

Build the browser-side command cadence controller behind the existing Movement prediction setting.
The client estimates server tick from snapshots, starts at a two-tick lead, stamps commands with
effective ticks, and records local command timeline diagnostics while still tolerating the old
server path. This phase must prove prediction-off and prediction-on command sequencing stay
monotonic and debuggable.

### [Phase 3 - Server Scheduler and History Buffer](phase-3.md)

Make the room task execute queued player commands on their accepted effective ticks and maintain the
authoritative history required for rollback. This phase should add a rolling state/keyframe and
command-log buffer for at least 6 ticks, plus logs for clone, restore, replay timing, and replay
command-count accounting. It creates the deterministic scheduling and history substrate but may
still fall back to late execution until rollback itself lands.

### [Phase 4 - Bounded Server Rollback](phase-4.md)

Use the history buffer to honor late commands through exact rollback inside the six-tick window and
clamped rollback at the oldest safe replayable tick when exact rollback is too old. The server
restores the nearest safe state, enters a non-reentrant catch-up mode, inserts commands at intended
or clamped ticks when the replay cursor has not passed them, replays greedily to present, and emits
corrected latest snapshots and rollback diagnostics. If the command is not clamp-safe, misses the
history window, or hits the replay command-count fuse, it uses late fallback; if it arrives behind
the active replay cursor, it falls forward to the earliest undrained replay tick during catch-up and
adjusts future lead.

### [Phase 5 - Movement Prediction on Effective Ticks](phase-5.md)

Rework owned-unit movement prediction so local motion starts on the accepted command cadence and
reconciles by replaying forward from authoritative snapshots. Existing move, attack-move, stop,
hold, and queued movement scenarios must be rerun under two-tick, delayed, exact rollback, clamped
rollback, fallback late, bursty, snapshot-gap, and coalesced snapshot profiles. The player-facing
goal is that healthy connections get stable two-tick response, while bursty connections are repaired
by bounded rollback rather than repeated visible snapbacks.

### [Phase 6 - Unit Intent Surfaces](phase-6.md)

Expand local owned-world response for non-movement unit orders: attack target, gather, setup,
teardown, and ability intent posture. These predictions should show accepted owned-unit intent,
pathing/posture, target markers, windups, and queue changes without predicting hidden enemies,
damage, resource income, deaths, or ability outcomes. Each command family needs tri-state accepted,
late, exact rollback, clamped rollback or explicit live-fallback-only, rejected, no-op, and
correction coverage before default enablement.

### [Phase 7 - Building, Rally, Queue, and Build Intent](phase-7.md)

Expand provisional owned-world response for building-facing commands: rally, train, research,
cancel, build intent, and safe progress displays. Existing train/rally optimism should move onto
the same scheduled-command result model, while build intent gets a reversible owner-only local
ghost and only becomes an authoritative scaffold after the server confirms it. This phase must not
predict resource spending, supply changes, spawned units, completed upgrades, or completed
buildings before server snapshots confirm them.

### [Phase 8 - Catch-up Replay and Prediction Observability](phase-8.md)

Instrument bounded rollback and client prediction/replay so slow catch-up passes and frame stalls are
visible without blocking the initial rollout on CPU timing proof. Server-side replay must log replay
distance, command count, clamped rollback, elapsed time, metronome delay, snapshot gaps, and
command-cap fallback; client-side prediction should move or isolate expensive work where needed. This
phase uses server perf traces, the existing frame profiler, net reports, and a repeatable browser
perf harness to keep the command loop observable.

### [Phase 9 - Rollout, Tuning, and Regression Matrix](phase-9.md)

Turn the hybrid cadence and rollback path into the default behavior under the Movement prediction
setting after correctness gates pass and catch-up diagnostics are in place. Lock in thresholds for
the two-tick floor, six-tick rollback window, upward lead adjustment, decay, replay command-count
fuse, clamped fallback, correction budgets, metronome delay, snapshot gaps, and fallback modes. This
phase updates docs, operator playbooks, and tri-state/perf suites so future gameplay work cannot
quietly break command responsiveness.

## Phase Index

1. [Phase 1 - Effective-Tick and Rollback Protocol](phase-1.md)
2. [Phase 2 - Client Command Cadence Controller](phase-2.md)
3. [Phase 3 - Server Scheduler and History Buffer](phase-3.md)
4. [Phase 4 - Bounded Server Rollback](phase-4.md)
5. [Phase 5 - Movement Prediction on Effective Ticks](phase-5.md)
6. [Phase 6 - Unit Intent Surfaces](phase-6.md)
7. [Phase 7 - Building, Rally, Queue, and Build Intent](phase-7.md)
8. [Phase 8 - Catch-up Replay and Prediction Observability](phase-8.md)
9. [Phase 9 - Rollout, Tuning, and Regression Matrix](phase-9.md)

## Non-Goals

- Do not implement lockstep networking.
- Do not make the client authoritative for validation or gameplay outcomes.
- Do not predict hidden enemy state, fog reveal, damage, kills, resource income, production
  completion, spawned units, completed upgrades, or match outcome.
- Do not build anti-cheat or malicious backdating prevention in this plan.
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
- focused Rust tests for room scheduling, rollback replay, protocol DTOs, sim-wasm, and command
  services
- server rollback/catch-up diagnostic checks once Phase 8 adds or updates them
- browser perf harness checks once Phase 8 adds or updates them

Tri-state coverage should prefer scenario artifacts over visual judgment. For each predicted command
family, include at least one healthy two-tick case, one late-arrival exact rollback case, one
outside-window clamped rollback case when the command is clamp-safe, one outside-window late fallback
case when the command is not clamp-safe or history is unavailable, one command that arrives behind
the active replay cursor, one rejected/no-op case, one coalesced or skipped snapshot case, and one
prediction-disabled authoritative-only case.
