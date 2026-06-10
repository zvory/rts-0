# Lag Elimination Plan

## Purpose

Eliminate command-response lag caused by network round trips while keeping the Rust server fully
authoritative. The browser should run a local Rust simulation through WASM for prediction, send the
same player commands to the remote server, and reconcile local prediction against authoritative
server snapshots as they arrive.

This is not a diagnostic plan. The working premise is that network latency is the player-facing
problem and that local prediction is the chosen fix. The plan is staged so the game gains local
responsiveness without accepting desyncs, fog leaks, or untestable client/server divergence.

## Core Model

- The server remains authoritative for match outcome, fog, combat, economy, production, and all
  cross-player interactions.
- The browser runs a local `rts-sim` WASM prediction engine for the current player's experience.
- Client commands are applied immediately to the local predictor and mirrored to the remote server.
- Server snapshots carry enough acknowledgement data for the client to know which local commands
  the server has processed.
- On each authoritative snapshot, the client rewinds or resets prediction to the server-approved
  state, drops acknowledged commands, reapplies unacknowledged local commands, and renders the
  corrected predicted view.
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

## Phase Index

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

Every implementation phase must include programmatic verification in at least one of these forms:

- protocol round-trip tests for Rust and JS mirrors
- deterministic native replay tests
- native-vs-WASM parity tests using the same command streams
- reconciliation tests with delayed, dropped, and coalesced snapshots
- browser smoke tests that assert visible command response before authoritative echo
- fog leak tests that prove prediction baselines expose no hidden enemy state
- performance tests with explicit CPU, memory, bundle-size, and frame-time budgets
