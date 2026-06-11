# Fog Memory And Perspective Pathing Plan

This plan stages server-side memory of seen buildings, artillery use of that memory, and later
client-perspective pathfinding. Each phase should be implemented, committed, merged to `main`, and
pushed before the next phase begins. After each phase, the implementing agent must provide a
handoff message describing what the next agent should do and which core features should be manually
tested.

## Phase 1: Server-Side Building Memory

Add authoritative per-player memory for enemy buildings that have entered that player's current
vision, storing the latest server-known state visible at that moment. Keep this server-side first so
artillery and future pathing can depend on a trusted memory model without asking the client what it
has explored. Make the memory lifecycle explicit for construction, damage, destruction, ownership,
smoke, and death-vision so later phases do not accidentally leak hidden state.

See [phase-1.md](phase-1.md).

## Phase 2: Artillery Uses Remembered Buildings

Change artillery target selection so fogged enemy buildings are eligible only when the firing
player has remembered them, using the remembered position and latest seen footprint/state rather
than omniscient live building locations. Keep direct visibility behavior unchanged: currently
visible buildings still use live entity state, while remembered buildings become stale intel that
can be wrong after the enemy repairs, cancels, destroys, or replaces structures out of sight. Add
tests around never-seen buildings, seen-then-fogged buildings, destroyed remembered buildings, and
smoke-blocked visibility.

See [phase-2.md](phase-2.md).

## Phase 3: Expose Remembered Building Intel To The Client

If player-facing UI needs stale building silhouettes or target affordances, add a protocol field for
remembered-but-not-currently-visible building views. The server remains authoritative and sends only
the recipient player's remembered records, never live hidden state, and the client renders them as
non-selectable stale intel under fog. This phase should update `server/crates/protocol`,
`client/src/protocol.js`, `docs/design/protocol.md`, and the relevant UI modules together.

See [phase-3.md](phase-3.md).

## Phase 4: Design Client-Perspective Pathing Semantics

Before implementation, resolve the gameplay questions for pathing against player perspective:
what units should assume about unseen building blockers, when they discover a blocked route, and
how move, attack-move, explicit attack, gather, build, and rally orders should react. The likely
default is that move orders repath when a newly discovered blocker invalidates the path, while
attack-oriented orders may attack the blocker if it is hostile and blocking progress, but this needs
explicit user confirmation. This phase produces a short accepted design note and updates
`docs/design/server-sim.md` before any pathing code changes.

See [phase-4.md](phase-4.md).

## Phase 5: Perspective-Aware Occupancy And A* Pathing

Introduce a pathing occupancy view that includes terrain plus buildings known to the unit owner's
perspective, not every hidden live building on the map. Path requests should choose authoritative
full occupancy only for systems that truly need omniscient validation, while movement planning uses
owner-perspective occupancy so units can path into apparently open fog and be surprised by unseen
blockages. Preserve panic-free tick behavior, bounded path budgets, cache correctness, and clear
debug path snapshots.

See [phase-5.md](phase-5.md).

## Phase 6: Blockage Discovery And Order Response

Implement the accepted behavior for units that encounter an unseen building blocking their planned
route. Detection should be based on actual movement/path progress against live authoritative
occupancy, then transition the order according to the design from phase 4: repath, attack, stop, or
fail with feedback as appropriate for the current order. Add focused self-play/regression coverage
for hidden wall-offs, newly scouted blockers, destroyed blockers opening paths, and attack-move
behavior around enemy buildings.

See [phase-6.md](phase-6.md).

## Cross-Phase Constraints

- Keep fog authoritative and server-side. Never accept remembered building positions, explored
  tiles, or path blockers from the client.
- Do not leak hidden live state. Remembered records are stale player intel, not a backdoor into the
  current entity store.
- Preserve the wire protocol mirror. Any snapshot/protocol change must update Rust protocol DTOs,
  compact transport, `client/src/protocol.js`, and `docs/design/protocol.md` together.
- Preserve the `Game` API seam unless a phase explicitly updates `docs/design/server-sim.md` and all
  callers in the same commit.
- Keep `Game::tick()` panic-free. Stale entity ids, missing remembered records, invalid path goals,
  and destroyed blockers must degrade to no-op/repath/fail states rather than panics.
- Treat smoke and death vision deliberately. If smoke hides a building, memory should not refresh
  through smoke; death vision may show stale intel only if the same snapshot visibility rules allow
  it.
- Keep pathing caches scoped to the occupancy view that produced them. A path found with
  perspective occupancy must not be reused as if it were found with full live occupancy.
- Phase completion means implementation, verification, commit, merge into `main`, and push to
  `origin/main`; do not start the next phase on an unmerged branch.
