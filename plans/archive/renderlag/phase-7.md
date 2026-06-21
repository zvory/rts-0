# Phase 7 - Frame Budget Reframe

## Phase Status

- [x] Done.

## Objective

Reframe render-lag measurement around concrete frame-work budgets instead of a single 120 FPS
advisory. The harness and docs should show whether a workload is under the 60, 120, 240, and 480 FPS
frame-work budgets, and should make clear that local `requestAnimationFrame` FPS is display and
browser dependent.

## Work

- Update render budget reporting so every workload summary includes these advisory targets:
  - 60 FPS: 16.67 ms;
  - 120 FPS: 8.33 ms;
  - 240 FPS: 4.17 ms;
  - 480 FPS: 2.08 ms.
- Keep the 120 FPS result visible, but stop treating it as the final local headroom goal. A workload
  near 8 ms p95 is barely clearing 120 locally and should still be treated as risky for weaker
  hardware.
- Make the browser perf harness console output and `summary.json` rollups show the next missed
  budget and the margin to each budget.
- Document why the repo compares `frame.work` p95/average/max rather than literal local RAF FPS.
- Document how to estimate player impact cautiously from a local FPS ratio, including the caveat
  that a relative improvement is more useful than adding a flat FPS delta.
- Preserve the existing rule that frame timing warnings are advisory and machine-local. Do not add a
  hard CI failure on absolute FPS, browser FPS, or Chrome trace timing.

## Expected Touch Points

- `scripts/client-perf-harness.mjs`
- `docs/perf-tracing.md`
- `tests/client_contracts.mjs` or a focused harness test if pure report helpers are exported
- `tests/README.md` only if command guidance changes

## Implementation Checklist

- [x] Add 60/120/240/480 FPS budget reporting to workload and comparison summaries.
- [x] Update console output so the next missed budget is visible during local runs.
- [x] Document the frame-work budget model and RAF/display-refresh caveat.
- [x] Keep warnings advisory and avoid laptop-specific CI gates.
- [x] Add focused report-shape coverage where practical.
- [x] Run the current render-lag suite and record exact artifact paths.
- [x] Run verification and record exact results.
- [x] Mark this phase as done in this file.

Completed verification on 2026-06-19:

- `node --check scripts/client-perf-harness.mjs` passed.
- `node tests/client_contracts.mjs` passed.
- `node scripts/check-docs-health.mjs` passed.
- `git diff --check` passed.
- `node scripts/client-perf-harness.mjs --render-lag-suite --seconds 10` passed.
  Artifacts:
  - `target/client-perf/matt-alex-replay/2026-06-19T23-57-48-442Z`
  - `target/client-perf/vehicle-wall-stress/2026-06-19T23-58-00-084Z`
  - `target/client-perf/selected-unit-hud-stress/2026-06-19T23-58-11-324Z`
  - `target/client-perf/render-lag-comparison/2026-06-19T23-58-22-564Z/summary.json`

Local Matt/Alex replay result: `frame.work` p95 was 8 ms, clearing the 60 FPS and 120 FPS
frame-work budgets locally while missing the 240 FPS headroom budget by 3.83 ms. This is
machine-local evidence only.

## Verification

- `node scripts/client-perf-harness.mjs --render-lag-suite --seconds 10`
- focused harness/report tests added by this phase
- `node tests/client_contracts.mjs` if report helpers are covered there
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Run the render-lag suite from a clean checkout and inspect one workload `summary.json` plus the
rollup summary. Confirm a human can tell which FPS budgets were cleared, which budget is missed
next, and that top-level `frame.work` is not double-counted with nested renderer or minimap rows.

## Handoff Expectations

Report the exact budget fields added, the final local command, artifact paths, and whether the
current Matt/Alex replay clears 60, 120, 240, or 480 FPS frame-work budgets. State that the result is
local evidence only and name the first budget that still fails.
