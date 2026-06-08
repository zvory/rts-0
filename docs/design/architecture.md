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
  currently see, plus visual-only entities inside the one-second lingering sight left behind
  when that player's unit/building dies. This makes the fog cheat-proof (hidden enemies are never
  sent outside live or explicit lingering death vision).
- Lobby-time spectators are connected humans who are not seated in the simulation. They receive
  snapshots filtered to the union of all active players' current fog, all player resource rows,
  and no controllable units/buildings.
  Spectators must join or switch roles before the match starts; mid-match joins are rejected.
- The **client** renders snapshots, interpolating entity positions between them for
  smoothness, and draws the **fog overlay** from the server-provided current visibility grid
  while keeping explored history locally. Local sight stamping exists only as a fallback for
  older/dev object snapshots; the server remains the fog authority.
- Local development also exposes a dev-only watch entry at `/dev/selfplay` that auto-runs
  scripted self-play and streams **full-world** snapshots (no fog) to the ordinary match
  renderer. This path is isolated from normal lobby play and is only for debugging.
- The same Rust process serves the static client files, so development is a single
  `cargo run` and then open the printed local URL.

### Tick & networking model
- `TICK_HZ = 30` (~33 ms per simulated tick).
- The server broadcasts a snapshot every `SNAPSHOT_EVERY_N_TICKS` ticks (default 1 →
  30 snapshots/s).
- Commands are queued on arrival and drained at the start of each tick (deterministic
  ordering per connection; ordering across connections is arrival order).
- The client renders at `requestAnimationFrame` (~60fps), interpolating between the two
  most recent snapshots using wall-clock time.

---
