# Phase 2 - Architecture Checker Guard

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Add an automated guard for checkpoint policy coverage. The guard should catch new authoritative
state owners or stateful services that lack a registry entry, checkpoint annotation, or explicit
derived/transient policy. Prefer integration with the existing architecture checker if that keeps
the workflow simple.

## Expected Touch Points

- `server/crates/archcheck/**` or the current architecture-checking crate
- The checkpoint policy registry from Phase 1
- Focused checker tests or fixtures

## Verification

- Run the architecture checker.
- Run focused checker tests.

## Manual Testing Focus

No gameplay manual testing is expected.

## Handoff

The handoff must include the exact checker command and one example of the failure message.
