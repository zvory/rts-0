# Phase 0 - Inventory and Contract Decision

Status: Done.

## Goal

Map the existing selected-unit cap, command hardening, mirrored balance data, and selected-panel UI
before changing behavior. Decide whether this rollout should use a manual mirrored constant/helper
or introduce generated client configuration for command-budget values.

## Scope

- Inventory client selection admission paths:
  - `client/src/input/selection.js`
  - `client/src/input/control_groups.js`
  - any `GameState` selection/control-group helpers
  - command composition paths under `client/src/input/` and `client/src/command_composer.js`
  - minimap and HUD command issuing paths that can send selected unit ids
- Inventory current count-cap sites, including both helper-level `.slice(0, 12)` caps and
  `GameState.MAX_SELECTION_SIZE` enforcement in selection/control-group helpers.
- Inventory selected-panel rendering in `client/src/hud.js` and related CSS.
- Inventory server command validation in `server/crates/sim/src/game/services/commands.rs`,
  including `MAX_UNITS_PER_COMMAND`, `dedupe_cap_units`, planner facts, and every `SimCommand`
  variant that carries unit ids.
- Inventory the command source seam for human, AI, replay, and dev-scenario commands, including
  `Game::enqueue`, room-task live AI enqueueing, and any replay/dev harness enqueue paths.
- Inventory balance/config mirrors:
  - `server/crates/rules/src/balance.rs`
  - `server/src/config.rs`
  - `client/src/config.js`
  - any existing dump or parity tools.
- Identify focused tests that already cover selection, command-card context, protocol parity, and
  command hardening.

## Expected Deliverables

- A short note in this phase file listing:
  - every old 12-unit cap site found
  - every server command unit-list validation site found
  - every client command-send path that must be guarded before or with server rejection
  - how Phase 1 should distinguish or explicitly handle live AI commands
  - the recommended mirror strategy for command-budget constants and weights
  - the tests that later phases should extend
- No gameplay behavior changes.
- No UI behavior changes.

## Inventory Notes

### Old 12-unit cap sites

- `client/src/input/selection.js` has two helper-level `.slice(0, 12)` caps:
  `_closestOwnUnitKindInViewport` caps ctrl/double-click same-kind unit selection after distance
  ordering, and `_closestIdsToPoint` caps drag-box unit selection after anchor-distance ordering.
- `client/src/state.js` defines `GameState.MAX_SELECTION_SIZE` as `12`.
- `client/src/state.js` enforces `GameState.MAX_SELECTION_SIZE` in `setSelection` and
  `addToSelection`, so direct click replacement/additive selection, drag selection, same-kind
  selection, and any helper that calls these methods inherits the count cap.
- `client/src/state.js` also enforces the same cap in control groups:
  `setControlGroup`, `addToControlGroup`, and `_pruneControlGroup`.
- Existing tests encode the old count behavior in `tests/client_contracts.mjs`, especially the
  control-group assertions that groups store own units/buildings only "up to 12" and ignore
  overflow when full.

### Server command unit-list validation sites

- `server/crates/sim/src/game/services/commands.rs` defines `MAX_UNITS_PER_COMMAND = 256` as the
  absolute per-command unique-id work bound.
- `dedupe_cap_units` is the central count-only validation helper. It preserves first-seen id order,
  drops duplicate ids, and truncates at `MAX_UNITS_PER_COMMAND`; it does not currently reject
  over-budget commands.
- Multi-unit command variants that feed planner validation through `planner_facts` and
  `apply_planned_unit_order`: `Move`, `AttackMove`, `Attack`, `SetupAntiTankGuns`, `Gather`, and
  `Build`.
- Multi-unit command variants that directly iterate `dedupe_cap_units`: `TearDownAntiTankGuns`,
  `SetAutocast`, and `Stop`.
- `UseAbility` has two validation paths: Artillery `PointFire` directly iterates
  `dedupe_cap_units`, while the generic ability path may dedupe to choose a caster and then passes
  units into planner facts/order planning.
- `Train`, `Research`, `Cancel`, `SetRally`, and `Rejected` do not carry unit id lists and should
  stay outside command-budget validation.
- The order planner also has its own `max_units_per_command` config and dedupe path, but Phase 1
  should validate/reject at the command-service seam before planner trimming can silently narrow a
  hostile payload.

### Client command-send paths to guard

