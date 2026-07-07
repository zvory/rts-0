# Phase 1 - State Inventory and Contract

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Inventory every authoritative state owner that affects future simulation results. Produce the first
`GameCheckpoint` design doc section with field groups, validation expectations, and an explicit
policy for serialized, derived, and transient state. Include entity id allocation/high-water state
as serialized checkpoint state. Document AI controller memory as external/transient: AI players
restore as player slots, and any live AI resumes from a fresh controller. This phase should identify
hard cases before DTO work starts.

## Expected Touch Points

- `docs/design/server-sim.md`
- `docs/design/protocol.md` if serialized artifact shape is documented there
- `docs/context/server-sim.md`
- `server/crates/sim/src/game/**`
- `server/crates/rules/src/**`

## Verification

- Run documentation or formatting checks only if touched files require them.
- Add no broad test burden unless the phase introduces a small inventory test or script.

## Manual Testing Focus

No gameplay manual testing is expected. Review the inventory against a live mental checklist:
entities, entity id allocator, orders, cooldowns, projectiles, smoke, resources, production, fog,
RNG, players, external AI policy, and timers.

## Handoff

The handoff must call out state categories that are risky, unclear, or intentionally deferred.
