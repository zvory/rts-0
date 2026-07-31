# Phase 6 - Expose the Transactional Action Planner

## Phase Status

- [ ] Ready for implementation after Phase 5 merges.

## Objective

Promote the useful local transaction machinery already in `ai_core/actions.rs` into a small public
authoring API. The planner owns same-think affordability, worker/node/producer reservations,
deterministic command accumulation, and minimal tactical unit canonicalization while the simulation
remains the authority. Jeff continues through compatibility wrappers and must emit the exact same
commands and traces.

## Public Surface

- Add public `AiActionPlanner`, `SpendBudget`, `ReservationLedger`, `UnitGroup`, and
  `ActionBlocker` types under `rts_ai::sdk`.
- Decouple the planner from the unused private `AiFacts` lifetime. Earlier frame/rule/query services
  choose candidates; the planner receives explicit actors, producers, targets, kinds, and positions.
- Preserve current `SpendBudget` behavior:
  - saturating Steel, Oil, and free-supply accounting;
  - entity-kind classification before cost lookup;
  - the legacy `with_committed_steel` compatibility operation;
  - no forecasting, debt, refunds, or authoritative economy promises.
- Expose separate worker, resource-node, and production-building reservation namespaces. The same
  numeric ID in separate namespaces does not collide; tactical units are not automatically
  reserved because Jeff may intentionally command them more than once in a think.
- Define immutable `UnitGroup` as a sorted/deduplicated non-empty ID set for tactical group
  commands only. Do not feed caller-ordered worker pools, producer candidates, target candidates,
  or raw commands through it.
- Cover common typed operations:
  - paid build at a selected tile and resume without repaying original cost;
  - train and research at an explicit producer;
  - gather from an explicit node;
  - move, attack-move, direct attack, Hold Position, and Anti-Tank Gun setup.
- Every typed operation preflights all local checks, mutates nothing on failure, then atomically
  commits budget/reservations and appends one ordinary command.
- Preserve Phase 4's tracked-submission correlation: a caller may request a tracked form of a
  supported operation and receive its `AiIntentId` only after local preflight succeeds. Failure
  returns `ActionBlocker` without consuming an ID or mutating the task ledger.
- Typed blockers describe only locally known failures such as empty group, unsupported kind,
  insufficient local budget, already reserved actor/resource/producer, or no eligible candidate.
  They are not simulation acceptance/rejection results.
- `finish()` returns commands in planner call order. Do not globally sort, merge, coalesce, or
  deduplicate command batches.
- Provide a narrow SDK-owned `UncommonAction` enum for the currently supported abilities, formation
  moves, artillery, rally, cancel, and autocast cases not covered by typed planner helpers. The
  planner emitter translates it to `SimCommand`; public strategies never construct or import a raw
  simulation command. Future commands require adding an explicit SDK variant rather than silently
  escaping the boundary.

## Compatibility Migration

- Move or wrap current budget, reservations, command accumulation, and trace labeling behind the
  public implementation while keeping existing helper selection policy private.
- Preserve exactly:
  - caller-provided build-worker pool/member order;
  - ascending producer order supplied by current facts;
  - unit-priority and count tie-break order;
  - first-occurrence resource-worker candidates and distance/node-ID choice;
  - oil assignment producing a paid Pump Jack build;
  - resume construction not reserving original cost;
  - helper-call command order, trace strings, queue flags, and lack of tactical reservations;
  - the steel-only cross-tick pending-build commitment quirk for Jeff.
- Keep compatibility wrappers for `try_build`, resume, train, research, resource assignment, and
  tactical helpers in this phase. Full Jeff cutover belongs to Phase 7; wrapper deletion belongs to
  Phase 8.

## Expected Touch Points

- New `server/crates/ai/src/sdk/actions.rs` and SDK exports.
- `server/crates/ai/src/ai_core/actions.rs` and its tests.
- Import-only or wrapper changes in decision production/expansion/defense/frontal/turtle/trace code.
- An outside-crate SDK action integration test.
- `docs/design/ai.md`.

No protocol, client, balance, `Game`, or simulation command-processing change belongs in this phase.

## Implementation Checklist

- [ ] Add public budget, reservation, blocker, group, and planner types.
- [ ] Implement atomic local failures and call-ordered finish.
- [ ] Cover the bounded common action set and SDK-owned uncommon actions.
- [ ] Keep worker/producer candidates distinct from sorted tactical groups.
- [ ] Route legacy helpers mechanically through the shared implementation.
- [ ] Preserve typed task submission-to-ID correlation through planner success/failure.
- [ ] Preserve trace strings, command shapes/order, queue flags, and budget quirks.
- [ ] Add external-consumer action tests and full Jeff parity.
- [ ] Document local blockers and authority limits.
- [ ] Mark this phase done in the implementation commit.

## Verification

- Test `UnitGroup` sorting/deduplication/empty rejection and immutable iteration.
- Test saturating committed-Steel/free-supply behavior and exact unit/building/upgrade reservation.
- Prove each blocker leaves budget, reservations, commands, and trace unchanged.
- Test independent reservation namespaces, every typed command shape/queue flag, mixed operation
  call order, and uncommon-action translation.
- Keep legacy helper tests exact and add an outside-crate integration test using no private imports.
- Run the exact Phase 1 normal/full Jeff transcript; fixture regeneration is forbidden.
- Run focused `rts-ai` nextest, clippy, crate-boundary checks, docs health, and diff check.

## Manual Test Focus

Inspect representative baseline transcript entries for build, train, gather, stage, hold/setup, and
attack operations, and confirm the public integration test reads like a small practical AI rather
than an internal test harness. Player-facing impact is none.

## Non-Goals

- No declarative goal language, behavior tree, GOAP, or automatic planner.
- No authoritative command lifecycle, new placement/path service, or full wrapper for every command.
- No public raw-`SimCommand` escape hatch, global command normalization, or automatic combat-unit
  reservations.
- No full Jeff decision-engine migration or pending/stage cleanup.
- No serialized SDK or external-language ABI.

## Handoff Expectations

Report the public action API, mapping from every legacy helper to planner methods, remaining wrappers
and internal low-level translations, blocker inventory, candidate-order rules, exact transcript results, and
external-consumer test. Give Phase 7 a bounded migration order and name every legacy behavior that
must survive wrapper deletion.
