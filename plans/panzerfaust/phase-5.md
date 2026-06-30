# Phase 5 - Production Exposure And Command Card

## Phase Status

Status: pending.

## Objective

Expose the completed Panzerfaust through normal Kriegsia production once the hidden runtime and
client readability are in place. This is the first phase where ordinary players should be able to
train the unit in a normal match.

## Scope

- Add Panzerfaust to the current faction's normal trainable unit catalog.
- Expose Panzerfaust from Barracks after the owner has a completed Training Centre.
- Add the train command-card button:
  - Hotkey `E`, preserving Rifleman `Q` and Machine Gunner `W`.
  - Approved short description and tooltip from [checklist.md](checklist.md).
  - Cost display: 60 steel / 15 oil.
  - Supply display: 1 supply.
  - Build time display: 400 ticks or the existing UI format equivalent.
  - Disabled states for missing Training Centre, insufficient resources, insufficient supply,
    non-owned selection, and non-commandable contexts.
- Ensure server production semantics are correct:
  - Training queue admission requires affordability, supply, owner, faction, and completed
    Training Centre.
  - Queue cancellation and refund use the normal Barracks training rules.
  - Rally, spawn placement, death cleanup, supply reservation, and rematch cleanup follow existing
    unit-production behavior.
- Ensure client mirrors and generated surfaces agree with Rust:
  - Config facade and internal rules mirror.
  - Faction catalog dump.
  - HUD command card descriptors.
  - Wiki/stats surfaces where applicable.
- Before exposing the unit, add or verify a minimal non-misleading audio mapping. Phase 6 owns the
  richer polish pass, but Phase 5 must not let Panzerfaust attacks fall through to obviously wrong
  Rifleman, Tank cannon, artillery, or debug feedback once normal players can train the unit.
- Keep AI from training Panzerfaust units in the first implementation pass.
- Confirm AI-owned Panzerfaust units spawned by lab/dev setup can still use the Phase 3 acquisition
  behavior.
- Collect player-facing patch-note bullets for the unit becoming trainable.

## Expected Touch Points

- `server/crates/rules/src/defs.rs`
- `server/crates/rules/src/faction.rs`
- `server/crates/rules/src/economy.rs`
- `server/crates/rules/src/bin/dump-faction-catalog.rs`
- `server/crates/sim/src/game/services/production.rs`
- `server/crates/sim/src/game/services/commands/tests/production.rs`
- `server/crates/ai/src/*.rs`
- `client/src/config.js`
- `client/src/config/*.js`
- `client/src/hud.js`
- `client/src/hud_command_card.js`
- `client/src/hotkey_profiles.js`
- `tests/hud_command_card.mjs`
- `tests/client_contracts/hud_contracts.mjs`
- `tests/client_contracts/config_contracts.mjs`
- `docs/design/balance.md`
- `docs/design/client-ui.md`
- `plans/panzerfaust/checklist.md`

## Edge Cases To Cover

- Barracks without a completed Training Centre does not train Panzerfaust and shows the intended
  disabled state.
- Barracks with a completed Training Centre shows Panzerfaust in the expected slot with hotkey `E`.
- Training fails safely when resources or supply are insufficient.
- Queue cancellation refunds correctly.
- Multiple selected Barracks use existing round-robin train behavior.
- The unit does not appear for the wrong faction or hidden fixture catalogs.
- AI build orders do not start producing Panzerfaust units.
- A freshly trained Panzerfaust can fire once and convert through the Phase 3 runtime.
- A freshly trained Panzerfaust uses at least a deliberate placeholder or silence for attack audio,
  not an accidental fallback to another weapon's sound.

## Verification

- Focused Rust tests for faction catalog trainability, Training Centre prerequisite, affordability,
  supply reservation, queue completion, cancellation/refund, and AI build exclusion.
- Focused HUD/config tests for command-card descriptor, hotkey, disabled state, tooltip, and cost.
- `node scripts/check-faction-catalog-parity.mjs`.
- `node scripts/check-wiki.mjs`.
- `node tests/hud_command_card.mjs`.
- `node tests/client_contracts/config_contracts.mjs`.
- `node scripts/check-client-architecture.mjs` if client module boundaries change.
- A focused live server/client integration test if production command behavior crosses the wire in a
  way not covered by Rust tests.
- `git diff --check`.

## Manual Test Focus

Start a normal match, build a Barracks and Training Centre, train a Panzerfaust with the command card
and hotkey, cancel one queued Panzerfaust, then train another to completion. Confirm it consumes the
right resources and supply, spawns from Barracks, fires once at a Tank, uses non-misleading
feedback, converts into a Rifleman, and no AI opponent starts training Panzerfaust units.

## Handoff Expectations

Name the production tests, command-card tests, catalog/wiki checks, and any UI strings changed.
Tell Phase 6 which launch, impact, conversion, selection, or command acknowledgement feedback still
needs deliberate audio or visual polish.
