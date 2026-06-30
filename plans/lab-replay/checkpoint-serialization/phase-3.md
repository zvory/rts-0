# Phase 3 - Orders, Economy, and Production State

Status: Not started.

## Scope

Extend checkpoints to long-lived non-combat gameplay state. Cover pending commands, active orders,
worker collection state, build progress, production queues, rally settings, supply, resources, and
tech or loadout state that changes future simulation. This phase should make ordinary economy and
construction states safe to restore.

## Expected Touch Points

- `server/crates/sim/src/game/commands.rs`
- `server/crates/sim/src/game/services/**`
- `server/crates/sim/src/game/entity/**`
- Sim tests for production, building, and worker behavior

## Verification

- Add resume tests that checkpoint during resource gathering, production, and building progress.
- Run focused sim tests for the touched systems.

## Manual Testing Focus

In a local match, queue workers or units, place a building, and verify no visible behavior regresses
after the phase.

## Handoff

The handoff must call out any gameplay queue or order type that remains outside checkpoint coverage.
