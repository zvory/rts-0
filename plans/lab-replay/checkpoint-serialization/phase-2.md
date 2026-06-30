# Phase 2 - Core Tick Zero Round Trip

Status: Not started.

## Scope

Add the initial `GameCheckpoint` DTO and import/export path for minimal tick-zero state. The
checkpoint should restore map identity, tick count, player slots, teams, starting resources, RNG
state, and basic entity data with exact entity-id preservation. This should support empty labs and
fresh normal match starts, not arbitrary mid-game state yet.

## Expected Touch Points

- `server/crates/sim/src/game/**`
- `server/crates/protocol/src/lib.rs` only if the checkpoint DTO is shared through the protocol
  crate
- `docs/design/server-sim.md`
- Focused Rust tests under the sim crate

## Verification

- Run focused Rust tests for checkpoint serde, validation, and tick-zero round trip.
- Include a test that exports a game, imports it, and exports it again in canonical form.

## Manual Testing Focus

Start one normal match and one blank lab after the phase lands, then verify the game still reaches
the same first visible state as before.

## Handoff

The handoff must list which fields are supported and which mid-game fields are still absent.
