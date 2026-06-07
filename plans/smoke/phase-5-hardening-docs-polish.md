# Phase 5: Hardening, Docs, and Polish

## Objective

Lock down edge cases, update design docs, add regression coverage, and then polish the visible
smoke experience without expanding gameplay scope.

## Hardening

- Bound all ability unit lists through existing caps.
- Reject non-finite coordinates and out-of-map targets safely.
- Ensure smoke cloud ids never collide with entity ids if they share any client-side maps.
- Cap active smoke clouds or prove command cooldown/resources make unbounded cloud counts
  impossible under realistic load.
- Ensure no `unwrap`, `expect`, or unchecked indexing is added to `Game::tick()` paths.
- Verify stale queued ability orders skip safely when the caster dies, cooldown state changes, tech
  disappears, target point is invalid, or pathing fails.

## Documentation

- Update `docs/design/protocol.md` for:
  - `useAbility`;
  - ability cooldown projection;
  - active smoke snapshot records;
  - compact snapshot version changes.
- Update `docs/design/balance.md` with Smoke:
  - carrier;
  - Steelworks requirement;
  - 25 steel / 25 oil cost;
  - 9-tile range;
  - 2-tile radius;
  - 5-second duration;
  - 20-second cooldown;
  - expected offensive role.
- Update `docs/design/server-sim.md` for:
  - ability definitions;
  - ability order execution;
  - smoke cloud world state;
  - dynamic LOS blockers.
- Update `docs/design/client-ui.md` for:
  - generic targeted ability mode;
  - range previews;
  - smoke rendering.
- Refresh context capsules only if section structure shifts.

## Polish

- Add localized notice strings if the current generic notices are not sufficient.
- Add distinct Smoke command feedback at the target point.
- Add audio hooks if the existing notice/command sound system has a clear extension point.
- Later-only unless cheap:
  - canister projectile visual;
  - terrain restrictions for water/stone target points;
  - smoke dissipate animation;
  - AI use of Smoke.

## Patch Notes to Capture

- Scout cars gain Smoke on hotkey `D`.
- Smoke costs 25 steel and 25 oil, requires Steelworks, has 9-tile range, 2-tile radius, 5-second
  duration, and 20-second cooldown.
- Smoke blocks line of sight and prevents targeting through or inside the cloud.
- Units inside smoke provide no vision, making Smoke an offensive tool for closing on long-range
  defenses.

## Verification

- `cd server && cargo test`
- Start server and run:
  - `node tests/server_integration.mjs`
  - `node tests/regression.mjs`
  - `node tests/ai_integration.mjs`
  - `node tests/minimap_input_contracts.mjs` if minimap targeting changes
  - `cd tests && npm install && node client_smoke.mjs` if UI/rendering changed
- Manually test a tank closing on a deployed AT gun under smoke.

## Done

- All relevant design docs and capsules are current.
- Automated tests cover the important visibility/combat/cooldown/queue edge cases.
- Smoke is playable without stale hidden targets, misleading fog, or tick-path panic risk.
