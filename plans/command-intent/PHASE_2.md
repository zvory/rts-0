# Phase 2 - Complete Server Intent Vocabulary

Status: Planned.

Goal: extend the server order-intent model so every planned command stage can be represented and
promoted authoritatively.

## Scope

- Extend `OrderIntent` to represent:
  - self-targeted abilities, e.g. queued Charge
  - world-targeted abilities as one-caster stages, e.g. queued Smoke
  - AT-gun setup facing intents
- Extend promotion in `services::order_queue` for the new intent variants.
- Keep promotion panic-free:
  - dead/stale caster: skip
  - cooldown active at promotion: skip
  - depleted finite-use ability: skip
  - tech missing: skip
  - unaffordable execution cost: skip or existing execution rejection behavior
  - invalid facing/target point: skip
- Preserve the design rule that issue-time readiness is required for queued ability acceptance; no
  future cooldown projection.
- Make queue-full notices work for new intent variants.

## Ability Rules

- Queued Charge appends a self-ability intent to every selected ready rifleman.
- Queued Smoke appends one world-ability intent to one ready scout car per click.
- A later queued attack-move applies to all selected units, including those that did not receive the
  specialized ability stage.
- Costs are paid when the ability executes, not when the intent is appended.

## AT Setup Rules

- `setupAtGuns` gains queued semantics.
- The stored point means "face toward this world point from the unit's current position when the
  setup stage promotes."
- Mixed selections append setup only to AT teams and ignore non-AT units for that command.

## Tests

- Queued `move -> charge -> attackMove` executes in order for riflemen.
- Non-riflemen skip Charge but execute surrounding movement/attack-move stages.
- Queued smoke wall distributes one smoke per click across ready scout cars by queue length.
- Scout cars with smoke on cooldown at issue time do not receive queued smoke.
- Queued smoke skips cleanly when a caster dies, loses tech, loses uses, or cannot afford execution.
- Queued AT setup after move sets facing from arrived position toward the stored point.
- Mixed AT/non-AT selection queues setup only on AT teams and later attack-move on all compatible
  units.

## Done

- Server can represent, queue, promote, and skip every intent described in §3.5 of
  `docs/design/server-sim.md`.
- `cargo test -p rts-sim` passes.
