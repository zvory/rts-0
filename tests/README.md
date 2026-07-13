# Tests

## Run everything (one command)

`run-all.sh` builds the server in debug (overflow checks **on**, which the hardening regression
tests rely on), boots it, polls `GET /` until healthy, runs Rust lint/fast scripted
tests and all the live-server suites, tears the server down, and exits non-zero if **any** suite
fails. The private server runs with `RTS_TEST_TICK_MS=5` by default, so live-server tests wait on
simulated progress instead of real-time 30 Hz wall clock; normal `cargo run` remains 30 Hz.
If a server is already answering on the port it is reused and left running.

This command is the portable local full gate. GitHub Actions runs the same coverage as split
parallel jobs and keeps the required aggregate PR check named `./tests/run-all.sh`. Run focused local
verification for the files or contracts you changed, then rely on that aggregate check before merge.
The CI changed-file classifier keeps docs-only PRs and post-merge `main` pushes cheap, and lets
conservative client-only PRs and pushes skip Rust nextest, lint, and Rust architecture work
while still running server-build, live Node, and browser coverage. Contract-adjacent client files
fall back to the full gate. The CI
Rust/architecture job installs `cargo-nextest` and invokes `tests/run-all.sh --only-rust`, so the
required Rust path is the same nextest-backed runner developers use locally.
Run `./scripts/install-hooks.sh` once per
checkout to install the tracked hooks locally; those hooks run cheap staged-diff checks and
`node scripts/check-docs-health.mjs` instead of the full suite. Normal `main` updates go through
owned PRs with auto-merge armed.
The `Main test gate` workflow cancels superseded runs for the same PR and cancels stale post-merge
`main` push runs when a newer `main` push starts. Canceled stale `main` runs do not trigger beta
deploys; only a successful push run for `main` can do that.

```bash
tests/run-all.sh                 # local gate: cargo nextest + clippy + API suites + client smoke
tests/run-all.sh --full-ai       # local gate plus long AI self-play/simulation coverage
tests/run-all.sh --no-rust       # skip Rust test/lint
tests/run-all.sh --no-client     # skip the headless-browser smoke test
tests/run-all.sh --only-rust     # architecture policy + Rust test/lint only
tests/run-all.sh --only-live-node # JS contracts + live Node API suites only
tests/run-all.sh --only-browser  # browser smoke + configured tri-state browser suites only
tests/run-all.sh --with-tri-state-browser  # include latency-sensitive tri-state browser scenarios locally
tests/run-all.sh -v              # also print headers and pass/fail lines; timing summary is always printed
PORT=8090 tests/run-all.sh       # use a different port
CARGO_TARGET_DIR=/tmp/rts-target tests/run-all.sh  # override the per-worktree Cargo target dir
RTS_SERVER_BIN=/tmp/rts-server tests/run-all.sh --only-live-node  # reuse a prebuilt server
RTS_NODE_DEPS_CACHE_DIR=/tmp/rts-node-deps tests/run-all.sh  # override shared Node deps cache
RTS_RUN_TRI_STATE_BROWSER=1 tests/run-all.sh  # env-form local opt-in for tri-state browser scenarios
CHROME=/path/to/chrome tests/run-all.sh
```

`run-all.sh` prints a final timing summary even in the default quiet mode, including server
build/boot, each background suite, browser scenario groups, and client dependency hydration when it
runs. Rust tests inside `run-all.sh` use `cargo nextest run --config-file .config/nextest.toml
--manifest-path server/Cargo.toml --profile default`. Install nextest with
`cargo install cargo-nextest --locked`; if it is missing, the local Rust gate fails with that
install hint instead of falling back to Cargo's built-in test runner.
The workspace currently has no Rust doctests, so `run-all.sh` does not run a separate
`cargo test --doc` step.

For slow Rust runs, start with the context and timing already printed by the runner. The Rust-only
path prints the Cargo target dir, Rust version, cargo version, cargo-nextest version, nextest
per-test status/slow-test output, and the final command-level timing summary. In CI, the Rust job
also prints the Cargo cache exact-hit result from Actions, shell timing details for the Rust
top-level suites, and a slowest-testcase summary from nextest's JUnit XML. It also uploads
`server/target/nextest/default/junit.xml` as the `nextest-junit` artifact with 7-day retention when
the Rust lane runs. Use that plus the target dir to decide whether time is going into rebuilds, then
use nextest's slow-test output and the JUnit summary to narrow test runtime.

