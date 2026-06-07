# Bewegungskrieg — Design & Architecture

A simple but functional real-time-strategy game inspired by StarCraft: Brood War.
Server-authoritative simulation in **Rust**; client in **HTML/CSS/JS** rendered with
**PixiJS** (WebGL) loaded from a CDN, plus a small Web Audio sound engine. Built to be iterated on
for years, so the boundaries below are contracts: keep them stable and well-documented.

This file is the design index. The source of truth for each contract area lives in
`docs/design/*.md`. If you implement a module, code against the relevant design file. If you must
change an interface, update the relevant design file in the same change.

Read the matching context capsule in `docs/context/` first. Capsules are task-scoped routing docs
that point to the relevant design file and code.

## Design Docs

| Contract area | Source of truth |
| --- | --- |
| High-level architecture, tick/networking model | [docs/design/architecture.md](docs/design/architecture.md) |
| Wire protocol, snapshots, events | [docs/design/protocol.md](docs/design/protocol.md) |
| Rust server, `Game` API, rules, concurrency | [docs/design/server-sim.md](docs/design/server-sim.md) |
| JS client modules, rendering, teardown | [docs/design/client-ui.md](docs/design/client-ui.md) |
| Balance constants, units, buildings, economy | [docs/design/balance.md](docs/design/balance.md) |
| Shared coding conventions | [docs/design/conventions.md](docs/design/conventions.md) |
| Hardening and untrusted input | [docs/design/hardening.md](docs/design/hardening.md) |
| AI opponents | [docs/design/ai.md](docs/design/ai.md) |
| API-driven self-play test harness | [docs/design/testing.md](docs/design/testing.md) |

## Global Invariants

- **Wire protocol is mirrored.** `server/src/protocol.rs` and `client/src/protocol.js` must agree
  on every tag, field name, and shape. Change both together and update
  [docs/design/protocol.md](docs/design/protocol.md).
- **Balance is mirrored.** `server/src/config.rs` is authoritative; `client/src/config.js` mirrors
  the UI/render/fog subset. Change both together and update
  [docs/design/balance.md](docs/design/balance.md).
- **The `Game` API is the seam.** `lobby.rs`/`main.rs` touch the simulation only through the public
  API in `game/mod.rs`. Keep signatures stable; if one changes, update
  [docs/design/server-sim.md](docs/design/server-sim.md) and all callers.
- **`Game::tick()` must be panic-free.** No `unwrap()`/`expect()`/unchecked indexing on the tick
  path; stale ids should be no-ops. Use checked arithmetic on anything derived from client input.
- **Fog is authoritative and cheat-proof.** Anything sent per-player must be gated on
  visibility/ownership. Never send a player an entity or position they cannot see.
- **Clients are untrusted.** Validate and bound everything from the wire: command unit lists, frame
  sizes, placement coordinates, and arithmetic derived from client input.

## Context Capsules

| Task | Start here |
| --- | --- |
| Simulation, tick, services, AI, self-play harness | [docs/context/server-sim.md](docs/context/server-sim.md) |
| Rendering, input, HUD, client modules, teardown | [docs/context/client-ui.md](docs/context/client-ui.md) |
| Wire messages, snapshot shape, fog filtering | [docs/context/protocol.md](docs/context/protocol.md) |
| Costs, supply, sight, unit/building stats | [docs/context/balance.md](docs/context/balance.md) |
| Node integration tests, regression, client smoke | [docs/context/testing.md](docs/context/testing.md) |
| Hardening limits, server bind, build/run pipeline | [docs/context/deployment.md](docs/context/deployment.md) |
