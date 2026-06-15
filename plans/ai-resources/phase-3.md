# Phase 3 - Availability-Safe Assignment

Status: Done.

## Goal

Make worker assignment unable to emit known-invalid gather commands and ensure failed oil
assignment cannot reserve workers away from valid steel assignment.

## Scope

- Update `actions::assign_workers_to_resource` and `ResourceAssignmentPolicy` so candidate resource
  selection is availability-aware.
- Prefer passing a filtered candidate-node view from the economy plan or availability model. If that
  is too invasive, add an explicit predicate or allowed-node-id set to `ResourceAssignmentPolicy`
  and require all economy callers to provide it.
- Ensure `nearest_unreserved_node` ignores nodes that are not currently mineable, depleted,
  pre-reserved, or already reserved by the action context.
- Preserve deterministic nearest-node ordering and stable tie-breaking by node id.
- Ensure a worker is reserved only after a valid assignable node is chosen. If oil assignment cannot
  find a valid node, the same idle worker must remain eligible for the later steel assignment pass.
- Audit non-economy callers such as self-play scripts or special harnesses that call
  `assign_workers_to_resource`. Either route them through the availability model or explicitly
  document why they use a different target source.
- Add targeted tests proving that the action layer refuses non-mineable oil even if upstream
  economy intent asks for it.

## Expected Touch Points

- `server/crates/ai/src/ai_core/actions.rs`
- `server/crates/ai/src/ai_core/decision/mod.rs`
- `server/crates/ai/src/ai_core/decision/resources.rs`
- `server/crates/ai/src/selfplay/scripts.rs` if harness callers need an availability-aware policy
- Focused action and decision tests under `server/crates/ai/src/ai_core/`

## Behavioral Requirements

- `assign_workers_to_resource` must not emit `Command::Gather` for a node classified as
  non-mineable by the current availability model.
- An impossible oil request must not reserve an idle worker or resource node.
- Steel assignment should still emit valid gather commands in the same decision pass when free
  mineable steel exists.
- Existing occupied-node and action-context reservation behavior must continue to prevent duplicate
  assignment to the same node within one decision pass.
- Panic reassignment may include currently gathering workers only when the target node is valid and
  mineable.

## Verification

- Add focused action tests for:
  - non-mineable oil candidate ignored
  - no worker reservation after failed oil candidate selection
  - same worker can then be assigned to mineable steel
  - completed-expansion oil candidate is accepted
- Run targeted tests such as:

```bash
cd server
cargo test -p rts-ai assign_workers_to_resource
cargo test -p rts-ai availability
```

- If command emission structures or traces change, run the relevant profile-backed fast tests that
  cover ordinary mining.

## Manual Testing Focus

Inspect an AI opening where workers spawn near the City Centre before expansion. Confirm no gather
commands are repeatedly sent to out-of-range oil and idle workers move to available steel.

## Handoff

After implementation, mark this phase done and summarize the assignment API change, the action-layer
backstop, and the tests run. Call out any remaining raw resource-selection callers and whether they
are test-only or still need Phase 4 cleanup.
