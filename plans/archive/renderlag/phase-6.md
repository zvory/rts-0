# Phase 6 - Render Budget Harness And Playbook

## Phase Status

- [x] Done.

## Objective

Make render optimization results repeatable and understandable after the code changes land. Future
agents should be able to run one render-lag comparison path, see whether the client is near the
120 FPS budget, and know which phase labels to inspect without treating machine-local FPS as a hard
CI contract.

## Work

- Extend or document the browser perf harness workflow for render-lag comparisons:
  - Matt/Alex replay workload;
  - vehicle-wall stress workload;
  - at least one selected-unit or HUD-heavy check, scripted if practical;
  - optional Chrome trace capture when a phase needs deeper diagnosis.
- Add advisory budget reporting around the 120 FPS target:
  - total frame budget is 8.33 ms;
  - recurring phase costs above 1-2 ms should be called out;
  - p95 bucket, max, worst-phase count, and shape context should be shown together;
  - warnings should guide humans but should not fail CI on absolute timing.
- Update docs so future investigators know how to separate:
  - top-level `frame.work`;
  - nested renderer subphases;
  - minimap subphase probes if they remain local-only;
  - per-player beta reports versus local replay measurements.
- Preserve artifact hygiene. Detailed traces and timing artifacts should stay under ignored
  `target/client-perf/` directories and should not be committed.
- Add any low-risk harness tests needed for new parser/reporting output, but do not add a brittle
  laptop-specific FPS gate.

## Expected Touch Points

- `scripts/client-perf-harness.mjs`
- optional new script under `scripts/` if comparison output is clearer outside the harness
- `docs/perf-tracing.md`
- `docs/context/testing.md` if workflow guidance changes
- `tests/client_contracts.mjs` or harness-specific Node tests if new report code is pure JS
- `tests/select-suites.mjs` only if new files need suite routing

## Implementation Checklist

- [x] Define the final render-lag comparison workflow and artifact naming.
- [x] Add advisory 120 FPS budget warnings without hard failing on absolute timing.
- [x] Include selected-unit or HUD-heavy coverage if practical.
- [x] Document how to interpret top-level versus nested phase timings.
- [x] Document that Matt/Alex per-player beta FPS and local replay measurements must stay separate.
- [x] Add focused tests for new parser/reporting behavior where practical.
- [x] Run verification and record exact results.
- [x] Mark this phase as done in this file.

## Verification

- `node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 10`
- `node scripts/client-perf-harness.mjs --workload selected-unit-hud-stress --seconds 10`
- any new selected-unit or render-lag comparison command added by this phase
- `node tests/client_contracts.mjs` or focused harness tests for pure JS changes
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Run the documented workflow from a clean checkout and confirm the output is useful to a human:
workloads are named, artifact paths are clear, warnings are advisory, and top-level versus nested
phase costs are not double-counted. Open one generated artifact and confirm it contains the fields
called out by the docs.

## Handoff Expectations

Provide the final commands future agents should run, the artifact locations, and the advisory budget
interpretation. State whether the optimization campaign has remaining phase costs above the 120 FPS
budget and recommend any follow-up plan only from measured evidence.
