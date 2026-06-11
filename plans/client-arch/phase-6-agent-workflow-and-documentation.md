# Phase 6 - Agent Workflow and Documentation

## Objective

Make the client architecture rules visible to future agents and cheap to follow. This phase turns
the new checks and extracted seams into everyday workflow, not tribal knowledge.

## Work

- Add or update a short client architecture section in `docs/design/client-ui.md`.
- Refresh `docs/context/client-ui.md` with:
  - the current module map
  - the architecture checker command
  - rules for allowed cross-area imports
  - the expectation that UI refactors need programmatic contract coverage
- Add a small checklist for future client changes:
  - Did this add a listener, timer, WebSocket handler, or GPU resource? Add/update `destroy()`.
  - Did this add a non-shell cross-area import? Prefer DI through `Match` or update the checker
    allowlist with a reason.
  - Did this change command-card behavior? Add descriptor or DOM contract coverage.
  - Did this change rendering? Run client smoke and add a targeted check where possible.
  - Did this touch `protocol.js` or `config.js`? Update the mirrored server/docs files.
- Document how to handle intentionally large files:
  - do not churn them just to reduce line count
  - when adding features, prefer extracting a focused helper or collaborator
  - update/baseline ratchets only with a reason
- If Phase 1 added suite selection, document which suite names should run for client changes.

## Implementation Segments

Mark each segment complete as it lands:

- [ ] Update `docs/design/client-ui.md` with the current architecture rules.
- [ ] Refresh `docs/context/client-ui.md` with the current module map and checker command.
- [ ] Add the future-client-change checklist covering teardown, imports, command cards, rendering,
  and mirrored contracts.
- [ ] Document large-file ratchet handling and suite-selection expectations.
- [ ] Verify docs links and record the commands/results in the final handoff.

## Verification

- `node scripts/check-client-architecture.mjs` if Phase 1 has landed
- `node tests/select-suites.mjs --verify`
- Documentation links in `docs/context/client-ui.md` resolve to existing files.

## Manual Test Prompt

No manual UI test should be required for this phase unless runtime client files changed. At handoff,
tell the user:

> Manual testing: none expected for docs-only workflow updates. If runtime client code changed,
> manually smoke only that changed surface and explain why it was touched in this documentation
> phase.

## Safety Notes

This phase should not change runtime code except for small test-selector/checker polish. Keep docs
specific enough to guide agents, but short enough that they will actually read them before changing
client code.

## Outcome

No gameplay or visual change. Future client work starts with clearer local rules, and the repo has a
repeatable path for improving architecture without risky UI rewrites.
