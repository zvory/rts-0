# Phase 3 - Steel/Oil Resource Policy Hardening

Status: Designed, not implemented.

## Objective

Keep Steel, Oil, and Supply as the shared economy contract for all factions in this plan, and make
that decision explicit in code, docs, and tests. Current-faction gameplay and wire resource payloads
should remain unchanged while command validation, costs, supply, scoring, replay analysis, and HUD
mirrors become ready for faction-specific catalogs that still price things in Steel/Oil/Supply.

## Scope

- Document Steel, Oil, and Supply as the global faction economy contract for this plan.
- Keep fixed `steel`, `oil`, `supplyUsed`, and `supplyCap` snapshot/player-resource payloads.
- Keep start-payload map resource nodes as Steel/Oil nodes.
- Keep replay artifacts, match-history replay payloads, branch keyframes, replay analysis, score
  screens, and compact snapshots on the current Steel/Oil/Supply schema.
- Move or wrap cost, affordability, spend, refund, and supply helpers so gameplay callers can pass
  faction-catalog cost data while still using Steel/Oil/Supply fields.
- Add server-side validation proving a player can only spend resources on kinds, upgrades, and
  abilities legal for that player's faction.
- Add tests or architecture-check notes identifying the approved modules where direct Steel/Oil/
  Supply references are expected, so future generic-resource work has a clear migration inventory.
- Keep prediction/WASM unchanged for the current faction and disabled for non-default factions
  unless an explicit later phase updates it.
- Add a short deferred-generic-resources note to the relevant design docs if this phase edits them.

## Expected Touch Points

- `server/crates/rules/src/`
- `server/crates/sim/src/game/player_state.rs`
- `server/crates/sim/src/game/services/economy.rs`
- `server/crates/sim/src/game/services/construction.rs`
- `server/crates/sim/src/game/services/production.rs`
- `server/crates/sim/src/game/scoring.rs`
- `server/crates/sim/src/game/analysis.rs`
- `client/src/config.js`
- `client/src/hud_command_card.js`
- `client/src/replay_analysis_overlay.js`
- `tests/sim_wasm_smoke.mjs`
- `docs/design/balance.md`
- `docs/design/protocol.md` only if documenting the deferred generic-resource decision there

## Verification

- Rust tests proving current-faction Steel/Oil/Supply start, spend, refund, supply reservation, and
  score behavior remain unchanged.
- Rust command tests proving faction-illegal build/train/research/ability attempts do not spend
  Steel/Oil or reserve Supply.
- Rust or JS tests proving replay analysis and score values still report Steel/Oil consistently.
- Client command-card tests proving current-faction costs, affordability, and shortage display are
  unchanged.
- Prediction/WASM smoke or client contract proving non-default factions remain prediction-disabled
  unless explicitly supported.

## Manual Testing Focus

Start a normal current-faction match and verify Steel, Oil, Supply, gathering, spending, training,
researching, spectator resource rows, score screens, and replay-analysis display still look correct.

## Handoff Expectations

The handoff must document that Steel/Oil/Supply remain the resource payload shape, name the approved
direct-resource modules, list any helper wrappers added for faction-catalog costs, and tell Phase 4
how to define faction starting Steel/Oil/Supply and supply rules.

## Player-Facing Outcome

The current faction should look and play unchanged. The faction architecture now explicitly supports
different costs and starts within the existing Steel/Oil/Supply economy, with truly generic
resources deferred to a later standalone migration.
