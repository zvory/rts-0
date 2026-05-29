# Tests

End-to-end tests that run against a **live server**. Start it first:

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
drive `Game` through `enqueue`/`tick`/`snapshot_for`, exercising gathering, gas, Depot/Barracks
construction, Soldier/Heavy training, and combat. On failure it writes replay artifacts under
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

> CI note: both tests assume a server on `:8080`. A simple CI job builds the server,
> launches it in the background, waits for `GET /` to return 200, then runs both scripts.
