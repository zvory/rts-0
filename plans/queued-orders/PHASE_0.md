# Phase 0 - Contract and Queue Foundation

Status: Done.

Goal: introduce the internal queue model and protocol contract without exposing queued orders to
players yet.

## Scope

- Add a queued-command field to the wire contract, most likely `queued?: bool` on command variants
  that can eventually append.
- Keep omitted `queued` equivalent to `false` for backward compatibility.
- Add an internal `OrderIntent` model that can represent at least:
  - point move
  - point attack-move
  - target attack
  - gather node
  - build intent
- Add a per-unit queue with a hard cap of 8 intents.
- Add helper methods for:
  - clearing queued orders
  - appending with cap enforcement
  - popping/promoting the next valid intent
  - clearing active order plus queued orders for replacement commands
- Add a no-op building rally queue shape with a hard cap of 2 stages, but do not expose multi-stage
  rallies yet.

## Design Notes

The queue should sit next to `MovementState::order`, not replace it. Active `Order` remains the
execution state used by movement, combat, gather, and construction systems. Queued intents should
not contain path waypoints, movement phases, attack phases, gather progress, or build progress.

Promotion should be centralized. Avoid scattering "pop next order" logic directly through movement,
combat, economy, and construction systems. Systems should report or trigger completion through a
small helper so later phases can add order kinds without changing every call site again.

## Tests

- Normal move command clears an existing queued list.
- Shift move appends until the cap and drops additional entries.
- `Stop` clears active order and queued orders.
- Stale queued unit ids and invalid queued coordinates do not panic.
- Serialized command logs preserve the `queued` flag for replay.

## Done

- No player-visible queued behavior is required in this phase.
- Existing non-queued commands behave exactly as before.
- `cargo test` in `server/` should pass.
