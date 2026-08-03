# Phase 7 - Finish Bounded Existing-AI Adoption

## Phase Status

- [ ] Conditional after Phase 6: implement only for concrete remaining duplication named in the
      Phase 5/6 handoffs.

## Objective

Finish small, proven migrations that make Jeff and AI 2.1 easier to work on through the SDK. This is
not a mandatory all-at-once conversion of the shared decision engine. Keep compatibility projections
where they preserve historical policy inputs, and stop when each public surface has a real current-AI
consumer and a single implementation owner.

## Entry Decision

Before editing, inventory the exact remaining duplicate rule lookups, known-world queries, budgets,
reservations, and action emission identified by Phases 5 and 6.

- If no material duplicate remains, mark this phase unnecessary with evidence and proceed to the
  final review; do not manufacture a migration.
- If the work contains more than one independently reviewable slice, split it into follow-up phase
  files rather than creating one large parity PR.
- Do not require removal of `AiObservation`, `AiFacts`, or the legacy frame projection merely for
  architectural symmetry. They may remain useful internal strategic views.

## Work

- Migrate only named production call sites onto the shared rulebook, query, and typed action
  implementations.
- Prefer one bounded slice—such as remaining placement delegation or remaining action emission—per
  implementation phase/PR.
- Preserve cadence, controller memory, map-cache timing, retreat order, candidate and command order,
  floating-point arithmetic, pending-build behavior, stage/attack suppression, and trace output.
- Keep truthful public SDK knowledge separate from legacy synthetic policy inputs. A richer SDK fact
  must not silently alter Jeff or AI 2.1 choices.
- Delete code only when the migrated call sites prove a one-for-one implementation obsolete.
- Update `docs/design/ai.md` with actual ownership and retained compatibility seams.

## Expected Touch Points

- Existing `sdk/**` modules and specifically named `ai_core/**` call sites.
- Focused compatibility tests and `docs/design/ai.md`.

Avoid replay/scorecard/synthetic harness rewrites, broad directory moves, policy checkers, protocol,
client, balance, and simulation command processing.

## Implementation Checklist

- [ ] Record the concrete duplication and bounded migration selected from Phase 5/6 evidence.
- [ ] Split the work if more than one independently reviewable slice remains.
- [ ] Move only those call sites to the existing shared implementation.
- [ ] Preserve legacy strategic projections and all ordering/timing/arithmetic quirks.
- [ ] Remove only the duplicate made demonstrably unused by this migration.
- [ ] Pass the unchanged transcript after each slice and before delivery.
- [ ] Document actual ownership and retained shims.
- [ ] Mark the phase done or unnecessary with evidence.

## Verification

- Run focused old/new equivalence tests for the selected slice.
- Run the exact Phase 1 normal/full transcript without fixture regeneration.
- Run focused public SDK/reference-consumer tests, `rts-ai` nextest, strict clippy, crate/simulation
  architecture checks, docs health, and diff checks.
- Inspect the PR as a semantic call-site migration; defer unrelated cleanup.

## Non-Goals

- No mandatory whole-engine cutover, aesthetic rewrite, or adapter purge.
- No policy or balance improvement in a parity migration.
- No task status, command-result protocol, goal scheduler, behavior tree, GOAP, tactical solver,
  plugin ABI, or non-Rust interface.
- No transcript regeneration or accepted parity exception.

## Handoff Expectations

Report why the phase was necessary (or why it was skipped), exact production call sites migrated,
the implementation owner after migration, deletions made, retained compatibility projections, and
parity evidence. Give Phase 8 only cleanup/checker candidates supported by this final state.
