# Phase 8 - Evidence-Driven Cleanup and Narrow Ratchets

## Phase Status

- [ ] Conditional after Phase 7 review; skip any work without a concrete duplicate or recurring
      boundary violation.

## Objective

Remove only compatibility code proven obsolete by merged migrations and add only narrow checks that
prevent a demonstrated regression. The public reference strategy from Phase 4 is the authoring
conformance specimen; do not create a second ceremonial SDK showcase.

## Entry Decision

- Independently rerun the Phase 1 transcript from current `origin/main`.
- List each proposed deletion with its replacement owner and zero remaining legitimate consumers.
- List each proposed architecture rule with the concrete regression it prevents and the exact file
  roles that must remain exempt.
- If those lists are empty, mark the phase unnecessary and archive the plan. Cleanup is not required
  merely because this is the last numbered phase.

## Work

- Delete one-for-one obsolete compatibility implementations only after proving them unused.
- Retain low-level parsing/command access for runtime adapters, replay, scorecard, host, and synthetic
  harness code with legitimate responsibilities.
- Strengthen the Phase 4 public reference consumer if later SDK additions are otherwise unexercised.
- Add a boundary check only for a narrow, stable rule already respected by production strategy code,
  such as preventing new raw protocol parsing or `SimCommand` construction outside documented
  adapters. Use explicit small role allowlists and focused checker tests.
- Update `docs/design/ai.md` and the author guide to describe the actual final lifecycle, knowledge
  boundary, local-action semantics, existing-AI consumers, and retained exceptions.

## Expected Touch Points

- Only obsolete compatibility files named by the Phase 7 handoff.
- The existing Phase 4 reference integration test/example.
- A focused checker and suite selection only if the entry decision justifies one.
- `docs/design/ai.md` and the author guide.

No gameplay policy, balance, protocol, client, simulation validation, or broad reorganization belongs
here.

## Implementation Checklist

- [ ] Rerun exact parity before cleanup.
- [ ] Document the replacement owner and consumer audit for every deletion.
- [ ] Delete only proven-unused one-for-one implementations.
- [ ] Extend the existing reference consumer only where needed.
- [ ] Add only regression-backed narrow boundary rules, or explicitly skip the checker.
- [ ] Preserve legitimate runtime/replay/scorecard/host/synthetic exceptions.
- [ ] Pass the transcript after each deletion cluster and mark the phase done or unnecessary.

## Verification

- Run the exact Phase 1 normal/full transcript without fixture regeneration.
- Run the public reference consumer and all focused SDK tests.
- If a checker is added, test both forbidden production-strategy access and every documented allowed
  role; wire suite selection only after those tests pass.
- Run focused `rts-ai` nextest, strict clippy, crate/sim architecture checks, docs health, and diff
  checks. GitHub's main gate remains authoritative.

## Non-Goals

- No cleanup quota, broad import policy, directory reorganization, or aesthetic rewrite.
- No second conformance strategy solely to touch every type.
- No policy improvement, task lifecycle, goal framework, plugin ABI, or speculative hardening.

## Completion and Handoff Expectations

Report whether the phase was implemented or skipped, parity evidence, each deleted implementation
and replacement owner, retained low-level exceptions, any regression-backed boundary rules, and the
final supported authoring path. Archive unresolved task/result semantics and other speculative work
as deferred research rather than unfinished implementation.
