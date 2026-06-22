# Phase 1 - Quickstart Caller Migration

## Phase Status

- [ ] Pending.

## Objective

Move active tests, tooling, and client-facing compatibility seams off the legacy quickstart/debug
path before deleting the protocol command and server behavior.

## Work

- Audit active quickstart callers and assumptions with targeted searches for `setQuickstart`,
  `quickstart`, `QUICKSTART`, debug solo starts, debug loadouts, and owner-only debug diagnostics.
- Replace tri-state, regression, client-contract, and live Node setup that currently depends on
  quickstart with normal starts, existing dev scenarios, lab flows, or narrower unit/contract tests
  that prove the behavior without enabling the debug preset.
- Remove or rewrite tests whose only purpose is proving the legacy quickstart/debug path still works.
- Keep any temporary production code compatibility in place until Phase 2, but make the active tests
  no longer call `Net.setQuickstart`, send `setQuickstart`, or assert `lobby.quickstart`.
- Keep normal solo-start behavior covered separately from quickstart. A solo room may still skip
  countdown, but the test should assert normal setup and normal resources.
- Leave archived plans and historical incident logs alone unless an active checker reads them.
- Record any compatibility callers that cannot be migrated cleanly as explicit blockers before Phase
  2 begins.

## Expected Touch Points

- `tests/tri_state/`
- `tests/regression.mjs`
- `tests/client_contracts/*.mjs`
- `tests/lobby_browser_integration.mjs` if lobby setup coverage references quickstart
- `client/src/lobby.js` only if the optional hidden quickstart DOM compatibility creates test
  coupling that should be removed before the protocol deletion
- `docs/design/testing.md` and `docs/context/testing.md` if test setup guidance changes

## Verification

- `rg -n "setQuickstart|quickstart: true|quickstart === true|debugSoloStart|QUICKSTART" tests client/src docs/design docs/context`
- `node tests/client_contracts.mjs`
- `node tests/select-suites.mjs --verify`
- `tests/run-all.sh --with-tri-state-browser --no-rust` if tri-state browser scenarios are changed
- `node tests/regression.mjs` if the regression quickstart path is rewritten and a local server is
  available
- `git diff --check`

If a live Node command is not run because no local server is available, state that explicitly in the
handoff and identify the PR gate that will cover it.

## Manual Test Focus

Open a normal one-player room and start it. Confirm it starts without the old Debug/quickstart setup:
normal resources, normal opening units/buildings, and no debug army or movement-diagnostic toggle.

## Handoff Expectations

List every quickstart caller that was removed or rewritten, name any remaining active references that
Phase 2 must delete, and state whether the quickstart protocol command is now unused by active tests.
