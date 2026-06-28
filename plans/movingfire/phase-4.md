# Phase 4 - Remove Legacy Charge Runtime State

## Phase Status

Status: planned.

## Objective

Remove legacy Charge gameplay state now that Methamphetamines no longer depends on fake persistent
charge ticks. Keep only the compatibility needed for old clients or replay command logs.

## Scope

- Remove `charge_ticks` and charge-refresh gameplay plumbing where it is no longer needed.
- Remove or quarantine legacy Charge ability descriptors, constants, command handlers, tests, and
  comments that imply current gameplay support.
- Keep old `charge` wire commands and replay entries parseable as no-ops when compatibility requires
  it.
- Update client-visible metadata and docs so Methamphetamines is the permanent upgrade source of
  rifleman moving fire.
- Clean up stale tests by converting them to Methamphetamines behavior tests or deleting no-longer
  relevant Charge assertions.

## Expected Touch Points

- `server/crates/sim/src/game/entity/`
- `server/crates/sim/src/game/services/ability_orders.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/rules/src/faction.rs`
- `server/crates/rules/src/balance/abilities.rs`
- `client/src/config/`
- protocol/replay compatibility tests
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/balance.md`

## Verification

- Focused Rust command and replay tests proving old Charge commands are harmless compatibility
  no-ops.
- Focused Rust meth tests proving the upgrade still grants the intended speed and moving-fire
  behavior.
- `node scripts/check-faction-catalog-parity.mjs` if ability metadata or client mirrors change.
- `node tests/protocol_parity.mjs` if protocol mirrors change.
- `git diff --check`.

## Manual Test Focus

Confirm Methamphetamines still upgrades riflemen correctly in a live match. Load or replay an old
command log with Charge entries and confirm it does not crash or alter current gameplay.

## Handoff Expectations

List exactly what compatibility surface remains for Charge, what was removed from runtime state, and
which docs/tests now define Methamphetamines as the authoritative behavior.
