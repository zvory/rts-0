# Phase 0 - Reference Policy and Contract Anchor

Status: Done.

Goal: freeze the command-intent rules in a pure, heavily tested policy surface before changing live
command execution.

## Scope

- Add `services::order_planner` as the pure reference implementation for command/queue planning.
- Keep the planner independent of `EntityStore`, fog, pathing, economy, and mutation-heavy command
  execution.
- Model planner inputs as issue-time facts:
  - selected unit ids after dedupe/cap
  - unit ability readiness and finite uses
  - current queue length
  - unit compatibility with move/attack/gather/build/setup/ability actions
  - whether an immediate ability can execute without interrupting active orders
- Model planner output as unit-local actions:
  - replace active order
  - append queued intent
  - execute ability now while preserving orders
- Document the rules in `docs/design/server-sim.md` and point the server-sim capsule at that
  section.

## Rules Frozen Here

- Commands are valid at issue time only; do not project future movement, cooldown expiry, tech, or
  affordability.
- Costs are paid at execution time, not queue time.
- Immediate ordinary orders replace active order state and clear queued future intents.
- Queued commands append future unit-local intents.
- Stale queued stages are skipped at promotion time.
- World-targeted abilities allocate one ready carrier per click.
- Self-targeted abilities broadcast to all selected ready carriers.
- Reactive world abilities may execute without interrupting active movement when already fireable.
- AT-gun setup is a queueable facing intent for AT teams only.

## Tests

- Queued world ability assigns one ready carrier per click and round-robins by queue length.
- Queued world ability requires readiness at issue time.
- Queued self ability broadcasts to ready carriers.
- Queued specialized ability followed by attack-move still gives attack-move to the whole group.
- Invalid targets/resources do not create queued stages.
- Queue full emits a notice.
- Immediate fireable world ability can preserve existing movement/queue state.
- Immediate non-fireable world ability replaces an idle ready caster.
- AT setup queues only for setup-capable units.

## Done

- `cargo test -p rts-sim order_planner` passes.
- The server-sim design doc names `order_planner` as the reference implementation.
- No live gameplay behavior depends on the planner yet.
