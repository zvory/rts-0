# RTS

A small but functional real-time-strategy game inspired by StarCraft: Brood War.
Build buildings, train units, gather minerals & gas, scout through fog of war, and wipe
out your opponent. Server-authoritative multiplayer with a Rust server and an
HTML/CSS/JS + PixiJS client. No sound.

> Status: v1 (focused MVP). One faction, 3 unit types, 4 building types, 2 resources,
> fog of war, last-player-standing. Built to be iterated on — see `DESIGN.md` for the
> architecture and the contracts every module follows.

## Quick start

You need a recent Rust toolchain (`cargo`). No JS build step — the client is plain ES
modules and loads PixiJS from a CDN.

```bash
cd server
cargo run --release
```

Then open <http://localhost:8080>. To play head-to-head, open it in **two browser
windows**, join the same room, both click **Ready**, and the host clicks **Start match**.
A solo start drops you into a peaceful sandbox so you can explore and build.

Set `RTS_ADDR` to change the bind address (default `0.0.0.0:8080`).

## Controls

| Action | Input |
|--------|-------|
| Select unit / building | Left-click |
| Box-select | Left-drag |
| Move / gather / attack target | Right-click (context-sensitive) |
| Attack-move | `A` then left-click |
| Stop | `S` |
| Pan camera | Arrow keys / WASD / screen-edge / drag minimap |
| Zoom | Mouse wheel |
| Build (worker selected) | Command card buttons, then click to place |
| Train (HQ/Barracks selected) | Command card buttons |

## Layout

```
server/   Rust authoritative server (axum + tokio). Also serves the client files.
client/   HTML/CSS/JS client (PixiJS via CDN). Served at /.
DESIGN.md Architecture, wire protocol, module contracts, and balance. Read this first.
```

## Development

```bash
cd server
cargo run            # debug build, serves client/ and the websocket at :8080
cargo fmt            # format
cargo clippy         # lint
```

The wire protocol is defined in two mirrored files that must stay in sync:
`server/src/protocol.rs` and `client/src/protocol.js`. Balance lives in
`server/src/config.rs` (authoritative) mirrored by `client/src/config.js`.

## Known future work
- Upgrade PixiJS v7 → v8 (async `Application.init`).
- Unit-vs-unit collision avoidance / flocking (units currently soft-overlap).
- Spectator mode, replays, and a binary wire format for scale.
- AI opponents.