The client smoke test self-skips (not a failure) only when a Chrome binary is missing. When Chrome
is available, `run-all.sh` hydrates `puppeteer-core` into a shared dependency cache keyed by the
SHA-256 hash of `tests/package-lock.json`, then links this worktree's ignored `tests/node_modules`
to the matching cache entry. Hydration uses `npm ci`, so a lockfile/package mismatch fails the
gate instead of silently reusing the wrong dependency tree.

## Lab Interact canary

The canonical fast Lab Interact check is the focused contract set below. It uses the fake driver,
needs no Rust server or Chrome, and requires FFmpeg/ffprobe with H.264 support for its media checks:

```bash
node tests/lab_interact_cli_contracts.mjs
node tests/lab_interact_artifact_contracts.mjs
node tests/lab_interact_bulk_contracts.mjs
node tests/lab_interact_recording_contracts.mjs
node tests/lab_interact_fixed_capture_contracts.mjs
node tests/lab_interact_tailnet_preview_contracts.mjs
```

Run `node tests/lab_interact_cli_smoke.mjs` for the live browser canary. A standalone run needs
Chrome/Chromium and owns a private Rust server; `tests/run-all.sh --only-browser-scenarios=smoke`
reuses the gate's private server via `RTS_LAB_INTERACT_BASE_URL`. The canary authors only a shooter
and target, verifies update and authoritative stepped movement, fetches a clean 1000x700 PNG
preview, round-trips a setup and aliases, records a short H.264 clip, and proves reset/close/stale
session/shutdown cleanup. Tests remove only their UUID-owned session directories and exact portable
artifact/sidecar files, leaving concurrent Lab output untouched.

By default, the local gate and Cargo helper use an isolated target directory for each worktree
under `/tmp/rts-cargo-target/`. This keeps final binaries, test harnesses, and self-play
artifacts branch-local while keeping the checkout clean. Override with
`CARGO_TARGET_DIR=/path/to/target` when you need a specific target location. CI intentionally sets
`CARGO_TARGET_DIR` to `server/target` in Rust-building `Main test gate` jobs so the existing Cargo
cache restores the same target directory that Cargo uses during the run. The workflow builds the
debug server once, uploads it as an artifact, and passes it into live Node and browser jobs with
`RTS_SERVER_BIN` so those split jobs do not rebuild the server independently.

Installed repo hooks run `scripts/cleanup-worktrees.sh --auto` after commits and merges on `main`.
Auto cleanup removes only clean `zvorygin/*` worktrees whose branch head is reachable from local
`main` or `origin/main`, their matching target dirs, and a small bounded number of old target dirs
that do not map to any active worktree. It tolerates GitHub auto-deleting merged remote branches and
keeps dirty worktrees or unmerged heads. Use `scripts/cleanup-worktrees.sh --dry-run` to inspect
what would be removed. `node tests/wait_pr.mjs` verifies that the PR waiter fast-forwards the real
local `main` worktree, preserves unrelated working-tree files, invokes cleanup, and refuses a
divergent update.

We tested `sccache` as the cross-worktree Rust reuse layer and do not enable it automatically.
It cached Rust outputs when rebuilding the same target directory path, but produced 0% Rust cache
hits across different per-worktree target directories because Cargo passes target-dir-specific
`--out-dir`, `-L dependency=...`, and `--extern ...` paths into rustc. That made the cache keys
different for each worktree. Keep per-worktree target dirs for correctness; do not restore one
shared Cargo target dir just to recover reuse.

For scripts that need to print the default target dir or for explicit Cargo wrapper usage:

```bash
scripts/cargo-shared-target.sh test --manifest-path server/Cargo.toml self_play -- --nocapture
```

Everything below documents the individual suites the runner orchestrates.

---

The suites run against a **live server**. To run one on its own, start the server first:

```bash
cd server && cargo run        # serves the client + websocket on the configured RTS_ADDR
```

## Server integration (no dependencies)

Drives two WebSocket clients through the whole lifecycle — lobby, host/colors, ready,
start (map + per-player payload), starting economy, **fog of war**, gathering, training,
and disconnect → win — asserting the authoritative pipeline end to end. Uses Node's
built-in global `WebSocket` (Node ≥ 22), so there is nothing to install.

```bash
node tests/server_integration.mjs
# override endpoint: RTS_WS=ws://host:port/ws node tests/server_integration.mjs
```

## Team integration harness baseline (no dependencies)

Drives a live room through multi-client setup, custom team-slot assignment, AI seating/removal,
readiness, match start, snapshot waits, and game-over waits using shared helpers in
`tests/team_harness.mjs`. The suite does not start its own server; start one first or use
`tests/run-all.sh`, which boots a private server and sets `RTS_WS`.

