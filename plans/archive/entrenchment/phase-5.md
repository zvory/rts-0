# Phase 5 - Client Rendering, UX Polish, And Hardening

## Phase Status

Status: done.

## Objective

Make authoritative trenches readable in the client and harden the complete feature across
multiplayer, fog, replay, reconnect, and docs. This phase should deliver persistent brown trench
ground, supporting UI/status affordances, and a practical first-pass occupied-unit visual indicator.

## Scope

- Render trench ground from authoritative trench snapshots, not from local inference.
- Use a world layer below units and above or near terrain/ground decals so trench ground persists
  after the unit leaves and remains readable under fog.
- Visually connect nearby trenches into a continuous brown trench area where practical.
- Keep trench rendering deterministic and bounded. Avoid per-frame unbounded display-object churn
  when many trenches exist.
- Add minimal HUD or selection-panel text/status needed to explain researched availability,
  dig-in/occupied state, and combat benefits.
- Add a modest occupied-unit visual indicator using the implementing agent's judgment. Prefer a
  simple, readable, low-risk treatment over a complex art direction; it can be refined after the
  feature is shipped.
- Ensure rematch teardown clears trench rendering resources and snapshot state.
- Add dev/lab scenario coverage or fixtures that let humans inspect newly created trenches,
  existing neutral trench reuse, fogged trenches, and crowded slotting.
- Harden reconnect/replay behavior so existing trenches are restored from server snapshots and are
  not lost like client-only death decals.
- Update `docs/design/client-ui.md`, `docs/design/protocol.md`, `docs/design/server-sim.md`, and
  `docs/design/balance.md` with the final player-facing and wire behavior.

## Expected Touch Points

- `client/src/state.js`
- `client/src/renderer/index.js`
- `client/src/renderer/decals.js` or a new renderer trench layer
- `client/src/renderer/layers.js`
- `client/src/renderer/entities.js`
- `client/src/hud.js`
- `client/src/hud_selection_panel.js`
- `client/src/protocol_snapshot.js`
- `client/src/config/rules_mirror.js`
- `client/assets/decals/` or another existing client asset path if bitmap/SVG masks are used
- `server/crates/sim/src/game/setup/dev_scenarios.rs`
- `tests/client_contracts/`
- `docs/design/client-ui.md`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/balance.md`

## Verification

- Focused client contract tests for snapshot trench state, renderer layer behavior, teardown, and
  selection/status refresh.
- `node scripts/check-client-architecture.mjs`.
- `node tests/client_contracts/protocol_contracts.mjs`.
- `node tests/client_contracts/renderer_feedback_contracts.mjs` or a focused renderer contract
  test if available.
- `node tests/protocol_parity.mjs` if rendering work requires final protocol tweaks.
- `node scripts/check-faction-catalog-parity.mjs` and `node scripts/check-wiki.mjs` if UI-visible
  rules metadata changes.
- A focused Rust integration or dev-scenario test if fixture setup changes.
- `git diff --check`.

## Manual Test Focus

Play a match long enough to research Entrenchment, dig several nearby trenches, move away, and
return with friendly and enemy eligible infantry. Confirm brown trench ground persists, nearby
trenches read as connected, fog/replay/reconnect behavior is stable, and the first-pass
occupied-unit indicator is readable without obscuring normal unit state.

## Handoff Expectations

Summarize the final rendering layer, asset approach, teardown behavior, and manual scenarios that
passed. Include the complete patch-note bullets for the feature and clearly state whether the
occupied-unit visual treatment is a provisional first pass.
