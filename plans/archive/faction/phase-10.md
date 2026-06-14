# Phase 10 - Second Faction Start and Economy Slice

Status: Blocked until Phase 9 approves a hero-centric Ekat rules spec.

## Objective

Implement only the first Phase 9-approved hero-centric Ekat slice. The old RTS-style
start/economy/production path was purged and must not be recreated without explicit approval.

## Scope

- Use the existing reserved `ekat` id only if the approved spec says the slice is ready to run.
- Expose Ekat only through the Phase 9-approved assignment path unless this phase explicitly
  implements normal lobby selection.
- Add only the approved hero start state, controls, progression/economy, abilities, and supporting
  entities.
- Do not add workers, base buildings, supply structures, production buildings, or trainable infantry
  unless the user explicitly approves them by name in the new spec.
- Add client command-card/control entries, hotkeys, and readable placeholder/final art for the
  approved hero slice.
- Keep server catalog strictness: reject current-faction build/train/research/economy commands from
  second-faction players and reject second-faction commands from current-faction players.
- Keep AI unable to select or be assigned the second faction.
- Disable prediction for the second faction unless the approved brief and implementation explicitly
  add WASM support.
- Update the lifecycle matrix with the real faction start path, AI rejection behavior, replay
  reconstruction behavior, and prediction-disabled state.
- Collect factual patch-note bullets for Steel/Oil/Supply costs, start, production, and UI changes.

## Expected Touch Points

- `server/crates/rules/src/`
- `server/crates/sim/src/game/`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `server/src/lobby/`
- `client/src/protocol.js`
- `client/src/config.js`
- `client/src/hud.js`
- `client/src/hud_command_card.js`
- `client/src/renderer/`
- generated or checked client catalog artifacts/scripts
- `docs/design/balance.md`
- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/design/server-sim.md`

## Verification

- Rust tests for the approved hero start state, commands, progression/economy, and legality.
- Rust command tests for illegal cross-faction commands in both directions.
- Protocol parity tests for any new faction/kind/ability/event ids touched by this slice.
- Client command-card/control descriptor tests for the approved hero actions.
- Server integration test for the approved Ekat start path.
- Prediction-disable test for Ekat starts if WASM is not updated.
- Replay/branch schema test for the Ekat start slice if replay or branch start is exposed.
- Balance/design docs updated with player-facing hero stats and progression/economy behavior.

## Manual Testing Focus

Start a local match as Ekat and verify the approved hero controls, start state,
progression/economy, command legality, AI restriction, and prediction-disabled state. Also start a
Kriegsia match and verify the original RTS start and economy were not regressed.

## Handoff Expectations

The handoff must include patch-note bullets, the implemented hero-slice details, the assignment path
used, lifecycle matrix updates, tests run, known limitations, and exactly what Phase 11 may add next.

## Player-Facing Outcome

Players can enter the approved dev/local Ekat path and exercise the first hero-centric slice.
