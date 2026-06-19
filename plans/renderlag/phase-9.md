# Phase 9 - Harsher Render Stress Matrix

## Phase Status

- [ ] Pending.

## Objective

Make local render benchmarking harsh enough to stand in for unavailable low-end machines. The suite
should vary workload, CPU throttle, viewport, and device pixel ratio so future optimization work can
target the first measured bottleneck rather than guessing from a normal laptop run.

## Work

- Extend the browser perf harness with a stress-matrix mode that can run repeated samples across:
  - Matt/Alex replay;
  - vehicle-wall stress;
  - selected-unit HUD stress;
  - at least one new fog/combat-heavy workload with many visible units, projectiles/effects, fog
    edges, selection overlays, and minimap activity.
- Add Puppeteer/CDP CPU throttling controls for local-only runs, such as 1x, 2x, and 4x. Report the
  throttle factor in every workload artifact and rollup.
- Add viewport and device-pixel-ratio variation. At minimum, support a small/default/large viewport
  matrix and an explicit DPR/device-scale-factor override where Chrome permits it.
- Run each matrix cell enough times to make p95/average movement believable. Default to a small
  repeat count for local ergonomics and make a longer comparison command available for serious
  before/after work.
- Summarize each cell against the 60/120/240/480 frame-work budgets and rank the first failing
  workload/configuration by top measured phase.
- Keep traces opt-in. Matrix summaries should remain JSON/markdown-sized unless a human explicitly
  passes `--trace`.
- Document the recommended "local low-end substitute" command and how to interpret CPU/DPR stress
  without claiming it exactly models Matt's hardware.

## Expected Touch Points

- `scripts/client-perf-harness.mjs`
- optional pure helper under `scripts/` if matrix summarization grows too large
- `server/crates/sim/src/game/setup/dev_scenarios.rs` if a new dev scenario is needed
- `client/src/dev_scenarios.js` or related client scenario index files if URL routing changes
- `docs/perf-tracing.md`
- `docs/context/testing.md` if workflow guidance changes
- focused harness tests for matrix parsing and summary output

## Implementation Checklist

- [ ] Add stress-matrix CLI options for workload sets, repeat count, CPU throttle, viewport, and DPR.
- [ ] Add or reuse a fog/combat-heavy workload that exercises renderer, fog, minimap, effects, and
      selection overlays.
- [ ] Include matrix configuration in every artifact and rollup.
- [ ] Rank failing cells by missed budget and top measured phase.
- [ ] Keep trace capture opt-in and artifacts under ignored `target/client-perf/` paths.
- [ ] Add focused parser/report tests.
- [ ] Run a short matrix locally and record exact artifact paths.
- [ ] Run verification and record exact results.
- [ ] Mark this phase as done in this file.

## Verification

- short stress matrix command added by this phase
- longer documented command may be attempted but should not be required if it is too expensive
- focused harness/report tests added by this phase
- `node scripts/client-perf-harness.mjs --render-lag-suite --seconds 10`
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If a new dev scenario touches simulation setup, also run the focused Rust or Node scenario coverage
selected by those files.

## Manual Test Focus

Run the short stress matrix and inspect the rollup. Confirm each workload/configuration is named,
CPU throttle and DPR are visible, budget failures are advisory, and the top measured phase points to
an actionable subsystem rather than a vague "slow frame" label.

## Handoff Expectations

Provide the exact short and long matrix commands, artifact paths, the first failing budget/config,
and the top measured phase for that failure. Recommend the next optimization phase only from the
matrix evidence, and explicitly state when the diagnostics are still insufficient.
