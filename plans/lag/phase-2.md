# Phase 2 - Client Command Cadence Controller

## Phase Status

- [ ] Planned.

## Objective

Build the browser-side command cadence controller behind the existing Movement prediction setting.
The client should estimate authoritative server tick, start commands at a two-tick lead, stamp
commands with intended effective ticks, and expose rollback-aware diagnostics without requiring
server scheduling or rollback to be enabled yet.

## Scope

- Add a small `CommandCadenceController` owned by the prediction path and injected through
  `Match`/`PredictionController`; do not let input, HUD, minimap, or transport modules compute
  `executeTick` themselves.
- Use recent authoritative snapshots to estimate current server tick.
  - Base the estimate on the latest accepted authoritative snapshot tick plus elapsed local time
    converted to 30 Hz ticks.
  - Clamp the estimate so it never moves backward and never jumps far ahead of the latest snapshot
    without an explicit diagnostic reason.
  - Before the first snapshot estimate, either omit `executeTick` or use the documented neutral
    behavior from Phase 1; do not guess from wall-clock alone.
- Start every compatible live active player at `commandLeadTicks = 2`.
- Stamp outgoing gameplay commands with `executeTick = estimatedServerTick + commandLeadTicks`.
- Keep the existing `rts.prediction.enabled` setting and gear-menu "Movement prediction" control as
  the only runtime/debug gate.
- When Movement prediction is disabled:
  - omit predictive execute ticks unless Phase 1 chose an explicit neutral value
  - clear prediction overlays
  - preserve current authoritative-only behavior
  - keep `clientSeq` monotonic
- Add a command result ingester that records Phase 1 owner-only result entries by `clientSeq`
  without using them to reconcile movement yet.
- Add debug output for lead ticks, estimated server tick, command issue time, intended execute tick,
  rollback eligibility window, server result status, result reason code, result age, and pending
  command age.

## Expected Touch Points

- `client/src/prediction_controller.js`
- `client/src/prediction_settings.js`
- `client/src/match.js`
- `client/src/settings_panels.js`
- `client/src/net.js`
- `tests/prediction_controller.mjs`
- `tests/tri_state/lanes/client_lane.mjs`
- `tests/tri_state/dsl.mjs`

## Verification

- Add prediction-controller tests for:
  - two-tick default lead
  - monotonic `clientSeq` with prediction toggled off/on
  - execute tick stamping based on latest server tick
  - pre-first-snapshot neutral behavior
  - clamped tick estimation after stale, duplicate, skipped, and burst-delivered snapshots
  - rollback-window display for commands whose intended tick is still within 6 ticks
  - Phase 1 command result entries attaching to the matching pending command by `clientSeq`
  - disabled setting preserving authoritative-only command behavior
- Add tri-state scenarios for:
  - `cadence_two_tick_stamp`
  - `cadence_prediction_disabled_authoritative_only`
  - `cadence_toggle_preserves_client_seq`
  - `cadence_result_metadata_is_diagnostic_only`
- Run:
  - `node tests/prediction_controller.mjs`
  - `node tests/tri_state/self_test.mjs`
  - focused new tri-state scenarios
  - `node scripts/check-client-architecture.mjs`

## Manual Testing Focus

Use the settings gear to toggle Movement prediction during a local match. Commands should keep
sending normally, the debug object should show cadence state only when enabled, and turning the
setting off should remove provisional overlays without corrupting selection or command sequences.

## Handoff Expectations

The handoff must state how server tick is estimated, how the initial two-tick lead is represented,
where rollback-aware diagnostics are exposed, how result entries are stored by `clientSeq`, what the
disabled/pre-first-snapshot behavior is, and what behavior remains stubbed until server scheduling
and rollback land.
