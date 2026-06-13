# Phase 3 - Generic Resource Contract

Status: Designed, not implemented.

## Objective

Replace fixed steel/oil economy payload assumptions with a generic resource contract that can
represent completely different faction economies. Current-faction gameplay should remain the same,
but old replay artifacts, compact snapshot versions, and old clients may break.

## Scope

- Introduce typed resource ids and definitions in the Rust-authoritative rules/faction catalog.
- Represent current Steel, Oil, and Supply through the new resource contract.
- Decide the exact semantic split between spendable resources and capacity resources such as
  supply. Supply may remain a specialized resource type, but it should still be faction-defined.
- Replace or wrap fixed `steel`, `oil`, `supplyUsed`, and `supplyCap` snapshot/player-resource
  assumptions with generic resource payloads.
- Bump compact snapshot and prediction protocol versions as needed without migration shims.
- Update replay artifacts, replay start metadata, match-history replay serialization, branch
  keyframes, and dev session starts to use the new schema.
- Update HUD/resource rendering for the current faction through the generic payload while keeping
  player-visible Steel/Oil/Supply presentation unchanged.
- Add server-side affordability/spend/refund helpers that operate on resource ids instead of
  steel/oil pairs.
- Add architecture-check pressure against new direct `steel`/`oil` economy fields outside approved
  compatibility shims during migration.
- Disable prediction for non-default or generic-resource fixture factions until the WASM adapter is
  intentionally updated.

## Expected Touch Points

- `server/crates/rules/src/`
- `server/crates/sim/src/game/player_state.rs`
- `server/crates/sim/src/game/services/economy.rs`
- `server/crates/sim/src/game/services/construction.rs`
- `server/crates/sim/src/game/services/production.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/sim/src/game/replay.rs`
- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/lib.rs`
- `server/src/lobby/`
- `client/src/protocol.js`
- `client/src/state.js`
- `client/src/hud.js`
- `client/src/config.js`
- `tests/protocol_parity.mjs`
- `tests/sim_wasm_smoke.mjs`
- `docs/design/protocol.md`
- `docs/design/balance.md`

## Verification

- Rust tests proving current-faction Steel/Oil/Supply values match the old start and spend behavior.
- Rust tests for generic affordability, spending, refunding, capacity checks, and spectator
  per-player resource projection.
- Protocol parity tests for resource ids, generic resource payloads, and compact snapshot version.
- Client HUD/resource tests proving current-faction resource display is unchanged.
- Replay/branch serialization tests proving the new schema round-trips; no old replay compatibility
  test is required.
- Prediction/WASM test or client contract proving unsupported faction/resource payloads disable
  prediction with a clear reason.

## Manual Testing Focus

Start a normal current-faction match and verify Steel, Oil, Supply, gathering, spending, training,
researching, spectator resource rows, and post-match replay display still look correct.

## Handoff Expectations

The handoff must document the resource payload shape, compact snapshot version change, replay schema
change, which fixed steel/oil helpers remain temporarily, and how Phase 4 should define faction
starting resources and supply rules.

## Player-Facing Outcome

The current faction should look and play unchanged, but the underlying economy contract can now
represent factions that do not use Steel/Oil.
