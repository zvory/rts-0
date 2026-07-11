# Checkpoint Architecture Guards Plan

> [!WARNING]
> **POTENTIALLY STALE SUBDIVISION - DO NOT IMPLEMENT YET.**
> This lab-replay subdivision depends on assumptions that may change when
> `plans/archive/game-state/plan.md` lands. Re-evaluate this subplan and its phase files before
> implementation.

## Purpose

Make checkpoint completeness hard to accidentally regress. This stage adds explicit ownership rules,
checks, and targeted tests so new authoritative state must declare how it round-trips. It should run
after the core checkpoint contract exists.

## Phase Summaries

### [Phase 1 - State Ownership Registry](phase-1.md)

Create a lightweight registry or documentation table for authoritative state owners and their
checkpoint policy. Each entry should identify the owner, serialized fields, derived fields, and
transient fields. This gives reviewers and future agents one place to check when adding new systems.

### [Phase 2 - Architecture Checker Guard](phase-2.md)

Add an automated guard that catches likely hidden authoritative state. The guard can live in the
existing architecture checker or a focused test, and should prefer explicit annotations or registry
entries over fragile name matching. This phase should fail loudly when new stateful systems are
added without checkpoint policy.

### [Phase 3 - Regression Harness Integration](phase-3.md)

Wire checkpoint resume coverage into targeted test selection for replay/checkpoint/sim changes.
The harness should remain focused and opt-in where it is expensive, but easy for implementation
agents to run. This phase should make the right validation command obvious during future work.

### [Phase 4 - Contributor and Design Docs](phase-4.md)

Update contributor-facing docs, context capsules, and design docs with checkpoint rules. Document
how to add new authoritative state, how to mark derived/transient state, and what tests to add. This
phase turns the guards into a maintained engineering habit.

## Overall Constraints

- Guards should enforce checkpoint policy without blocking harmless client-only or derived state.
- Avoid brittle string scans where a typed or registry-backed check is practical.
- Keep expensive resume scenarios out of the default full local loop unless the repo later chooses
  to promote them.
- Every guard failure should tell the implementer what policy or test to add.

## Handoff Requirements

Every handoff must explain which future omissions are now caught automatically and which still rely
on review.
