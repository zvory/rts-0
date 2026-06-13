# Lag Elimination Plan

## Purpose

Eliminate command-response lag caused by network round trips while keeping the Rust server fully
authoritative. The browser should run a local Rust simulation through WASM for prediction, send the
same player commands to the remote server, and reconcile local prediction against authoritative
server snapshots as they arrive.

This is not primarily a diagnostic plan. The working premise is that network latency is the
player-facing problem and that local prediction is the chosen fix. The plan is staged so the game
gains local responsiveness without accepting desyncs, fog leaks, or untestable client/server
divergence. The testing centerpiece is a tick-steppable tri-state scenario harness: every prediction
behavior should be inspectable as remote authoritative state, local prediction/reference state, and
browser client state over the same scripted clicks and ticks.

## Core Model

- The server remains authoritative for match outcome, fog, combat, economy, production, and all
  cross-player interactions.
- The browser runs a local `rts-sim` WASM prediction engine for the current player's experience.
- Client commands are applied immediately to the local predictor and mirrored to the remote server.
- Server snapshots carry enough acknowledgement data for the client to know which local commands
  the authoritative simulation has consumed. Socket/room receipt may be tracked internally for
  diagnostics, but reconciliation drops pending commands only after sim-consumption acknowledgement.
  That acknowledgement must survive the actual live compact snapshot transport, not only
  object-shaped serde snapshots.
- On each authoritative snapshot, the client rewinds or resets prediction to the server-approved
  state, drops acknowledged commands, reapplies unacknowledged local commands, and renders the
  corrected predicted view.
- Prediction is based on an owner-safe baseline. Fog-filtered render snapshots are not assumed to
  be full simulation state, and hidden enemy state must never be sent to make prediction easier.
- Prediction starts with owned-unit responsiveness only. More systems are enabled only after
  automated replay, parity, reconciliation, and smoke tests prove the previous surface is stable.

## Principles

- Do not ship `rts-server` to browsers. Ship a browser-safe `rts-sim` wrapper crate plus a JS
  adapter.
- Do not let fog-filtered snapshots become a hidden full-world state channel. Any prediction
  baseline sent to clients must be owner-safe.
- Prefer exact native-vs-WASM parity tests over visual judgment.
- Prefer deterministic command streams, replay artifacts, and snapshot checksums over manual play
  testing.
- Keep every phase independently shippable behind a feature flag or disabled-by-default runtime
  switch until the reconciliation loop is proven.
- Never make the client authoritative. Client prediction is a rendering and responsiveness layer;
  rejection by the server must always win.
- Promote every hard-to-debug prediction bug into a named tri-state scenario before fixing it when
  practical, so future agents and humans can replay the same clicks, ticks, snapshots, and diffs.

## Testing Centerpiece: Tri-State Scenarios

All lag/prediction work should orbit a legible scenario harness that can step and inspect three
lanes for the same authored situation:

- Remote authoritative lane: a real server room and WebSocket path, driven by scripted commands and
  explicit tick advancement where dev-mode control is available.
- Local prediction/reference lane: initially a placeholder adapter, later a native `rts-sim`
  reference and the browser-safe WASM predictor from Phase 3.
- Browser client lane: the real client `GameState`, prediction controller state, debug marks, and
  rendered/client-facing snapshot state after each delivered authoritative snapshot.

Phase 0 builds the harness around the lanes that exist today: remote authoritative server behavior
and browser client state. It must define the local-lane adapter contract up front, but it does not
need to fabricate a predictor before the constituent crate exists. Later phases register the native
and WASM local lanes without changing how scenarios are authored.

Each scenario should be both human-inspectable and CI-runnable. A failing run should write an
artifact containing the scenario definition, command/click timeline, authoritative snapshots,
client debug marks, optional local-lane frames, and domain-aware diffs. The goal is that a person or
agent can answer, at step N and tick T: what did the server consume, what did local prediction
believe, what did the browser render, and where did those lanes diverge?

