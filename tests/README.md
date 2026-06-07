# Tests

## Run everything (one command)

`run-all.sh` builds the server in debug (overflow checks **on**, which the hardening regression
tests rely on), boots it, polls `GET /` until healthy, runs Rust formatting/lint/fast scripted
tests and all the live-server suites, tears the server down, and exits non-zero if **any** suite
fails. The private server runs with `RTS_TEST_TICK_MS=5` by default, so live-server tests wait on
simulated progress instead of real-time 30 Hz wall clock; normal `cargo run` remains 30 Hz.
If a server is already answering on the port it is reused and left running.

This command is the required local gate for every commit. Run `./scripts/install-hooks.sh` once per
checkout to install the tracked hooks locally. GitHub Actions also runs this command after pushes to
`main` as a shared signal, but `main` is intentionally left open for direct pushes.

```bash
tests/run-all.sh                 # fast gate: cargo fmt --check + cargo test + clippy + 3 API suites + client smoke
tests/run-all.sh --full-ai       # fast gate plus long AI self-play/simulation coverage
tests/run-all.sh --no-rust       # skip Rust fmt/test/lint
tests/run-all.sh --no-client     # skip the headless-browser smoke test
tests/run-all.sh -v              # print suite timings and pass/fail lines
PORT=8090 tests/run-all.sh       # use a different port
CARGO_TARGET_DIR=/tmp/rts-target tests/run-all.sh  # override the shared Cargo target cache
CHROME=/path/to/chrome tests/run-all.sh
```

The client smoke test self-skips (not a failure) when `puppeteer-core` or a Chrome binary is
missing. By default, repo-level Cargo config sends every checkout and worktree to the shared
target directory at `/tmp/rts-cargo-target/rts-0-server`, so plain `cargo build`, `cargo test`,
and `cargo clippy` reuse dependency builds instead of compiling from scratch in every worktree.
Override with `CARGO_TARGET_DIR=/path/to/target` when you need an isolated cache.

For scripts that need to print the shared target dir or for explicit wrapper usage:

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
exercising gathering, oil, Depot/Barracks/Factory construction, Rifleman/Tank training, rush
pressure, and combat. Successful runs replay the authoritative tick-stamped command log through a
fresh game and compare the replayed event stream and final snapshots against the live run. On
failure it writes replay artifacts under the Cargo target dir's `selfplay-failures/` directory. To
save a successful run too, set
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
error. Needs `puppeteer-core` and a local Chrome binary.

```bash
cd tests && npm install
node client_smoke.mjs
# env: RTS_URL (default http://127.0.0.1:8081/), CHROME (path to Chrome/Chromium)
```

> CI note: `run-all.sh` is the CI entry point — it builds the server, launches it, waits for
> `GET /` to return 200, checks Rust formatting, runs every suite, and exits non-zero on any
> failure. In a headless CI image without Chrome, the client smoke test self-skips; pass
> `CHROME=...` to include it.
