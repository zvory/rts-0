# Tests

## Run everything (one command)

`run-all.sh` builds the server in debug (overflow checks **on**, which the hardening regression
tests rely on), boots it, polls `GET /` until healthy, runs Rust formatting/lint/fast scripted
tests and all the live-server suites, tears the server down, and exits non-zero if **any** suite
fails. The private server runs with `RTS_TEST_TICK_MS=5` by default, so live-server tests wait on
simulated progress instead of real-time 30 Hz wall clock; normal `cargo run` remains 30 Hz.
If a server is already answering on the port it is reused and left running.

This command is the required local gate for ordinary commits. Run `./scripts/install-hooks.sh` once
per checkout to install the tracked hooks locally. Merge commits intentionally bypass the local hook
gate. GitHub Actions also runs this command after pushes to `main` as a shared signal, but `main` is
intentionally left open for direct pushes.

```bash
tests/run-all.sh                 # local gate: cargo fmt --check + cargo test + clippy + API suites + client smoke + tri-state scenarios
tests/run-all.sh --full-ai       # local gate plus long AI self-play/simulation coverage
tests/run-all.sh --no-rust       # skip Rust fmt/test/lint
tests/run-all.sh --no-client     # skip the headless-browser smoke test
tests/run-all.sh -v              # print suite timings and pass/fail lines
PORT=8090 tests/run-all.sh       # use a different port
CARGO_TARGET_DIR=/tmp/rts-target tests/run-all.sh  # override the per-worktree Cargo target dir
RTS_NODE_DEPS_CACHE_DIR=/tmp/rts-node-deps tests/run-all.sh  # override shared Node deps cache
CHROME=/path/to/chrome tests/run-all.sh
```

The client smoke test self-skips (not a failure) only when a Chrome binary is missing. When Chrome
is available, `run-all.sh` hydrates `puppeteer-core` into a shared dependency cache keyed by the
SHA-256 hash of `tests/package-lock.json`, then links this worktree's ignored `tests/node_modules`
to the matching cache entry. Hydration uses `npm ci`, so a lockfile/package mismatch fails the
gate instead of silently reusing the wrong dependency tree.

By default, the local gate and Cargo helper use an isolated target directory for each worktree
under `/tmp/rts-cargo-target/`. This keeps final binaries, test harnesses, and self-play
artifacts branch-local while keeping the checkout clean. Override with
`CARGO_TARGET_DIR=/path/to/target` when you need a specific target location.

Installed repo hooks run `scripts/cleanup-worktrees.sh --auto` after commits and merges on `main`.
Auto cleanup removes only clean `zvorygin/*` worktrees whose branch is already contained in local
`main`, their matching target dirs, and a small bounded number of old target dirs that do not map to
any active worktree. Use `scripts/cleanup-worktrees.sh --dry-run` to inspect what would be removed.

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

## Headless simulation self-play

Runs inside the Rust test suite with no live server. Plain `cargo test` runs the fast scripted
self-play coverage. Long profile-backed matchups, long AI simulation checks, and the full real-AI
match are gated behind `RTS_FULL_AI_TESTS=1` because they dominate wall-clock time.
The tests create scripted API clients that drive `Game` through `enqueue`/`tick`/`snapshot_for`,
exercising gathering, oil, Depot/Barracks/Vehicle Works construction, Rifleman/Tank training, rush
pressure, and combat. Successful runs replay the authoritative tick-stamped command log through a
fresh game and compare the replayed event stream and final snapshots against the live run. On
failure it writes normal `ReplayArtifactV1` replay artifacts under the Cargo target dir's
`selfplay-failures/` directory, plus self-play diagnostics beside them. To save a successful run too, set
`RTS_SELFPLAY_SAVE_REPLAY` to either `1` for an auto-generated artifact name or to an explicit safe
artifact name; successful runs are then written under the target dir's
`selfplay-artifacts/<name>/`.
When you open a replay artifact in the browser, use the server instance that produced it, or
start a fresh one on its own port before loading `/dev/selfplay?replay=<artifact_name>`.

```bash
RTS_SELFPLAY_SAVE_REPLAY=manual_worker_rush_latest \
  cargo test scripted_self_play_worker_rush_vs_economy

RTS_FULL_AI_TESTS=1 cargo test
```

For manual profile-vs-profile balance checks, use the fixed-horizon matchup CLI. It runs one
directed match to elimination or a tick cap and reports the winner, first damage, first attacks,
first tanks, command counts, and final army/base counts.

```bash
cd server
cargo run --bin ai-matchup -- rush tech
cargo run --bin ai-matchup -- expand tech
cargo run --bin ai-matchup -- saturation tech --seed 7 --ticks 20000 --json
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

> CI note: `run-all.sh` is the CI entry point — it builds the server, launches it, waits for
> `GET /` to return 200, checks Rust formatting, runs every suite, and exits non-zero on any
> failure. In a headless CI image without Chrome, the client smoke test self-skips; pass
> `CHROME=...` to include it. If Chrome is present but dependency hydration fails, the gate fails.

## Tri-State lag scenarios

`tests/tri_state/` is the lag/prediction scenario harness. It runs authored ES-module scenarios,
records lane artifacts under `server/target/tri-state-scenarios/<scenario>/<run-id>/`, and compares
the lanes with domain-aware summaries. The live-room harness includes a direct WebSocket
authoritative lane, a real browser client lane, and a WASM local lane backed by
`rts-sim-wasm`. When generated WASM assets are absent, the local lane records an explicit
`wasmAssetsMissing` disabled reason instead of omitting local artifacts.

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