## Phase Summaries

Phase 0 builds the tri-state scenario harness before prediction changes gameplay behavior. It
captures remote authoritative state and browser client state now, while defining the local lane
contract that Phase 3 will fill in. The result is a repeatable way to turn lag and prediction bugs
into inspectable scenario artifacts.

Phase 0.5 backfills the missing harness foundation after later prediction work landed ahead of it.
It turns the Phase 0 design into a concrete Node runner, scenario DSL, lane interfaces, artifact
writer, and first two-lane remote/client scenarios. This phase does not expand prediction; it makes
the already-built netcode inspectable.

Phase 1 adds the protocol contract for sequenced commands and authoritative snapshot
acknowledgements. It keeps gameplay behavior unchanged, but makes every live command correlate with
the simulation tick stream that consumed it. This gives later reconciliation code a reliable signal
for dropping or replaying pending local commands.

Phase 2 adds the client-side prediction buffer and reconciliation skeleton behind disabled runtime
paths. It records pending commands, reads authoritative acknowledgements, and exposes diagnostics
without changing the rendered match by default. This phase proves the browser can safely track the
state needed for prediction before local simulation is enabled.

Phase 2.5 backfills scenario coverage for the Phase 1 and Phase 2 work that already landed. It
adds DSL scenarios for command sequencing, sim-consumption acknowledgements, pending command drops,
stale/coalesced snapshots, and prediction-controller diagnostics in the browser lane. This phase
should be completed before trusting later prediction changes because it proves the ACK lifecycle is
legible in artifacts.

Phase 3 packages the Rust simulation surface for browser-safe local prediction. It defines how an
owner-safe baseline enters the local lane, verifies native and WASM parity, and registers the local
lane with the Phase 0 harness. This phase should still avoid visible prediction unless a developer
flag explicitly turns it on.

Phase 3.5 backfills the tri-state local lane integration for the WASM predictor that already
exists. It wires `rts-sim-wasm` into the scenario runner, records local-lane summaries beside
remote/client summaries, and adds owner-safe baseline leak checks as named scenarios. This phase
converts current WASM smoke/parity coverage into artifact-backed three-lane coverage.

Phase 4 enables owned-unit movement prediction as the first player-visible prediction surface. It
predicts only safe local movement behavior, reconciles against authoritative snapshots, and measures
correction distance under delayed snapshots. The player-facing goal is that move commands feel
immediate while the server remains authoritative.

Phase 4.5 backfills movement-prediction scenarios across realistic network conditions. It adds
delayed, dropped, jittered, burst, and coalesced snapshot profiles, then asserts immediate local
movement, bounded correction, convergence after acknowledgement, and no fog leaks. This phase is
the bridge between the already-shipped movement predictor and the later command/fog rollout work.

Phase 5 expands prediction around command acceptance, rejection, and UI optimism. It makes accepted
commands feel responsive while ensuring server rejection, resource validation, and command failure
notices always win. This phase should improve perceived responsiveness without letting local UI
optimism become authority.

Phase 6 hardens prediction around combat, fog, and cross-player boundaries. It either keeps combat
unpredicted or adds only tightly scoped owner-safe prediction after negative fog and desync tests
exist. The main outcome is confidence that prediction cannot leak hidden state or mask
authoritative combat results.

Phase 7 rolls prediction out under measured performance budgets and removes obsolete delay-oriented
paths only after the earlier gates pass. It verifies CPU, memory, bundle size, frame timing, and
scenario coverage across realistic network profiles. This phase is where prediction becomes the
default player experience if the evidence supports it.

Phase 8 adds conservative local extrapolation for already-started production and research progress.
It only advances progress bars for active queue items that the server has already shown in an
authoritative snapshot, and it never predicts queue acceptance, resource spending, completion, spawn,
or upgrade application. The player-facing goal is to keep safe, already-confirmed timers visually
moving during short snapshot gaps without hiding authoritative correction.