```bash
node tests/team_integration.mjs
# override endpoint: RTS_WS=ws://host:port/ws node tests/team_integration.mjs
```

## Headless simulation self-play

Runs inside the Rust test suite with no live server. Plain `cargo nextest run` runs the fast
scripted self-play coverage. Long profile-backed matchups, long AI simulation checks, and the full
real-AI match are gated behind `RTS_FULL_AI_TESTS=1` because they dominate wall-clock time.
The tests create scripted API clients that drive `Game` through `enqueue`/`tick`/`snapshot_for`,
exercising gathering, oil, Depot/Barracks/Vehicle Works construction, Rifleman/Scout Car/Tank
training, rush pressure, and combat. Successful runs replay the authoritative tick-stamped command
log through a fresh game and compare the replayed event stream and final snapshots against the live
run. On failure it writes normal `ReplayArtifactV1` replay artifacts under the Cargo target dir's
`selfplay-failures/` directory, plus self-play diagnostics beside them. To save a successful run
too, set `RTS_SELFPLAY_SAVE_REPLAY` to either `1` for an auto-generated artifact name or to an
explicit safe artifact name; successful runs are then written under the target dir's
`selfplay-artifacts/<name>/`.
When you open a replay artifact in the browser, use the server instance that produced it, or
start a fresh one on its own port before loading `/?replayArtifact=<artifact_name>`. The older
`/dev/replay-artifact?replay=<artifact_name>` route redirects to that canonical launch URL.

```bash
RTS_SELFPLAY_SAVE_REPLAY=manual_worker_rush_latest \
  cargo nextest run --config-file .config/nextest.toml \
    --manifest-path server/Cargo.toml scripted_self_play_worker_rush_vs_economy

RTS_FULL_AI_TESTS=1 cargo nextest run --config-file .config/nextest.toml \
  --manifest-path server/Cargo.toml
```

For manual profile-vs-profile balance checks, use the fixed-horizon matchup CLI. It runs one
directed match until a starting City Centre objective win or the tick cap and reports the winner,
first damage, first attacks, first tanks, command counts, and final army/base counts. No winner by
the default 25,000-tick horizon is a draw; army value and other material metrics are diagnostics,
not tiebreakers.

```bash
cd server
cargo run --bin ai-matchup -- ai ai
cargo run --bin ai-matchup -- ai_2_1 ai_turtle --seed 7 --ticks 3000 --json
cargo run --bin ai-matchup -- default ai_turtle --seed 7 --ticks 25000 --json
cargo run --bin ai-matchup -- --list-profiles
```

## Client smoke (headless browser)

Loads the real client in headless Chrome and asserts it renders the PixiJS scene and that
the full UI command loop works: box-select → build placement (which round-trips through the
server and shows the new building) → train-card rendering. Fails on **any** console or page
error. Needs a local Chrome binary; `run-all.sh` installs/reuses `puppeteer-core` through the
shared lockfile-keyed cache.

```bash
tests/run-all.sh --no-rust
# or, after run-all has hydrated tests/node_modules:
node tests/client_smoke.mjs
# env: RTS_URL (default http://127.0.0.1:8081/), CHROME (path to Chrome/Chromium)
```

## Browser performance harness

`scripts/client-perf-harness.mjs` drives headless Chrome against fixed dev-scenario workloads and
writes machine-readable summaries under `target/client-perf/<workload>/<timestamp>/`.

```bash
node scripts/client-perf-harness.mjs --list
node scripts/client-perf-harness.mjs --render-lag-suite --seconds 10
node scripts/client-perf-harness.mjs --workload selected-unit-hud-stress --seconds 10
node scripts/client-perf-harness.mjs --stress-matrix --render-lag-suite --seconds 4 --matrix-cpu 1,2 --matrix-viewport default --matrix-dpr 1 --matrix-repeat 1
```

The harness starts its own local server unless `RTS_URL` or `--base-url` points at a healthy server.
It fails on runtime errors or missing perf summaries, not on absolute FPS thresholds. The render-lag
suite writes per-workload summaries and a `target/client-perf/render-lag-comparison/<timestamp>/`
rollup with advisory 60, 120, 240, and 480 FPS frame-work budget margins plus the next missed p95
budget. Open a generated `summary.json` to inspect workload metadata, build/version, viewport,
entity/context counts, `renderBudget`, frame attribution, `frame.unattributed`,
`frame.rafDispatch`, frame timing aggregates, worst phases, page errors, and the generated client
net report.
The stress matrix writes a `target/client-perf/render-stress-matrix/<timestamp>/` rollup with CPU
throttle, viewport, DPR, repeat, first missed budget, top measured phase, and per-sample artifact
paths. It is advisory local evidence only; use the longer documented command in
`docs/perf-tracing.md` for serious before/after comparisons.

