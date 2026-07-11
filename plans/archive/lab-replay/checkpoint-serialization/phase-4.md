# Phase 4 - Combat, Effects, and Timed State

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Extend checkpoints to combat and in-progress effects. Cover cooldowns, attack targets, projectile
state, mortar shots, smoke, impact timers, death timers, and any authoritative timed effect that can
change future simulation. This phase should make checkpoint restore safe while active combat or
area effects are in flight.

## Expected Touch Points

- `server/crates/sim/src/game/services/**`
- `server/crates/sim/src/game/entity/**`
- `server/crates/sim/src/game/systems.rs`
- Sim tests for combat, artillery, smoke, and visibility

## Verification

- Add resume tests that checkpoint after firing but before impact or effect expiry.
- Compare original and restored games after enough ticks for the effects to resolve.
- Include fog-filtered projection comparisons where combat visibility matters.

## Manual Testing Focus

Run a local combat scenario with mortar or smoke behavior and inspect that normal effects still
appear and resolve.

## Handoff

The handoff must name any remaining authoritative timed state not covered by checkpoints.
