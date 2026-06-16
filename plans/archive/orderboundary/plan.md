# Order Boundary Refactor Plan

## Purpose

Refactor the server simulation command and queued-order boundary so `commands.rs` and
`order_queue.rs` stop acting as broad mutation hubs. Preserve current command semantics while
separating validation, planning, mutation, queued promotion, and ability launch responsibilities.

## Overall Constraints

- Keep the public `Game` API stable unless a phase explicitly updates `docs/design/server-sim.md`
  and every caller in the same change.
- Preserve current gameplay semantics for immediate orders, Shift queues, ability launch,
  queue-full notices, issue-time validation, promotion-time validation, stale queued stages, and
  deterministic point-move batching.
- Do not weaken ownership checks, command-budget checks, fog safety, or the panic-free tick path.
- Prefer small named mutation helpers over a new global simulation facade.
- Run focused Rust tests for touched command/order behavior and
  `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` after
  each implementation phase.
- After each phase, provide a handoff naming verification results, remaining risks, and the core
  command/order flows that should be manually tested.
- Implement, commit, merge to `main`, and push each phase before starting the next phase.

## Phase Summaries

### [Phase 1 - Characterization Tests](phase-1.md)

Add targeted tests for current command and queued-order behavior before moving production logic.
The tests should cover the risky flows concentrated in `commands.rs`, `order_queue.rs`, and
`ability_orders.rs`. This phase should change no gameplay behavior.

### [Phase 2 - Entity Order Mutation Helpers](phase-2.md)

Move repeated order-state mutations behind narrow `Entity` or `EntityStore` helper methods. The
helpers should name the difference between replacing active orders, clearing active orders, clearing
all orders, appending queued intents, and popping promoted stages. This reduces accidental order,
path, setup, or ability-state drift when later phases move command execution code.

### [Phase 3 - Command Execution Context](phase-3.md)

Introduce a local command execution context so command helpers stop passing long mutable parameter
lists everywhere. The context should remain private to command application rather than becoming a
general simulation facade. This phase prepares extraction without changing when commands apply.

### [Phase 4 - Planned Action Executor](phase-4.md)

Split the mutation side of pure order-planner actions away from command decoding and validation.
`commands.rs` should continue to validate player authority and construct planner facts, while the
executor applies the narrow planned effects. This creates a clearer API between command admission,
pure planning, and state mutation.

### [Phase 5 - Queue Promotion Executor](phase-5.md)

Apply the same boundary to queued promotion while preserving the current tick order in `systems.rs`.
`order_queue.rs` should identify ready stages and promotion-time facts, then delegate mutations to
shared execution helpers where safe. This phase must preserve deterministic queued move batching and
the semantic differences between issue-time and promotion-time ability execution.

### [Phase 6 - Ratchets And Documentation](phase-6.md)

Tighten archcheck and documentation after the new boundaries exist. The final guardrails should
make it hard to rebuild the current broad adapter shape accidentally. This phase should be mostly
docs and architecture policy, with no new gameplay behavior.

## Non-Goals

- Do not redesign command semantics, queue length, ability costs, cooldown behavior, or tick order.
- Do not remove every direct field read in one pass.
- Do not move AI, transport, lobby, or protocol concerns into `rts-sim` command services.
- Do not bless a broader archcheck baseline without a specific cleanup reason.

## Handoff Rules

Each phase file has an implementation checklist. Mark each item complete as it lands, include exact
verification commands in the final phase handoff, and name the manual command/order flows that need
human smoke testing. If a phase discovers a semantic ambiguity, stop and hand it off rather than
choosing a new behavior silently.
