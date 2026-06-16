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
- Update `docs/design/client-ui.md` and context docs with the final module map.
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
