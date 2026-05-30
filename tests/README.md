# Tests

## Run everything (one command)

`run-all.sh` builds the server in debug (overflow checks **on**, which the hardening regression
tests rely on), boots it, polls `GET /` until healthy, runs the Rust scripted tests and all the
live-server suites, tears the server down, and exits non-zero if **any** suite fails. If a server
is already answering on the port it is reused and left running.

```bash
tests/run-all.sh                 # cargo test + 3 API suites + client smoke
tests/run-all.sh --no-rust       # skip the cargo test step
tests/run-all.sh --no-client     # skip the headless-browser smoke test
PORT=8090 tests/run-all.sh       # use a different port
CHROME=/path/to/chrome tests/run-all.sh
```

The client smoke test self-skips (not a failure) when `puppeteer-core` or a Chrome binary is
missing. Everything below documents the individual suites the runner orchestrates.

---

The suites run against a **live server**. To run one on its own, start the server first:

```bash
cd server && cargo run        # serves the client + websocket on :8080
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

Runs inside the Rust test suite with no live server. The test creates two scripted API clients that
drive `Game` through `enqueue`/`tick`/`snapshot_for`, exercising gathering, gas,
Depot/Barracks/Tank Factory construction, Rifleman/Tank training, and combat. On failure it writes replay artifacts under
`server/target/selfplay-failures/`.

```bash
cd server && cargo test scripted_self_play_exercises_economy_tech_and_combat
```

## Client smoke (headless browser)

Loads the real client in headless Chrome and asserts it renders the PixiJS scene and that
the full UI command loop works: box-select → build placement (which round-trips through the
server and shows the new building) → train-card rendering. Fails on **any** console or page
error. Needs `puppeteer-core` and a local Chrome binary.

```bash
cd tests && npm install
node client_smoke.mjs
# env: RTS_URL (default http://127.0.0.1:8080/), CHROME (path to Chrome/Chromium)
```

> CI note: `run-all.sh` is the CI entry point — it builds the server, launches it, waits for
> `GET /` to return 200, runs every suite, and exits non-zero on any failure. In a headless CI
> image without Chrome, the client smoke test self-skips; pass `CHROME=...` to include it.
