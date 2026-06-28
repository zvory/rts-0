# Phase 2 - Waiting Construction Behavior

Status: planned.

## Goal

Implement the player-visible waiting behavior for arrived workers. Workers should hold valid build
orders at the site while waiting for resources, cancel when a building/scaffold claims the
footprint, and use a three-second grace timer for relevant unit blockers.

## Scope

- Update immediate build command handling so affordability is no longer a hard issue-time
  rejection for otherwise valid build orders.
- Update queued build promotion so an unaffordable build intent promotes into an active build order
  instead of being skipped, as long as the builder and site are otherwise valid.
- Preserve immediate rejection for unknown building kinds, missing worker eligibility, missing tech
  requirements, out-of-bounds/terrain-invalid footprints, resource-node blockers, and current
  building/scaffold blockers.
- In `construction_system`, when a `ToSite` or waiting worker is in arrival range:
  - resume a matching owned scaffold first, as today;
  - if placement is clear but resources are insufficient, keep the active build order and retry on
    later ticks;
  - if placement is clear and resources are sufficient, spend resources and spawn the scaffold;
  - if placement is blocked by a building/scaffold, clear the active build order;
  - if placement is blocked by a resource/terrain/invalid condition, clear the active build order;
  - if placement is blocked by a relevant unit body, increment the unit-blocked timer and keep the
    order until the three-second timeout;
  - if the unit blocker clears before timeout, reset the unit-blocked timer and return to ordinary
    resource/placement retry behavior;
  - if the unit blocker remains through timeout, clear the active build order.
- Keep failure notices from spamming every tick while waiting. Prefer entering-state notices such as
  one resource shortage notice and one delayed `Cannot build there` when a blocker times out or a
  building cancels the order.
- Keep active construction behavior unchanged once the scaffold exists.
- Keep `clear_active_order()` semantics for timed-out/canceled builds so queued handoff orders
  remain eligible for promotion if that is the current active-order failure pattern.
- Update or replace tests that currently expect:
  - queued unaffordable builds to be skipped;
  - unit-blocked final placement to clear the worker immediately.

## Expected Touch Points

- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/order_queue.rs`
- `server/crates/sim/src/game/services/construction.rs`
- `server/crates/sim/src/game/services/standability.rs`
- `server/crates/sim/src/game/entity/order.rs`
- `server/crates/sim/src/game/entity/entity.rs`
- Focused tests under:
  - `server/crates/sim/src/game/services/commands/tests/build.rs`
  - `server/crates/sim/src/game/services/order_queue.rs`
  - `server/crates/sim/src/game/services/construction.rs`
  - `server/crates/sim/src/game/systems.rs`

Avoid touching:

- Client protocol/rendering code
- Server lobby/room task code
- AI strategy code unless a focused test proves it must adapt
- Balance stats for buildings or units

## Implementation Notes

- The current command path checks affordability in `order_build`; that must be relaxed for build
  orders so a worker can reach the site and wait.
- The current queued promotion path checks affordability in `build_intent_promotion_error`; that
  must be relaxed for build orders unless resuming existing scaffolds already covers the case.
- Do not relax tech/worker/terrain/building validity. Waiting is only for resources and relevant
  unit-body blockers after a valid build intent exists.
- Keep resource payment atomic with scaffold spawn. If `spend_cost` fails after a previous
  affordability check, leave the worker waiting rather than dropping the order.
- Consider clearing the worker path when it enters waiting-at-site so it visibly stands near the
  footprint instead of trying to repath into the blocked/center tile.
- Reset the unit-block timer whenever the status is not `BlockedByUnit`, including resource wait,
  clear placement, scaffold resume, and permanent cancellation.
- Be careful with tick order: construction runs before same-tick collision cleanup, so transient
  overlaps may still be visible to the placement probe for that tick.

## Verification

Add focused Rust tests for:

- Immediate build order with insufficient resources is accepted and sends the worker toward the
  site, then waits at arrival without spawning a scaffold.
- Waiting worker starts construction when resources become available.
- Waiting worker cancels if another building/scaffold appears on the footprint.
- Unit-blocked footprint waits for less than three seconds without clearing the order.
- Unit-blocked footprint starts construction or returns to resource waiting when the blocker clears
  before timeout.
- Unit-blocked footprint clears the active order after three seconds of continuous blocking.
- Queued unaffordable build promotes into an active build order instead of skipping to the next
  queued intent.
- Existing scaffold resume still works without resources and without charging a second cost.

Suggested focused commands:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-sim construction
cargo test --manifest-path server/Cargo.toml -p rts-sim queued_build
cargo test --manifest-path server/Cargo.toml -p rts-sim build_order
git diff --check
```

Use narrower filters if these become too broad.

## Manual Testing Focus

In a local match or lab scenario, try four core flows:

- issue a build while broke, then mine enough resources and confirm the waiting worker starts it;
- spend resources elsewhere before a worker arrives, then confirm it waits and later starts;
- place a competing building/scaffold on the footprint and confirm the waiting worker cancels;
- park a unit on the footprint briefly, move it away before three seconds, then repeat and leave it
  in place long enough to confirm timeout to idle.

## Handoff

After implementation, mark this phase done and summarize the exact build-order waiting state
machine, notice behavior, and focused test names. Call out any remaining manual uncertainty around
the ambiguous "clears after three seconds" wording so Phase 3 can resolve it with the user if
needed.
