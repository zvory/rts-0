# Phase 1 - Faction Identity Contract

Status: Designed, not implemented.

## Objective

Introduce faction identity as a stable contract while keeping every player on the current faction
by default. This phase should make faction id part of match setup, start payloads, replay metadata,
and client state without adding a second real faction or changing current gameplay.

## Scope

- Add a canonical default faction id for the current faction.
- Carry faction id through lobby player setup or match initialization.
- Add faction id to the simulation's player initialization/state boundary.
- Expose faction id in start payload player metadata so the client can know each player's faction.
- Preserve replay and branch playback compatibility by recording or defaulting faction ids.
- Update protocol docs and mirrors in the same commit as any wire contract changes.
- Keep UI selection minimal: either hidden/defaulted, or a disabled/internal fixture only.

## Expected Touch Points

- `server/crates/sim/src/game/mod.rs`
- `server/crates/sim/src/game/setup.rs`
- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `server/src/lobby/`
- `client/src/protocol.js`
- `client/src/state.js`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`

## Verification

- Rust tests proving `Game::new` defaults every player to the current faction.
- Protocol parity tests for the added start-player faction field.
- Replay/branch or serialization tests proving old/default faction data is stable.
- Focused server integration test that a normal match starts with unchanged starting entities,
  resources, and supply.

## Manual Testing Focus

Start a normal local match and confirm the current faction behaves exactly as before: City Centre,
Workers, visible resources, command card, training, and gathering should be unchanged.

## Handoff Expectations

The handoff must name the faction id field locations, whether any replay defaulting behavior was
added, and the exact tests that prove current starts are unchanged. It should tell Phase 2 where to
attach faction-aware rules catalog lookups.

## Player-Facing Outcome

No intended gameplay change. The current faction now has explicit identity that later phases can
use to select tech trees and loadouts.

