# Phase 6 - Add a Typed Per-Think Action Builder

## Phase Status

- [ ] Ready for implementation after Phase 5 merges.

## Objective

Turn the useful local mechanics in `ai_core/actions.rs` into a practical typed action builder for AI
authors: same-think affordability, actor reservations, deterministic accumulation, and typed local
errors. This is a command-construction aid, not a task system, automatic planner, or promise that the
simulation accepted or completed anything.

## Public Surface

- Extend the Phase 3 `AiActions` surface with typed helpers required by the Phase 4 reference
  strategy and current Jeff/AI-2.1 call sites. Prefer extending the existing sink over introducing
  a second overlapping command API.
- Support the demonstrated core operations: paid build at an explicit candidate, resume, train,
  research, gather, move, attack-move/direct attack, Hold Position, and Anti-Tank Gun setup. Omit an
  operation with no real consumer rather than wrapping every command.
- Provide local `ActionError`/`ActionBlocker` results for facts the builder actually knows, such as
  empty group, unsupported kind, insufficient same-think budget, and already-reserved actor,
  resource, or producer.
- Preflight each helper before mutating local budget/reservations or appending its command. A local
  failure changes none of those values.
- Preserve distinct worker, resource-node, and producer reservation namespaces.
- If a tactical `UnitGroup` is useful at demonstrated call sites, make it a sorted/deduplicated
  non-empty ID set. Do not apply it to caller-ordered worker, producer, or target candidates.
- Return commands in helper-call order. Do not globally sort, merge, coalesce, or normalize batches.
- Keep raw `SimCommand` construction inside the runtime adapter/emitter; public strategies use
  SDK-owned action types only.

## Authority and Naming

- Use "builder," "batch," "local preflight," and "reservation." Do not call the API transactional
  without the qualifier "local," and do not use accepted, rejected, completed, succeeded, failed,
  or task language for simulation outcomes.
- Local success means only that an ordinary command was constructed and emitted. The simulation
  remains free to ignore it through normal validation.
- Do not allocate intent IDs or maintain a task ledger. Observational completion feedback remains a
  deferred research problem.

## Existing-AI Adoption

- Move or wrap current per-think budget, reservations, command accumulation, and trace labeling
  behind the shared implementation.
- Route bounded real Jeff/AI-2.1 production/economy and tactical call sites through it in this phase;
  do not leave the public builder as a parallel implementation for a later giant cutover.
- Preserve exact worker/producer candidate order, resource-node tie-breaks, paid Pump Jack behavior,
  resume-without-repay behavior, queue flags, helper-call order, trace strings, lack of automatic
  tactical reservations, and Jeff's steel-only cross-tick pending-build commitment quirk.
- Keep compatibility wrappers where deleting them would turn this into a broad rewrite.

## Expected Touch Points

- `server/crates/ai/src/sdk/actions.rs` or the Phase 3 action module.
- `server/crates/ai/src/ai_core/actions.rs` and focused decision call sites.
- The Phase 4 reference consumer and an outside-crate SDK integration test.
- `docs/design/ai.md` and the author guide.

No protocol, client, balance, `Game`, simulation validation, or command-result change belongs here.

## Implementation Checklist

- [ ] Add typed helpers for the concrete reference/current-AI operations.
- [ ] Add honest local blockers and mutation-free local failure.
- [ ] Preserve independent reservations and call-ordered emission.
- [ ] Add `UnitGroup` only where a real tactical consumer benefits.
- [ ] Route bounded existing-AI action call sites through the shared implementation now.
- [ ] Preserve command shapes/order, queue flags, traces, budgets, and compatibility quirks.
- [ ] Update the outside-crate reference strategy to use the completed author path.
- [ ] Pass the unchanged Phase 1 transcript and mark the phase done.

## Verification

- Prove every local blocker leaves budget, reservations, commands, and trace unchanged.
- Test independent reservation namespaces, command shapes/flags, mixed helper order, and any
  `UnitGroup` canonicalization.
- Compile and run the public reference consumer without private imports or raw commands.
- Run the exact Phase 1 normal/full transcript, focused `rts-ai` tests, strict clippy, crate/sim
  architecture checks, docs health, and diff checks.

## Non-Goals

- No intent IDs, status ledger, command receipts, idempotent goals, cross-tick scheduler, behavior
  tree, GOAP, or automatic planner.
- No claim of simulation atomicity, acceptance, legality, or completion.
- No wrapper for every command and no public raw-command escape hatch.
- No policy tuning, target selection, placement/path service, or full strategic-engine rewrite.

## Handoff Expectations

Report the typed helpers and concrete consumers, exact meaning of every local blocker, command/order
compatibility evidence, current-AI call sites migrated, wrappers retained, and any remaining
duplication that has earned a bounded Phase 7 migration.
