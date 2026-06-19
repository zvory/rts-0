# Phase 2 - Client Command Cadence Controller

## Phase Status

- [ ] Planned.

## Objective

Build the browser-side command cadence controller behind the existing Movement prediction setting.
The client should estimate authoritative server tick, start commands at a two-tick lead, stamp
commands with intended effective ticks, and expose diagnostics without requiring server scheduling
to be enabled yet.

## Scope

- Add a small cadence controller owned by the prediction path.
- Use recent authoritative snapshots to estimate current server tick.
- Start every compatible live active player at `commandLeadTicks = 2`.
- Stamp outgoing gameplay commands with `executeTick = estimatedServerTick + commandLeadTicks`.
- Keep the existing `rts.prediction.enabled` setting and gear-menu "Movement prediction" control as
  the only runtime/debug gate.
- When Movement prediction is disabled:
  - do not stamp commands with predictive execute ticks unless the protocol requires a neutral
    default
  - clear prediction overlays
  - preserve current authoritative-only behavior
  - keep `clientSeq` monotonic
- Add debug output for lead ticks, estimated server tick, command issue time, intended execute tick,
  and pending command age.

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
  - missing snapshots before the first estimate
  - disabled setting preserving authoritative-only command behavior
- Add tri-state scenarios for:
  - `cadence_two_tick_stamp`
  - `cadence_prediction_disabled_authoritative_only`
  - `cadence_toggle_preserves_client_seq`
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
where diagnostics are exposed, and what behavior remains stubbed until server scheduling lands.
