# Phase 3 - Lab Entry Replaces Visible Debug Mode

## Phase Status

- [x] Done.

## Objective

Make lab the player-facing experimentation path and stop presenting lobby Debug mode as a normal UI
control. This phase does not delete the legacy quickstart command or recreate the old Debug-mode
preset in lab.

## Work

- Remove or hide the normal lobby's visible `Debug mode` toggle.
- Add or refine the player-facing lab entry/share path where it fits the existing UI. This can be a
  simple link or route affordance; do not turn it into a landing page.
- Keep `/lab?room=<id>&map=<map>&seed=<seed>` as the direct shared URL contract unless a better
  existing route already covers it.
- Leave `setQuickstart` in the protocol/server as temporary internal/test compatibility. Do not
  break tri-state or regression scenarios that still rely on quickstart unless this phase also
  provides and verifies an explicit replacement.
- Update visible product wording and docs so users are directed to the lab for experimentation.
- Update docs to say the old debug preset is intentionally not part of this plan and should return
  later as a real lab preset/scenario feature.
- Keep match history `debug_mode` behavior and database schema unchanged unless the implementation
  proves the field is no longer written by any compatibility path.
- Add tests or selector coverage for the changed lobby UI surface.

## Expected Touch Points

- `client/index.html`
- `client/src/lobby.js`
- `client/src/lobby_view.js` or related lobby/browser view modules if the lab entry belongs there
- `client/src/bootstrap.js` if lab launch/share URL handling changes
- `client/styles.css`
- `client/src/net.js` only if visible quickstart methods are removed from client-facing seams
- `client/src/protocol.js` only if the wire command changes, which this phase should avoid by
  default
- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/design/match-history.md` only if quickstart persistence semantics change
- `tests/client_contracts.mjs`
- `tests/lobby_browser_integration.mjs` if the lobby browser surface changes
- `tests/regression.mjs` and `tests/tri_state/` only if quickstart compatibility is touched

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/protocol_parity.mjs` if protocol docs or mirrors change
- `node tests/select-suites.mjs --verify` if selector mappings need updates
- `git diff --check`

If quickstart compatibility code is touched, also run the narrow server or Node tests that still
exercise `setQuickstart`, such as:

- `cargo test --manifest-path server/Cargo.toml -p rts-server quickstart`
- `node tests/regression.mjs` if the live regression path is available

## Manual Test Focus

Open the normal lobby and confirm the old Debug mode toggle is no longer presented as the
experimentation path. Open or share a `/lab` URL and confirm it starts the lab workflow instead.

## Handoff Expectations

State exactly what visible UI changed, whether `setQuickstart` remains callable for compatibility,
and what follow-up plan should reintroduce debug-style presets as first-class lab scenarios.
