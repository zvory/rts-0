# Capsule: balance

Use when tuning costs, supply, sight, sizes, unit/building stats, or any number a player feels.

## Read first
- [docs/design/balance.md](../design/balance.md) — balance definitions & constants
  - §5.1 Target theme and MVP combat loop
  - §5.2 Current implementation constants

## Code map
- `server/src/config.rs` — **authoritative**
- `client/src/config.js` — mirrors the UI/render/fog subset (costs, supply, sight, sizes)

## Invariants
- **Mirror.** Change both files together when the value is visible to the client (cost, supply,
  sight, size, anything used by HUD/render/fog). Server-only tuning (damage curves, internal
  timers) stays in `config.rs`.
- Patch notes: collect player-facing bullets as you work (changed stats, economy, combat
  behavior, UI affordances, expected strategic impact). Factual, evidence-backed; if uncertain,
  say what changed and what to watch in playtest.

## Cross-capsule triggers
- Any rule/behavior change beyond a number → [server-sim.md](server-sim.md).
- New field on the wire → [protocol.md](protocol.md).
