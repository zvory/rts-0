# Phase 1 - Resource Availability Facts

Status: Not started.

## Goal

Introduce an explicit AI resource availability model that classifies every known resource node for
the current player without changing live AI behavior yet.

## Scope

- Add a small AI-owned data model for resource availability, likely near
  `server/crates/ai/src/ai_core/facts.rs` or a new `ai_core/resource_availability.rs` module.
- For each `AiResourceSummary`, derive at least:
  - node id, kind, position, and remaining value
  - whether the node has remaining resources
  - whether the node is currently mineable by one of the player's completed City Centres
  - nearest completed mining City Centre id, if any
  - current latched-worker count
  - whether the node is occupied by any current worker
  - whether the node is pre-reserved by the current economy plan or action context
  - whether the node is a future expansion candidate rather than a current mining target
- Derive mineability from completed own City Centres in the AI observation and the same mining-range
  constants the sim uses. Do not call private sim internals.
- Keep "known" and "mineable now" distinct. Expansion candidate discovery must still be able to use
  known non-main resources that are outside current City Centre range.
- Add helper queries for:
  - free mineable nodes by resource kind
  - occupied/latched counts by resource kind
  - current steel saturation target from free/remaining mineable steel
  - whether any free mineable oil exists
  - node lookup by id for trace/tests
- Preserve existing public behavior during this phase. It is acceptable for the new model to be
  built and tested but not yet consumed by economy decisions.

## Expected Touch Points

- `server/crates/ai/src/ai_core/observation.rs`
- `server/crates/ai/src/ai_core/facts.rs`
- Optional new module under `server/crates/ai/src/ai_core/`
- `server/crates/ai/src/ai_core/decision/resources.rs`
- Focused tests in `server/crates/ai/src/ai_core/facts.rs` or a new module test

## Behavioral Requirements

- A resource outside completed-City-Centre mining range must classify as known but not mineable.
- A resource in range of an incomplete City Centre must classify as known but not mineable.
- A resource in range of a completed City Centre with remaining resources must classify as mineable.
- Depleted resources must not count as free mineable targets.
- Latched workers must contribute to occupancy counts even before assignment code changes.
- Classification must be deterministic and stable by node id for equal-distance/tie cases.
- No phase-1 command output should change solely because the model exists.

## Verification

- Add focused Rust tests for mineability, incomplete City Centre exclusion, depleted-node exclusion,
  and occupancy counts.
- Run the smallest targeted AI test command that covers the new module, for example:

```bash
cd server
cargo test -p rts-ai resource_availability
```

- If the implementation touches crate boundaries, also run:

```bash
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
```

## Manual Testing Focus

No live gameplay manual test is required if behavior is not consumed yet. If debug traces or test
fixtures expose the model, inspect one opening snapshot and confirm home steel is mineable while
far expansion resources are known-only.

## Handoff

After implementation, mark this phase done and summarize the resource availability API, the exact
mineability predicate, the focused tests run, and any known behavior still using raw resource lists.
Tell Phase 2 which helpers should replace current economy target and desired-oil calculations.
