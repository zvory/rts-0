# Phase 3 - Reactive Noninterrupting Ability Execution

Status: Planned.

Goal: support immediate fireable abilities that do not destroy the unit's active movement plan.

## Scope

- Teach the server adapter to set `can_execute_without_interrupt` for immediate ability requests
  that are already legal to execute from the unit's current state.
- Add an application path for `ExecuteAbilityNow { preserve_orders: true }` that:
  - executes the ability effect
  - pays execution cost
  - consumes finite ability use if applicable
  - starts cooldown
  - preserves active order, path, target latch, and queued future intents
- Apply this first to Scout Car Smoke.
- Keep out-of-range immediate Smoke as the normal interrupting behavior unless explicitly queued.

## Non-Goals

- Do not create parallel side-task movement for out-of-range abilities.
- Do not let abilities fire through invalid fog/LOS/range constraints.
- Do not change ability cost timing.

## Tests

- A scout car on `Move` launches in-range immediate Smoke and continues toward its original
  destination.
- A scout car with queued future stages launches in-range immediate Smoke and keeps the queued list.
- Smoke cooldown, use count, and resource payment still update when preserving orders.
- Out-of-range immediate Smoke still replaces/order-stages the chosen caster per existing behavior.
- Non-finite or hidden/invalid targets remain no-ops.

## Done

- Reactive in-range Smoke is smooth and does not erase previous player planning.
- The noninterrupting path is covered by server tests and remains panic-free.