Phase 8.5 evaluates guarded construction-progress extrapolation separately from production timers.
It only considers scaffolds that already exist in authoritative snapshots and must prove that worker
interruptions, deaths, blocked construction, cancellation, and enemy denial snap back cleanly. This
phase may remain design-only or disabled-by-default if the scenarios show construction progress is
too state-dependent to predict safely.

## Phase Index

0. [Phase 0 - Tri-State Scenario Harness](phase-0-tri-state-scenario-harness.md)
0.5. [Phase 0.5 - Harness Foundation Backfill](phase-0.5-harness-foundation-backfill.md)
1. [Phase 1 - Prediction Protocol Contract](phase-1-prediction-protocol-contract.md)
2. [Phase 2 - Client Prediction Buffer and Reconciliation Skeleton](phase-2-client-prediction-buffer.md)
2.5. [Phase 2.5 - Prediction Buffer Scenario Backfill](phase-2.5-prediction-buffer-scenario-backfill.md)
3. [Phase 3 - WASM Simulation Package](phase-3-wasm-simulation-package.md)
3.5. [Phase 3.5 - WASM Local Lane Backfill](phase-3.5-wasm-local-lane-backfill.md)
4. [Phase 4 - Owned Unit Movement Prediction](phase-4-owned-unit-movement-prediction.md)
4.5. [Phase 4.5 - Movement Prediction Scenario Backfill](phase-4.5-movement-prediction-scenario-backfill.md)
5. [Phase 5 - Command Acceptance, Rejection, and UI Optimism](phase-5-command-acceptance-ui-optimism.md)
6. [Phase 6 - Combat, Fog, and Cross-Player Guardrails](phase-6-combat-fog-cross-player-guardrails.md)
7. [Phase 7 - Rollout, Performance Budgets, and Removal of Legacy Delay Paths](phase-7-rollout-performance.md)
8. [Phase 8 - Safe Production and Research Progress Extrapolation](phase-8-safe-production-research-progress.md)
8.5. [Phase 8.5 - Guarded Construction Progress Extrapolation](phase-8.5-guarded-construction-progress.md)

## Current Implementation Status

Archived. Every phase is marked done in its phase document. The final shipped scope keeps the
server authoritative, enables compatible owned-unit prediction with compatibility/performance
fallbacks, and leaves unsafe broader prediction surfaces authoritative-only.

## Non-Goals

- Do not redesign the renderer, HUD, or input model except where prediction requires a new seam.
- Do not add lockstep networking. This codebase is server-authoritative snapshot replication, and
  the plan keeps that model.
- Do not predict hidden enemies, hidden fog reveal, enemy commands, or unrevealed combat outcomes.
- Do not depend on manual multiplayer feel tests as the main safety check.
- Do not optimize by lowering server authority or accepting client-side anti-cheat risk.

## Required Verification Themes

Every implementation phase must either add or run relevant tri-state scenarios, then include
programmatic verification in at least one of these forms:

- tri-state scenario checks comparing remote authoritative state, local prediction/reference state
  when available, and browser client state over scripted clicks/ticks
- protocol round-trip tests for Rust and JS mirrors
- deterministic native replay tests
- native-vs-WASM parity tests using the same command streams
- reconciliation tests with delayed, dropped, and coalesced snapshots
- browser smoke tests that assert visible command response before authoritative echo
- fog leak tests that prove prediction baselines expose no hidden enemy state
- performance tests with explicit CPU, memory, bundle-size, and frame-time budgets

## Implementation and Handoff Rules

Implement one phase at a time. Each phase should be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit.

After implementing each phase, the agent must provide a handoff message for the next agent. The
handoff must summarize what changed, list verification commands and results, identify the next
phase or follow-up work, and name the core features that should be manually tested. Manual testing
notes should cover the changed player-facing surface and core prediction behavior, not an
exhaustive matrix.
