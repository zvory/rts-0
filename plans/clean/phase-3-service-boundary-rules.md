# Phase 3 - Service Boundary Rules

## Objective

Turn the current "services should stay focused" convention into concrete internal dependency
rules. This phase does not split files for its own sake; it prevents new sideways coupling while
allowing existing cleanup to happen gradually.

## Work

- Classify `game/services` modules into rough roles:
  - orchestrators: called directly from `systems.rs`
  - command adapters: translate `SimCommand` and validated facts into mutations
  - pure policy: facts in, decisions out
  - query/index services: spatial, occupancy, line of sight, world query
  - mutation helpers: movement coordinator, ability execution, construction, production, death
- Add checker rules for allowed edges between these roles.
- Require new pure policy modules to stay independent of mutable world state.
- Make command-family growth prefer a fact/planner/executor shape:
  - command input
  - issue-time facts
  - pure plan
  - narrow executor
- Start with `commands.rs` and `order_queue.rs` as grandfathered broad adapters rather than forcing
  an immediate split.

## Suggested Rules

- `systems.rs` may call tick systems directly.
- Tick systems should not call each other directly unless allowlisted.
- Pure policy modules may depend on rule/config types and local value objects, but not stores,
  events, fog, or coordinators.
- Query/index services may read `EntityStore` but should not mutate it.
- Mutation helpers may mutate `EntityStore`, but their public functions should have narrow names
  and narrowly scoped arguments.

## Verification

- Add checker tests for allowed and forbidden service edges.
- Add a small design note or checker output section explaining why a failed edge is forbidden.
- Run `cargo test` for the checker crate.

## Outcome

New code gets pushed toward low-coupling patterns by default, especially when agents add command
features or new combat/economy logic.