## SVG rig checks

Unit visuals are SVG-authored rigs rendered through Pixi. The temporary legacy equivalence harness
has been removed; future unit-art changes should keep the permanent rig contracts green:

```bash
node tests/rig_schema.mjs
node tests/svg_rig_importer.mjs
node tests/rig_runtime.mjs
node scripts/check-client-architecture.mjs
```

`rig_schema.mjs` and `svg_rig_importer.mjs` validate the normalized data contract and accepted SVG
authoring subset. `rig_runtime.mjs` covers animation sampling, live route definitions, part-group
rendering, teardown, and fail-closed behavior for missing rig definitions. The client architecture
check prevents `renderer/units.js` from regrowing unit-specific procedural drawing branches.

> CI note: `Main test gate` uses `run-all.sh` sub-suite modes in parallel jobs, then reports the
> required aggregate check as `./tests/run-all.sh`. In a headless CI image without Chrome, the
> client smoke test self-skips; pass `CHROME=...` to include it. If Chrome is present but dependency
> hydration fails, the gate fails.

## Tri-State lag scenarios

`tests/tri_state/` is the lag/prediction scenario harness. It runs authored ES-module scenarios,
records lane artifacts under `server/target/tri-state-scenarios/<scenario>/<run-id>/`, and compares
the lanes with domain-aware summaries. The live-room harness includes a direct WebSocket
authoritative lane, a real browser client lane, and a WASM local lane backed by
`rts-sim-wasm`. When generated WASM assets are absent, the local lane records an explicit
`wasmAssetsMissing` disabled reason instead of omitting local artifacts.

Tri-state browser scenarios run automatically in CI, where they are the shared signal for lag and
prediction regressions. Local `tests/run-all.sh` skips them by default because the browser lanes are
latency-sensitive under workstation CPU contention; opt in with `--with-tri-state-browser` or
`RTS_RUN_TRI_STATE_BROWSER=1`. WASM-backed tri-state groups still require generated assets and can
be disabled with `RTS_RUN_WASM_TRI_STATE=0`; the split browser CI job uses that switch while
client smoke verifies that the WASM adapter can load.

Run the no-server harness contract checks:

```bash
node tests/tri_state/self_test.mjs
```

Run one live scenario after starting the server and after `tests/run-all.sh` has hydrated
`tests/node_modules`:

```bash
node tests/tri_state/run.mjs --scenario remote_client_basic_move
node tests/tri_state/run.mjs --scenario queued_order_visibility
node tests/tri_state/run.mjs --scenario dev_scenario_step_tick
```

Run the lag backfill groups that are wired into `tests/run-all.sh`:

```bash
RTS_RUN_TRI_STATE_BROWSER=1 tests/run-all.sh --no-rust
node tests/tri_state/run.mjs --scenario phase-0.5
node tests/tri_state/run.mjs --scenario phase-2.5
```

Phase 2.5 scenarios exercise browser command sequencing, sim-consumption ACK handling,
pending-command drops, stale/duplicate/skipped snapshot diagnostics, receipt diagnostics, rejection
diagnostics, and command timeout reporting. The receipt/rejection scenarios use browser-lane
controller diagnostics because the production wire protocol intentionally exposes only
sim-consumption ACKs in snapshots.

Run the Phase 3.5 WASM local-lane scenarios after generating browser assets:

```bash
scripts/build-sim-wasm.sh
node tests/sim_wasm_smoke.mjs
node tests/tri_state/run.mjs --scenario phase-3.5
node tests/tri_state/run.mjs --scenario local_lane_simple_move
```

Phase 3.5 scenarios assert local initialization from the browser start payload, owner-safe baseline
import, no-op determinism, simple and queued movement summaries, pending command sequences,
correction magnitude, owner-safe baseline artifacts, and explicit unsupported-command reasons.

Run the intentionally failing artifact scenario without failing the shell:

```bash
node tests/tri_state/run.mjs --scenario forced_failure_artifact --allow-failure
```

Add new regressions as small files under `tests/tri_state/scenarios/` that import helpers from
`tests/tri_state/dsl.mjs`. Prefer command-level steps first (`selectOwn`, `issue`,
`waitForSnapshot`, `capture`, then assertions). Add pointer or HUD steps only when the regression
depends on input routing or UI behavior.
