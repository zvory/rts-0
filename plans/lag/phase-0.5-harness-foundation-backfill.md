# Phase 0.5 - Harness Foundation Backfill

## Objective

Build the missing scenario harness foundation around the code that exists today. This phase should
not expand prediction behavior. It should give agents and humans a repeatable way to author,
execute, inspect, and commit regression scenarios for lag and prediction bugs.

## Current Context

Phase 0 was designed as the first implementation phase, but later protocol, prediction-controller,
WASM, and movement-prediction work landed before the runner existed. The codebase already has useful
pieces: live WebSocket tests, headless Chrome smoke tests, dev scenario rooms with pause/step
controls, `window.__rtsPredictionDebug`, and `rts-sim-wasm` local summaries. This phase creates the
orchestration layer that ties those pieces together.

## Harness Shape

Create a small Node-based harness under `tests/tri_state/`:

- `run.mjs`: CLI entry point for one scenario, a scenario glob, or the CI-safe default set.
- `scenarios/`: reviewable DSL scenario files.
- `lanes/remote_lane.mjs`: server/WebSocket controller for lobby, command, snapshot, and tick
  capture.
- `lanes/client_lane.mjs`: Puppeteer controller for real browser state, debug summaries, and
  optional screenshots.
- `lanes/local_lane.mjs`: adapter interface plus an unavailable stub used before Phase 3.5.
- `diffs.mjs`: domain-aware diff helpers.
- `artifacts.mjs`: stable artifact writer.

Keep the first implementation intentionally small. The runner should support one browser client and
one remote authoritative room before it supports multiplayer interleavings.

## Scenario DSL

Define scenarios as ES modules rather than inventing a parser:

```js
export default scenario("remote_client_basic_move", {
  setup: {
    kind: "liveRoom",
    players: 1,
    prediction: "disabled",
    quickstart: true,
  },
  network: { mode: "direct" },
  steps: [
    selectOwn("worker", 0),
    issue("move", { dx: 160, dy: 0 }),
    waitForSnapshot({ minTickDelta: 1 }),
    capture("after-authoritative-move"),
    assertRemoteClientOwnedPosition({ unit: "worker", tolerancePx: 1 }),
  ],
});
```

The DSL should support command-level steps first. Add pointer/click steps only when a scenario
depends on input routing, placement, minimap, selection, or HUD behavior.

## Required Foundation Scenarios

- `remote_client_basic_move`: issue one owned move command and assert the client follows the
  authoritative remote lane after snapshots arrive.
- `queued_order_visibility`: issue queued move or attack-move stages and assert owner-only order
  plan summaries match between remote and client lanes.
- `dev_scenario_step_tick`: open one game-backed dev scenario, pause it, step one authoritative
  tick at a time, and assert artifact ticks advance exactly once per step.
- `forced_failure_artifact`: intentionally fail a tiny assertion and prove a readable artifact is
  written.

## Artifact Contract

Write artifacts under:

```text
server/target/tri-state-scenarios/<scenario>/<run-id>/
```

Each run should include:

- `scenario.json`: resolved setup, network profile, and script.
- `timeline.jsonl`: ordered script operations and captures.
- `remote.jsonl`: authoritative snapshots, ACK summaries, notices, and net status.
- `client.jsonl`: browser `GameState` summaries, selection, command target, prediction debug, and
  visible entity summaries.
- `local.jsonl`: stub frames with `localLane: "unavailable"` until Phase 3.5.
- `diffs.jsonl`: domain-aware diffs.
- `summary.md`: first failure, final status, and reproduction command.

Artifacts must be compact enough for CI logs to summarize, but detailed enough that an agent can
answer what the server consumed, what the browser rendered, and where the lanes diverged.

## Verification

- Node unit tests for the DSL helpers, timeline executor, and artifact writer.
- One live-server plus headless-browser scenario in the local gate or a clearly named optional
  suite if runtime is initially too high.
- Forced-failure artifact test.
- Local-lane unavailable stub contract test.
- Documentation in `tests/README.md` explaining how to run one scenario and how to add a new
  regression.

## Manual Testing Focus

Run one successful scenario and one forced-failure scenario. Inspect `summary.md`, `timeline.jsonl`,
and the lane files to confirm the artifact is understandable without reading the harness source.

## Handoff Expectations

At handoff, include the exact scenario command, artifact path, whether the live scenario is in the
default gate, and the first missing lane or network feature that Phase 2.5 should add.

## Player-Facing Outcome

No gameplay change. This phase turns existing lag and prediction behavior into inspectable
regression artifacts.
