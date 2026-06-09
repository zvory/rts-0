# Phase 1 - Client Architecture Checker

## Objective

Add a lightweight architecture checker for `client/src` that prevents new coupling while accepting
the current client shape. This phase should not change runtime client behavior.

## Work

- Add `scripts/check-client-architecture.mjs`.
- Parse static ES module imports under `client/src`.
- Classify modules into coarse areas:
  - `app-shell`: `main.js`, `app.js`, `match.js`, `replay_viewer.js`
  - `model`: `state.js`
  - `transport`: `net.js`, `protocol.js`
  - `rules-mirror`: `config.js`
  - `renderer`: `renderer/**`
  - `input`: `input/**`
  - `ui`: `hud.js`, `lobby.js`, `match_history.js`, `status_badge.js`, `minimap.js`
  - `platform`: `bootstrap.js`, `audio.js`, `combat_audio.js`, `alerts.js`, `fog.js`, `camera.js`
- Enforce conservative import rules:
  - Any module may import `protocol.js` and `config.js`.
  - `app-shell` may wire collaborators from all areas.
  - Area-internal imports are allowed.
  - Non-shell cross-area imports must be explicitly allowlisted with a short reason.
  - New `model -> input` imports are forbidden.
- Grandfather the current `Object.assign(Input.prototype, ...)` and
  `Object.assign(Renderer.prototype, ...)` sites, but fail if new facade prototype grafting sites are
  added.
- Emit file-size and fan-in/fan-out data. Start as warnings or baseline data unless the current repo
  already has stable thresholds.
- Add the check to `tests/run-all.sh` near the existing architecture checks.
- Add a suite name such as `client-architecture` to `tests/select-suites.mjs`, selected for
  `client/src/**`, `scripts/check-client-architecture.mjs`, and this plan/check docs.

## Verification

- `node scripts/check-client-architecture.mjs`
- `node tests/select-suites.mjs --verify`
- `tests/run-all.sh --no-client` should include and pass the client architecture check.

## Safety Notes

This is the safest first investment because it only reads source files and updates test selection.
If the first checker needs a current-state allowlist, prefer explicit allowlist entries over
loosening the general rule. Each allowlist entry should describe why it exists and what future
change can remove it.

## Outcome

No gameplay or visual change. Future agents get an immediate, local failure when they add client
dependencies that violate the intended architecture.
