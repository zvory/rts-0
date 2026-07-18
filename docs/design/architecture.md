## 1. High-level architecture

```
┌────────────────────────┐         WebSocket (JSON)         ┌──────────────────────────┐
│  Browser client (JS)   │  ── ClientMessage ───────────▶   │     Rust server          │
│  PixiJS renderer        │                                  │  axum + tokio            │
│  - lobby UI             │  ◀─ ServerMessage ──────────     │  - static file serving   │
│  - input / selection    │                                  │  - /ws upgrade           │
│  - camera / minimap      │                                  │  - Lobby (rooms)         │
│  - fog overlay (local)   │                                  │  - Game (authoritative)  │
└────────────────────────┘                                   └──────────────────────────┘
```

- The **server** owns the authoritative game state and runs a fixed-rate simulation
  loop (`TICK_HZ`). Clients only send **commands** (intent); they never mutate game
  state directly.
- Every tick the server produces a **per-player snapshot**, applying **fog of war**:
  a player only receives neutral/enemy entities standing on tiles that player can
  currently see. When a unit/building dies, it leaves temporary team sight at its final position
  for five seconds. Death-vision entities remain marked `visionOnly` for rendering and selection,
  while direct attack targeting uses a scoped fog union of live vision and active lingering death
  sources. Lingering death sight does not refresh remembered building intel or general
  auto-acquisition, and hidden non-revealed targets remain unavailable.
- Lobby-time spectators are connected humans who are not seated in the simulation. They receive
  snapshots filtered to the union of all active players' current fog, all player resource rows,
  and no controllable units/buildings.
  Spectators may join before or during a normal live match; active-player late joins and lobby role
  switches after match start are rejected. Shared replay playback also accepts late viewers at its
  current room-owned cursor, whether it came from a persisted replay lobby or automatic post-match
  replay.
- The **client** renders snapshots, interpolating entity positions between them for
  smoothness, and draws the **fog overlay** from the server-provided current visibility grid
  while keeping explored history locally. Local sight stamping exists only as a fallback for
  older/dev object snapshots; the server remains the fog authority.
- Local development exposes game-backed dev scenario pages under `/dev/scenarios` and a neutral
  saved-artifact replay launcher at `/?replayArtifact=<artifact_name>`. The older
  `/dev/replay-artifact?replay=<artifact_name>` route redirects to that canonical URL. Scenario
  rooms stream **full-world** snapshots for authored local debugging; saved self-play artifacts use
  the same replay viewer runtime as post-match and match-history replays.
- App-owned launch URLs use the namespaced `rtsLaunch` query parameter for setup automation. The
  initial live-match mode is `/?rtsLaunch=match...`, which joins a normal lobby and drives existing
  lobby messages for spectator AI self-play setup instead of adding a special server protocol.
- Interact's `game` namespace uses that same normal lobby path for one isolated human-vs-AI match.
  Its client bridge is gated to fresh `interact-game-*` player launches and exposes only bounded
  fog-filtered/UI inspection, camera, move, surrender, screenshots, and recording.
- The same server exposes a lightweight documentation wiki at `/wiki`. It renders only allowlisted
  Markdown under `docs/context` and `docs/design`, rewrites relative Markdown doc links into
  `/wiki/...` links, rejects traversal or unsupported paths, and serves `/wiki/stats` from
  Rust-authoritative rules and faction catalog data instead of client mirrors.
- The same Rust process serves the static client files, so development is a single
  `cargo run` and then open the printed local URL.

### Compatibility policy

The game is pre-alpha and latest-version-only. Do not preserve obsolete protocol, replay,
client/server, map, or asset behavior just for backwards compatibility unless a specific migration
or debugging workflow requires it. Breaking changes are acceptable when the design docs and all
current Rust/JS mirrors are updated together.

### Workspace crate boundaries

The Rust server workspace is split by dependency direction. Lower crates must not depend on higher
crates:

```
rts-server  -> rts-ai, rts-sim, rts-rules, rts-protocol, rts-contract
rts-ai      -> rts-sim, rts-rules, rts-protocol, rts-contract
rts-sim     -> rts-rules, rts-protocol, rts-contract
rts-protocol -> rts-contract
rts-rules
rts-contract
```

`rts-server` is the only crate that may own Axum/Tokio WebSocket/static-file serving and lobby room
tasks. `rts-sim` owns `Game`, tick systems, deterministic replay, map/fog/entity state, and
simulation perf accounting without importing server transport. `rts-ai` owns live controllers and
self-play harnesses and drives the sim only by observing snapshots and enqueueing ordinary
`SimCommand`s. `rts-rules` owns pure vocabulary, balance data, terrain, economy, and combat
formulas. `rts-protocol` owns serde wire DTOs and compact snapshot transport. `rts-contract` owns
shared semantic DTOs that are below the wire and sim layers.

The server wiki belongs to `rts-server` because it is an Axum route and because generated reference
HTML is a presentation of lower-crate data. Wiki prose comes from repository Markdown files; wiki
stats rows come from `rts-rules` definitions and faction catalogs. After changing docs links,
run `node scripts/check-docs-health.mjs` for local Markdown link hygiene. After changing
allowlisted docs, rules definitions, faction catalogs, upgrades, or ability metadata, run
`node scripts/check-wiki.mjs` to cover route safety, generated table completeness, and client
catalog parity.

`scripts/check-crate-boundaries.mjs` enforces the implemented Cargo package graph and rejects
server-only imports in lower crates. Any intentional graph change must update this section, the
script, and the affected context capsule in the same change.

### Tick & networking model
- `TICK_HZ = 30` (~33 ms per simulated tick).
- The server broadcasts a snapshot every `SNAPSHOT_EVERY_N_TICKS` ticks (default 1 →
  30 snapshots/s).
- Commands are queued on arrival and drained at the start of each tick (deterministic
  ordering per connection; ordering across connections is arrival order).
- The client renders at `requestAnimationFrame` (~60fps), interpolating between the two
  most recent snapshots using wall-clock time.

---
