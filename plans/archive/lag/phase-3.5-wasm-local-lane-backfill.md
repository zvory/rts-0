# Phase 3.5 - WASM Local Lane Backfill

Status: Done.

## Objective

Register the existing `rts-sim-wasm` predictor as the tri-state harness local lane and convert the
current WASM smoke/parity coverage into artifact-backed three-lane scenarios. This phase should
make local prediction state inspectable beside the remote authoritative lane and browser client
lane without expanding the prediction surface.

## Scope

Use the owner-safe prediction facade that already exists. Do not expose full authoritative server
state, hidden enemy internals, replay persistence, AI, SQL, Tokio, Axum, or server-only helpers to
the browser or harness.

## Harness Additions

- Add `WasmLocalLane` implementing the Phase 0.5 `local_lane.mjs` adapter interface.
- Initialize the local lane from the same start payload and player id as the browser client.
- Import owner-safe baselines using the existing WASM baseline import/export path.
- Enqueue the same local command stream with the same `clientSeq` values used by remote/client
  lanes.
- Advance local ticks explicitly from scenario steps.
- Capture `localLaneSummaryJson()`, render snapshots, diagnostics, pending command sequences,
  correction magnitude, unsupported fields, and disabled reasons.
- Diff local summaries against remote/client summaries using domain-aware movement, order-plan,
  pending-command, and correction assertions.

## Required Scenarios

- `local_lane_initializes_from_start`: construct the WASM lane from the match start payload and
  capture a local summary with no baseline imported.
- `local_lane_noop_ticks`: import the first owner-safe baseline, advance local ticks without
  commands, and assert deterministic stable owned state.
- `local_lane_simple_move`: enqueue a move command in all lanes, advance local ticks, and assert
  local owned movement advances while unsupported systems remain explicitly marked.
- `local_lane_queued_move`: enqueue queued movement stages and assert local active and queued order
  summaries match the command stream.
- `owner_safe_baseline_no_hidden_enemy_leak`: construct or capture a situation with hidden enemy
  state and assert local-lane baseline/artifact data does not include hidden ids, positions,
  orders, target ids, economy, or production state.
- `unsupported_command_is_explicit`: send an unsupported command such as construction or combat and
  assert the local lane reports unsupported/disabled reasons rather than silently claiming parity.

## Artifact Requirements

Three-lane artifacts must make unsupported state explicit. A missing local field should be recorded
as `unsupported` or `unknown`, not as a divergent value. This prevents the harness from hiding real
desyncs while still allowing the narrow movement predictor to coexist with authoritative-only
combat, economy, production, construction, and fog.

## Verification

- Run the Phase 0.5 and Phase 2.5 suites.
- Run `node tests/sim_wasm_smoke.mjs` when generated WASM assets are present.
- Run the new three-lane WASM scenarios.
- Keep the native Rust `rts-sim-wasm` tests for baseline safety and deterministic movement; the
  scenario suite should prove integration, not duplicate every pure Rust assertion.

## Manual Testing Focus

Inspect one three-lane artifact and verify the local lane is present, named, and comparable. Confirm
that unsupported fields are easy to distinguish from predicted fields that actually diverged.

## Handoff Expectations

At handoff, include the WASM asset requirement, the command to run one three-lane scenario, and any
baseline fields that Phase 4.5 needs for movement correction scenarios.

## Player-Facing Outcome

No new gameplay behavior. This phase makes the already-built WASM predictor visible in the harness.
