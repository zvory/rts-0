# Capsule: balance

Use when tuning costs, supply, sight, sizes, unit/building stats, or any number a player feels.

## Read first
- [docs/design/balance.md](../design/balance.md) — balance definitions & constants
  - Client mirror boundary inventory
  - §5.1 Target theme and MVP combat loop
  - §5.2 Current implementation constants

## Code map
- `server/crates/rules/src/balance.rs` — **authoritative**
- `server/src/config.rs`, `server/crates/sim/src/config.rs` — compatibility shims for local callers
- `client/src/config.js` — mirrors the UI/render/fog subset (costs, supply, sight, sizes)

## Invariants
- **Mirror.** Change `server/crates/rules/src/balance.rs` and `client/src/config.js` together when
  the value is visible to the client (cost, supply, sight, size, anything used by HUD/render/fog).
  Server-only tuning (damage curves, internal timers) stays in Rust rules/sim code.
- `/wiki/stats` is generated from Rust rules definitions and faction catalogs. Run
  `node scripts/check-wiki.mjs` after visible rules, catalog, upgrade, or ability metadata changes
  so the generated reference tables and client catalog mirror are checked together.
- Patch notes: collect player-facing bullets as you work (changed stats, economy, combat
  behavior, UI affordances, expected strategic impact). Factual, evidence-backed; if uncertain,
  say what changed and what to watch in playtest.

## Cross-capsule triggers
- Any rule/behavior change beyond a number → [server-sim.md](server-sim.md).
- New field on the wire → [protocol.md](protocol.md).
