# Phase 4: Debug Overlays and Artifacts

## Objective

Add optional debugging features after live scenario viewing is stable. This phase is intentionally
deferred because overlays and artifacts are useful only once the basic viewer is already part of the
workflow.

## Pathfinding Overlays

Optional scout-car corridor overlays:

- goal point;
- exit-clear threshold line;
- recent trail per scout car;
- next waypoint;
- current path goal;
- stuck/no-progress counters;
- static-blocked/repath/reverse-recovery counters.

Keep overlays dev-only. Do not add them to normal gameplay snapshots unless the client explicitly
enters a dev scenario/debug mode.

## Artifact Recording

Add optional scenario artifact recording after live viewing works:

```text
server/target/scenario-artifacts/<scenario_id>/<variant>/start.json
server/target/scenario-artifacts/<scenario_id>/<variant>/frames.json
server/target/scenario-artifacts/<scenario_id>/<variant>/summary.log
```

Artifacts should record client-facing frames first. Command-log artifacts are useful only for
game-backed scenarios whose setup can also be reconstructed.

## Replay Use

The artifact path should support sharing a failing local run without rerunning the simulation. If
possible, reuse the existing self-play replay inspection conventions so developers can open an
artifact with a local server and macOS `open`.

## Done

- Overlay data is available only in dev scenario/debug mode.
- Scenario artifacts can be recorded without changing normal match snapshots.
- Artifacts are readable enough to inspect or replay a failing run.

## Verification

- `cd server && cargo test`
- `node tests/regression.mjs`
- Manual artifact record and replay/open flow for at least one scenario.
