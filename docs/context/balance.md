# Capsule: balance

Use when tuning costs, supply, sight, sizes, unit/building stats, or any number a player feels.

## Read first
- [docs/design/balance.md](../design/balance.md) — balance definitions & constants
  - Final source-of-truth map and guardrails
  - Client mirror boundary inventory
  - §5.1 Target theme and MVP combat loop
  - §5.2 Current implementation constants

## Code map
- `server/crates/rules/src/defs.rs` — authoritative unit/building stat records
- `server/crates/rules/src/faction.rs` — authoritative faction catalogs, train/build/research
  availability, and Rust-exported ability/command-card metadata
- `server/crates/rules/src/balance.rs` — authoritative shared scalars and client-visible timing,
  range, duration, body, resource, and ability effect constants
- `server/src/config.rs`, `server/crates/sim/src/config.rs` — compatibility shims for local callers
- `client/src/config.js` — mirrors the UI/render/fog subset (costs, supply, sight, sizes)

## Invariants
- **Mirror.** Change `server/crates/rules/src/balance.rs` and `client/src/config.js` together when
  the value is visible to the client (cost, supply, sight, size, anything used by HUD/render/fog).
  Server-only tuning (damage curves, internal timers) stays in Rust rules/sim code.
- **Parity.** Run `node scripts/check-faction-catalog-parity.mjs` after client-visible rules,
  faction catalog, upgrade, ability descriptor/effect, resource amount, or mirrored config changes.
- `/wiki/stats` is generated from Rust rules definitions and faction catalogs. Run
  `node scripts/check-wiki.mjs` after visible rules, catalog, upgrade, or ability metadata changes
  so the generated reference tables and client catalog mirror are checked together.
- Patch notes: collect player-facing bullets as you work (changed stats, economy, combat
  behavior, UI affordances, expected strategic impact). Factual, evidence-backed; if uncertain,
  say what changed and what to watch in playtest.

## Cross-capsule triggers
- Any rule/behavior change beyond a number → [server-sim.md](server-sim.md).
- New field on the wire → [protocol.md](protocol.md).
