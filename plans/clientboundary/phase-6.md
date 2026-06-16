# Phase 6 - Remove Shims And Tighten Policy

## Phase Status

- [ ] Not implemented.

## Objective

Remove temporary compatibility shims and enforce the new client boundaries.

## Work

- Remove temporary `GameState` compatibility fields and methods once HUD, input, minimap, renderer,
  and tests use explicit facades.
- Update `scripts/check-client-architecture.mjs` classifications, allowlists, and large-file
  baseline expectations.
- Extend the checker with forbidden direct intent shim reads/writes outside allowlisted
  compatibility tests. At minimum, flag production references to `state.commandTarget`,
  `state.placement`, `state.commandCardMode`, `state.resourceMiningPreview`,
  `state.antiTankGunSetupPreview`, `state.abilityTargetPreview`, `state.liveCommandFeedback(...)`,
  and GameState intent shim methods from HUD/input/minimap/renderer.
- Update `docs/design/client-ui.md` and context docs with the final module map.
  Ensure `client_intent.js` is documented in the model area and stale `GameState` intent-shim
  language is removed.
- Keep this phase cleanup-only.

## Expected Touch Points

- `client/src/state.js`
- New helper files from earlier phases
- `scripts/check-client-architecture.mjs`
- `docs/design/client-ui.md`
- Affected tests

## Implementation Checklist

- [ ] Remove compatibility reads and methods.
- [ ] Tighten architecture checker policy.
- [ ] Update docs with final boundaries.
- [ ] Verify suite selection rules.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `node scripts/check-client-architecture.mjs`
- `node tests/select-suites.mjs --verify`
- Focused client tests selected by changed files
- Commit hook for the final merge-ready commit

## Manual Test Focus

Full live match pass for selection, command card, minimap, build placement, replay/spectator
teardown, and rematch.

## Handoff Expectations

Provide a before/after boundary map, remaining large-file risks, and future extraction candidates.
