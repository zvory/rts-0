# Queued Orders - Multi-Phase Plan

This plan adds Brood-War-style Shift queued commands while keeping the simulation
server-authoritative. The first goal is reliable queued movement and basic visual feedback. Worker
chaining, mixed combat orders, and richer rally behavior come later once the core queue machinery is
proven.

Current command queuing in `DESIGN.md` only means WebSocket commands are drained at the start of the
next tick. It is not a per-unit order queue. Today each mobile entity owns exactly one active
`Order`, and each ordinary command replaces that order immediately.

## Confirmed Decisions

- Build the feature over multiple phases.
- Initial gameplay scope is queued movement and attack-move, plus basic queued order markers.
- Worker gather is a terminal infinite order. Once a worker starts gathering, it should not advance
  to later queued orders.
- Queued orders after a build run after construction completes, not when the worker starts laying
  the building.
- Unit queues cap at 8 queued orders per unit.
- Building rally queues cap at 2 rally stages per building.
- If a queued order becomes invalid at promotion time, silently skip it and try the next queued
  order.
- Basic queued order markers are required in the first implementation phase that exposes queued
  commands to players.
- Multi-building rally editing can be implemented later.

## Core Model

Queued orders should be stored as intent, not execution state.

- Active `Order` remains the per-tick execution state.
- A new queue stores lightweight `OrderIntent` values.
- Promotion from queued intent to active order must reuse the same validation and coordinator paths
  as live commands where possible.
- Normal non-Shift commands replace the active order and clear the queued order list.
- Shift commands append to the queue without interrupting the current active order.
- `Stop` clears the active order, path, target id, worker carry state, and queued orders.
- A promoted order is validated against the current world state. Stale entities, dead targets,
  depleted resource nodes, invalid build sites, and blocked requirements are skipped without notice.
- Formation destinations are recalculated when a queued group move is promoted, not when it is
  queued.

## Order Completion Rules

- `Move`: advance when the unit reaches the destination or tolerant arrival marks it arrived.
- `AttackMove`: advance only when the movement destination is reached. Enemy engagements along the
  way do not consume the queued order.
- `Attack`: later phase. Advance when the target dies, becomes invalid, or the order is judged
  unreachable by explicit failure rules.
- `Gather`: terminal infinite. Do not auto-promote later queued orders after harvesting starts.
- `Build`: later phase. Advance after construction completes or after the build intent fails.

## Phases

- [x] [Phase 0 - Contract and queue foundation](PHASE_0.md)
- [x] [Phase 1 - Queued move and attack-move](PHASE_1.md)
- [x] [Phase 2 - Basic queued order markers](PHASE_2.md)
- [Phase 3 - Queued worker build and gather handoff](PHASE_3.md)
- [Phase 4 - Mixed attack and movement queues](PHASE_4.md)
- [Phase 5 - Multi-stage rallies](PHASE_5.md)
- [Phase 6 - Hardening, replay, and documentation audit](PHASE_6.md)

## Non-Negotiable Invariants

1. Server authority stays intact. Clients send intent only; the server owns active orders, queued
   orders, validation, movement, combat, economy, and production.
2. Protocol mirrors stay synchronized. Any wire change updates `server/src/protocol.rs`,
   `client/src/protocol.js`, and `DESIGN.md` in the same implementation change.
3. `Game::tick()` stays panic-free. Queue promotion must tolerate stale ids, dead entities,
   depleted resources, invalid coordinates, and non-finite client input.
4. Queue sizes are hard capped. A malicious client must not be able to allocate unbounded queued
   orders or force unbounded per-tick promotion work.
5. Fog remains authoritative. Owner-only queue and rally marker data must not reveal hidden enemy
   positions or hidden target ids.
6. Active order execution stays explicit. Do not hide long-running worker/build/combat progress in
   queued intent objects.
7. Replay determinism is preserved. Queued command flags and promotion behavior must replay from the
   command log deterministically.
8. Existing non-Shift command behavior remains compatible: normal commands replace current orders.

## Suggested Implementation Order

Implement the phases in order. Phase 1 should be the smallest playable slice: Shift-click a path,
watch selected units follow the queued waypoints, and use normal right-click or `Stop` to clear the
queue. Do not add worker build/gather chaining until move/attack-move promotion is tested.
