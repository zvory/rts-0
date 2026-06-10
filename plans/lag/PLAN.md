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

## Phase Index

0. [Phase 0 - Tri-State Scenario Harness](phase-0-tri-state-scenario-harness.md)
1. [Phase 1 - Prediction Protocol Contract](phase-1-prediction-protocol-contract.md)
2. [Phase 2 - Client Prediction Buffer and Reconciliation Skeleton](phase-2-client-prediction-buffer.md)
3. [Phase 3 - WASM Simulation Package](phase-3-wasm-simulation-package.md)
4. [Phase 4 - Owned Unit Movement Prediction](phase-4-owned-unit-movement-prediction.md)
5. [Phase 5 - Command Acceptance, Rejection, and UI Optimism](phase-5-command-acceptance-ui-optimism.md)
6. [Phase 6 - Combat, Fog, and Cross-Player Guardrails](phase-6-combat-fog-cross-player-guardrails.md)
7. [Phase 7 - Rollout, Performance Budgets, and Removal of Legacy Delay Paths](phase-7-rollout-performance.md)

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
