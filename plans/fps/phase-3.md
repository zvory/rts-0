# Phase 3 - Browser Perf Harness

## Phase Status

- [x] Done.

## Objective

Add a repeatable local browser performance harness that runs fixed workloads, collects the Phase 1/2
client performance summaries, and writes artifacts without manual browser or console work. This phase
should provide evidence for future optimization work while avoiding brittle machine-specific FPS gates.

## Work

- Add a script such as `scripts/client-perf-harness.mjs` that drives headless Chrome through Puppeteer
  or the repo's existing browser-test dependency path.
- Start or reuse a local server on an isolated port, following the existing `tests/run-all.sh` pattern
  for `RTS_URL`, `RTS_WS`, `PORT`, and Chrome selection where practical.
- Support fixed workloads:
  - the preserved Matt/Alex replay artifact copied at runtime from
    `docs/network-incident-examples/2026-06-19-beta-matt-alex/match-54-replay.json` into an ignored
    `server/target/selfplay-artifacts/<name>/replay.json` directory, then loaded via
    `/dev/replay-artifact?replay=<name>`;
  - at least one dev scenario or live match workload that stresses visible entities, fog, minimap,
    renderer overlays, or selection/HUD behavior;
  - optional future workload selection via flags without requiring every workload in the default run.
- Collect machine-readable summaries from `window.__rtsPerf`, current `MatchHealth` metrics, relevant
  `ClientNetReport` fields, console/page errors, viewport/device information, and workload metadata.
- Write artifacts under `target/client-perf/<workload>/<timestamp>/`, including summary JSON and
  optional Chrome tracing output when explicitly requested.
- Add a lightweight report mode suitable for local use and optional CI artifacts. The default should
  fail for harness/runtime errors and missing summaries, not for exact FPS or frame-time thresholds.
- Add a documented optional budget/comparison mode only if it is clearly soft or baseline-relative.
  The first version should record evidence for later threshold decisions.
- Document how to run the harness locally and how to interpret the output.

## Expected Touch Points

- new `scripts/client-perf-harness.mjs`
- `tests/package.json` or shared browser dependency setup only if a new dependency is truly needed
- `tests/run-all.sh` only if an opt-in lane or artifact hook is added
- `docs/perf-tracing.md`
- `tests/README.md` if test/harness workflow documentation is useful there
- `docs/network-incident-examples/2026-06-19-beta-matt-alex/README.md` only if replay reuse notes need
  clarification

## Implementation Checklist

- [x] Add the harness script with workload selection and isolated output directories.
- [x] Add Matt/Alex replay workload setup without committing generated `target/` artifacts.
- [x] Add at least one non-replay stress/dev workload.
- [x] Collect and write summary JSON for each workload.
- [x] Make the harness fail on runtime/test errors and missing performance summaries.
- [x] Document local usage and artifact interpretation.
- [x] Mark this phase as done in this file.

## Verification

- `node scripts/client-perf-harness.mjs --list`
- one short local harness run against the replay workload
- one short local harness run against the selected stress/dev workload
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If local Chrome is unavailable, document the exact blocker and run the non-browser validation the
script supports, but do not claim browser-performance verification passed.

## Manual Test Focus

Open one generated artifact directory and confirm the summary names the workload, build, viewport,
entity/context counts, frame timing aggregates, worst phases, console errors, and report-window
metadata. Confirm the replay workload can be opened manually through `/dev/replay-artifact` using the
same generated artifact name.

## Handoff Expectations

Report the artifact paths, the observed local baseline numbers, and whether any soft budget was added.
State clearly that these numbers are machine-local evidence for the next optimization campaign, not a
portable guarantee that all laptops will hit the same FPS.
