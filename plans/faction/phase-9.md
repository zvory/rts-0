# Phase 9 - Second Faction Start and Economy Slice

Status: Designed, not implemented.

## Objective

Implement the second faction's start, resource model, and first production path from the approved
brief/spec. This phase should produce a narrow playable economy slice, not the full faction.

## Scope

- Add the real second faction id and Rust catalog entry.
- Add the approved starting loadout, starting resources, supply/capacity rules, and first production
  anchor.
- Add the minimum builder/producer path or equivalent mechanic needed to create one basic unit path.
- Add client resource display, command-card entries, hotkeys, and readable placeholder/final art for
  the start and first production path.
- Keep server catalog strictness: reject current-faction build/train/research/economy commands from
  second-faction players and reject second-faction commands from current-faction players.
- Keep AI unable to select or be assigned the second faction.
- Disable prediction for the second faction unless the approved brief and implementation explicitly
  add WASM support.
- Collect factual patch-note bullets for resource, start, production, and UI changes.

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

- Rust tests for second-faction start loadout, resources, supply/capacity, and production legality.
- Rust command tests for illegal cross-faction commands in both directions.
- Protocol parity tests for new faction/resource/kind ids touched by this slice.
- Client command-card descriptor tests for start/economy/production commands.
- Server integration test for mixed current-faction plus second-faction match start.
- Prediction-disable test for second-faction starts if WASM is not updated.
- Balance docs updated with player-facing start/economy/production data.

## Manual Testing Focus

Start a local match as the second faction and verify start entities, resource display, first
production path, command legality, AI restriction, and prediction-disabled state. Also start a
current-faction match and verify the original start and economy were not regressed.

## Handoff Expectations

The handoff must include patch-note bullets, the implemented start/economy/production list, tests
run, known limitations, and exactly what Phase 10 may add next.

## Player-Facing Outcome

Players can enter a dev/local match as the new faction and exercise its starting economy and first
production path.
