# Phase 1 - State Ownership Registry

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Create the first authoritative-state registry for checkpoint policy. It should list systems or
owners, the state they own, whether the state is serialized, derived during import, or transient,
and the tests that cover it. Keep the registry close to design docs or code so it stays reviewable.

## Expected Touch Points

- `docs/design/server-sim.md`
- `docs/context/server-sim.md`
- Optional small machine-readable registry if the architecture checker will consume it later

## Verification

- Documentation review and any doc formatting checks needed.

## Manual Testing Focus

No gameplay manual testing is expected.

## Handoff

The handoff must identify registry entries that are incomplete or need implementation follow-up.