- `client/src/input/commands.js` issues selected-unit lists for viewport right-click commands:
  move, attack, gather, resume build, setup anti-tank guns, targeted move/attack-move, and
  world-point abilities.
- `client/src/minimap.js` issues selected-unit lists from `_issueOrder` for minimap move,
  attack-move, and world-point abilities.
- `client/src/hud.js` dispatches command-card intents carrying selected unit ids for `stop`,
  `setAutocast`, and ability intents.
- Production/rally/building-only commands from HUD and minimap (`train`, `research`, `cancel`,
  `setRally`) do not need unit-list budget checks, but the guard should leave them unchanged.
- Phase 1 should add a narrow send-side guard at the common issue-command boundary if practical;
  otherwise guard the three listed call surfaces before they send. Do not replace full selection
  admission until Phase 2.

### Command-source policy for Phase 1

- Human commands enter through `ClientMessage::Command` in `server/src/main.rs`, become
  `RoomEvent::Command`, and are enqueued in `server/src/lobby/room_task.rs` with
  `game.enqueue(seat_id, cmd)`.
- Live AI commands are produced in `server/crates/ai/src/live.rs` and enqueued from the room task
  with the same `Game::enqueue(player_id, command)` seam as humans.
- Replay playback enqueues recorded commands in `server/src/lobby/room_task.rs` through
  `ReplaySession::enqueue_for_current_tick`; self-play and replay validation paths under
  `server/crates/ai/src/selfplay/` also call `Game::enqueue` directly.
- Phase 1 should not change the public `Game::enqueue(player, cmd)` API unless the phase is revised
  to authorize a `CommandSource` contract. Recommended policy for this rollout: apply the same
  command budget to all `Game::enqueue` callers, including live AI, replay, self-play, and
  dev-scenario commands, and preserve `MAX_UNITS_PER_COMMAND` as the absolute hardening cap. This
  keeps Phase 1 inside the existing seam and avoids an unauthorised cross-file source-metadata
  contract. If AI needs exemption later, plan it explicitly.

### Mirror strategy

- Use manual mirrored constants/helpers first. Add command-budget constants next to existing
  balance/config mirrors rather than introducing generated JS/JSON config in this rollout.
- Authoritative weights should come from existing Rust unit supply values:
  `server/crates/rules/src/defs.rs` owns `UnitDef.stats.supply`, exposed through
  `server/crates/rules/src/balance.rs`/`server/src/config.rs`; the client mirror is
  `client/src/config.js` `STATS[kind].supply`.
- New command-budget constants should be mirrored deliberately in Rust and JS, for example base
  budget `24`, Command Car bonus `12`, and fallback selectable weight `1`. Add focused parity
  checks for these constants in later phases.
- No existing generated dump covers general stat/config generation.
  `scripts/check-faction-catalog-parity.mjs` verifies faction catalog data, not a complete
  generated balance mirror, so generation would be new infrastructure and is not justified for this
  narrow rollout.

### Focused tests later phases should extend

- Server hardening: extend `tests/regression.mjs`, which already covers malformed command metadata,
  heavily duplicated `units[]`, and oversized command frames.
- Server command behavior: add focused Rust tests in
  `server/crates/sim/src/game/services/commands.rs` for budget rejection, duplicate handling, and
  Command Car stacking.
- Client selection/control groups/HUD: extend `tests/client_contracts.mjs`, which already covers
  `GameState` selection/control-group helpers, team selection filtering, viewport command sends,
  command-card selected ids, same-kind selection helpers, and control-group hotkeys.
- Minimap command sends: extend `tests/minimap_input_contracts.mjs`, which already covers minimap
  move/attack-move/ability-like selected-id sends and replay command suppression.
- Mirror/parity checks: extend `tests/client_contracts.mjs` for mirrored `STATS[*].supply` and new
  command-budget constants; use `tests/protocol_parity.mjs` only if Phase 1 changes wire protocol.

## Verification

- Run only read-only or docs-focused checks needed for confidence, such as `rg` inventories.
- If the phase only updates this plan document, no automated suite is required.

## Manual Testing Focus

None. This is an inventory phase with no intended player-facing change.

## Handoff Expectations

The handoff must name the chosen mirror strategy, the exact files Phase 1 should edit for server
budget validation, the client send paths Phase 1 must guard, and the command-source policy Phase 1
must implement or deliberately revise.
