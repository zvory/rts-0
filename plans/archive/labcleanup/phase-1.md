# Phase 1 - Behavioral Canary and Reliable Gate

## Phase Status

- [x] Done.

## Objective

Make the existing real-browser CLI smoke the trusted behavioral canary for the cleanup. It should
prove a small scene can be opened, changed, observed, captured as a valid image, and torn down without
freezing the entire pre-alpha interface or turning into a slow exhaustive suite.

## Work

### Strengthen the existing live smoke

- Extend `tests/interact_cli_smoke.mjs`; do not add a second end-to-end harness.
- Keep the scenario small: one shooter and one target with aliases, not a large unit population.
- Exercise this semantic flow through ordinary CLI invocations:
  1. start from shutdown and open an isolated session;
  2. query `status`/`catalog`, use `time` to pause, and spawn the two entities;
  3. inspect and update an entity, then confirm the observable change;
  4. issue one order and use `time` to step enough authoritative ticks to observe a valid result;
  5. focus the camera and capture a clean screenshot;
  6. fetch the returned Tailnet preview and prove it is a nontrivial PNG with the requested dimensions;
  7. export the setup, remove an entity, import the setup, and confirm the scene/aliases are restored;
  8. retain one short live H.264 recording canary;
  9. reset, close, prove a stale session is rejected, and shut the daemon down.
- Assert stable meaning rather than representation: success/failure class, changed entity facts,
  clean readiness/render diagnostics, content type/signature/size/dimensions, restored aliases, and
  complete teardown. Do not assert full response snapshots, exact ids, exact ticks, or pixel values.
- Move the existing 36-extra-unit/truncation stress from the live smoke into fake-driver contract
  coverage if equivalent bounded-list coverage is not already present.

### Make the test gate honest and isolated

- Give `tests/interact_fixed_capture_contracts.mjs` its own named/tracked `run_suite_bg` entry in
  `tests/run-all.sh`. The current bare invocation can fail without being included in the aggregated
  runner result.
- Stop Lab artifact tests from recursively deleting the shared
  `target/interact/lab/artifacts` directory. Track every generated artifact/sidecar/setup/replay path,
  clean only test-owned files in `finally`, and remove a directory only if the test owns it or it is
  empty.
- Make fake CLI contracts clean up their own exported setup and replay outputs, including failure
  paths. UUID filenames remain sufficient isolation; do not add a production artifact-root framework.
- Keep the live CLI smoke in the existing browser smoke shard, where CI can reuse the gate's private
  server for speed. Keep standalone driver smoke as a focused/manual check rather than duplicating
  browser/server startup in the main gate.

### Select the right checks

- Add explicit Interact contract and smoke selections in `tests/select-suites.mjs` for changes to:
  - `scripts/interact/**`;
  - `client/src/interact_bridge.js`;
  - the Rust Lab artifact bridge;
  - Interact tests, docs, and project-local skill instructions where applicable.
- Add selector self-test cases proving representative `driver` and `command_service` changes select
  both Lab contracts and the browser smoke.
- Document the canonical fast contract command, the live browser canary, its server ownership modes,
  dependencies such as Chrome/FFmpeg, and expected cleanup.

## Expected Touch Points

- `tests/interact_cli_smoke.mjs`
- `tests/interact_cli_contracts.mjs`
- `tests/interact_artifact_contracts.mjs`
- `tests/interact_fixed_capture_contracts.mjs`
- `tests/run-all.sh`
- `tests/select-suites.mjs`
- `tests/README.md`
- `docs/context/testing.md`
- `docs/design/testing.md`
- `docs/interact-cli.md`

## Implementation Checklist

- [x] Reshape the existing smoke into the small semantic workflow above.
- [x] Prove update/remove/setup round-trip behavior without exact response snapshots.
- [x] Preserve valid PNG preview and short recording assertions.
- [x] Move the large-unit stress assertion to focused contract coverage.
- [x] Track fixed-capture contract failure in the aggregate runner.
- [x] Make artifact/setup/replay cleanup test-owned and failure-safe.
- [x] Add Lab-specific suite selections and selector self-tests.
- [x] Update the testing runbook and CLI documentation.
- [x] Mark this phase done in this file in the implementation commit.

## Verification

Run the smallest focused set that proves the changed plumbing and canary:

```bash
bash -n tests/run-all.sh
node tests/select-suites.mjs --verify
node tests/interact_cli_contracts.mjs
node tests/interact_artifact_contracts.mjs
node tests/interact_bulk_contracts.mjs
node tests/interact_recording_contracts.mjs
node tests/interact_fixed_capture_contracts.mjs
node tests/interact_tailnet_preview_contracts.mjs
node tests/interact_cli_smoke.mjs
node scripts/check-docs-health.mjs
git diff --check
```

During implementation, deliberately make the fixed-capture contract fail once and confirm the
aggregate runner records the named failure, then restore it. In the final successful run, confirm no
daemon PID, socket, runtime directory, or test-owned artifact remains.

If the local browser binary uses a different path, set the repository's documented `CHROME` value.
The CI-equivalent reused-server path may instead be exercised with the documented browser smoke
shard; do not start two redundant full servers merely to satisfy both forms.

## Acceptance Criteria

- One existing live smoke, not two, proves the core command-and-artifact workflow.
- The preview response is a valid nontrivial PNG and the short recording remains a valid media
  artifact; no golden pixels or long media run is required.
- Update, remove, and setup export/import are observed through real CLI calls.
- Fixed-capture contract failures cannot be lost by `tests/run-all.sh`.
- Concurrent Lab contracts cannot recursively delete one another's artifacts.
- Relevant Lab source changes select focused contracts and browser smoke explicitly.
- Success and failure paths leave no daemon or owned output behind.

## Manual Test Focus

Run `open`, `spawn`, `update`, `inspect`, one `order`, and `screenshot` from separate CLI invocations.
Open the returned Tailnet preview once and confirm it shows the intended two-entity scene, then run
`close` and `shutdown` and confirm the daemon process/socket are gone. (`status` is not a teardown
probe because invoking an ordinary CLI command may start the daemon.)

## Non-Goals

- Exact public-interface snapshots or backward-compatibility guarantees.
- Golden images, visual thresholds, deterministic fixed capture, or cancellation/long-duration media
  canaries.
- Every command/error permutation, load testing, crash injection, performance certification, or a
  browser/GPU/OS matrix.
- Rewriting the tests around `node:test` or introducing a new general-purpose test framework.

## Handoff Expectations

Report the final smoke sequence, its typical duration, the exact focused/gate commands that passed,
and the artifact/daemon cleanup guarantees added. Tell the Phase 2 agent to keep this canary semantic
while moving internals, and name the open/spawn/update/order/screenshot/preview/shutdown workflow for
manual re-testing.
