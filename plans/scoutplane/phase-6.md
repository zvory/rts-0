# Phase 6 - Production Exposure And City Centre Button Behavior

## Phase Status

Status: pending.

## Objective

Expose the completed Scout Plane through normal Kriegsia City Centre production. This is the first
phase where ordinary players should be able to build a Scout Plane in a normal match, so it must land
only after hidden server behavior, fog/upkeep, client controls, and rendering are all working.

## Scope

- Add Scout Plane to the current Kriegsia production/catalog surfaces:
  - produced from completed City Centre.
  - visible command-card button for factions that can use it.
  - button in grid slot `Z`.
  - grid hotkey `Z`, RTS classic hotkey `S`.
  - label `Scout Plane`.
  - cost display 50 Steel / 50 Oil.
  - 600-tick production time.
  - 0 supply.
- Implement unlock and disabled behavior:
  - visible but disabled until the player owns a completed Gun Works or completed Vehicle Works.
  - disabled reason: `Requires Gun Works or Vehicle Works.`
  - normal resource shortage feedback when resources are insufficient.
- Implement the one active or in-production limit end to end:
  - server rejects a second queued plane while one is active or already in any City Centre queue.
  - client command card shows the appropriate non-queueing behavior.
  - optimistic production and cancel/refund accounting remain correct.
  - cancellation before launch uses existing production cancellation/refund behavior.
- Implement City Centre button behavior:
  - with no active or in-production plane, queue production at the selected City Centre.
  - with an active plane, select the existing plane and pan the camera to it instead of sending a
    train command.
  - with a Scout Plane already in production, do not queue another plane.
  - if multiple City Centres are selected, preserve current train-button producer selection patterns
    except where the active/in-production limit overrides them.
- Keep AI from launching or commanding Scout Planes in the first implementation.
- Update generated/wiki/stats and design docs in this phase if production exposure changes visible
  catalog, balance, or protocol surfaces.
- Do not add aircraft combat, anti-air, repair, crash, transport, bombing, final audio, or AI usage.

## Expected Touch Points

- `server/crates/rules/src/defs.rs`
- `server/crates/rules/src/faction.rs`
- `server/crates/rules/src/economy.rs`
- `server/crates/rules/src/bin/dump-faction-catalog.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/production.rs`
- `server/crates/ai/src/`
- `client/src/config.js`
- `client/src/config/*.js`
- `client/src/hud.js`
- `client/src/hud_command_card.js`
- `client/src/hud_command_dom.js`
- `client/src/hotkeys*.js`
- `client/src/match*.js`
- `client/src/camera.js`
- `tests/client_contracts/hud_contracts.mjs`
- `tests/client_contracts/config_contracts.mjs`
- `tests/hud_command_card.mjs`
- `docs/design/balance.md`
- `docs/design/client-ui.md`
- `docs/design/protocol.md` only if command shape or snapshot shape changes
- `plans/scoutplane/requirements.md` only if implementation discovers a requirement ambiguity

## Edge Cases To Cover

- City Centre button is visible but disabled before completed Gun Works or Vehicle Works.
- Either completed Gun Works or completed Vehicle Works unlocks the button.
- Incomplete Gun Works or Vehicle Works does not unlock the button.
- Queueing spends 50 Steel and 50 Oil and reserves 0 supply.
- Production completes after 600 ticks and launches using the Phase 3 behavior.
- Insufficient Steel or Oil emits normal resource shortage feedback.
- A second train request is rejected while a plane is active.
- A second train request is rejected while a plane is already in production.
- Pressing the button with an active plane selects and pans to that plane without spending resources.
- Pressing the button with a plane in production does not queue another or spend resources.
- Canceling queued Scout Plane production refunds using existing production behavior.
- Destroying the producing City Centre before completion uses existing production interruption
  behavior.
- Multiple selected City Centres do not bypass the one-active-or-in-production limit.
- AI catalogs/plans do not train or command Scout Planes.

## Verification

- Focused Rust tests for production prerequisite, cost, build time, supply, active limit,
  in-production limit, cancel/refund, and destroyed-producer behavior.
- Focused AI tests or catalog assertions proving AI does not train Scout Planes.
- Focused client config/catalog parity tests.
- Focused HUD command-card tests for slot, hotkeys, disabled reason, cost, active select/pan, and
  in-production non-queueing behavior.
- `node scripts/check-faction-catalog-parity.mjs`.
- `node scripts/check-wiki.mjs`.
- `node scripts/check-client-architecture.mjs` if client module wiring changes.
- `tests/run-all.sh --no-rust` if live client/server production behavior changes need browser smoke.
- `git diff --check`.

## Manual Test Focus

In a normal local match, build Gun Works or Vehicle Works, train one Scout Plane from City Centre,
confirm resources are spent and it launches to the rally point, then press the button again while it
is active and confirm selection/pan instead of a second queue. Repeat while one is in production,
with insufficient Oil, with multiple City Centres selected, and after canceling queued production.

## Handoff Expectations

Name the final production/catalog files, active-limit helper, City Centre button behavior, hotkey
mapping, AI exclusion path, and manual normal-match test path. Call out any known roughness in
select/pan, optimistic UI, or production cancellation that Phase 7 should harden.
