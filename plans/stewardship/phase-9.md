# Phase 9 - Roll Back Failed Match Startup

Status: Incomplete.

## Objective

Make match startup transactional across both Match-owned resources and App-owned session assembly.
After a failure, all allocated resources must be released, the application must return to a usable
screen state, and a later match start must succeed.

## Work

- Make `Match` catch failures inside its own construction, unwind every listener, timer, audio
  helper, DOM/Pixi resource, and partially initialized module it already owns, then rethrow or return
  a structured construction failure to App.
- Make teardown idempotent and safe when any normal Match field or listener callback has not yet been
  initialized. Ensure cleanup itself continues after one module's destroy hook throws.
- Make `App.onStart()` own the full assembly transaction: Lab client, Lab control policy, Match or
  ReplayViewer, Lab panel, and temporary screen/presentation changes.
- Preserve the visibility/layout conditions required to measure and initialize the game screen, but
  record and restore prior App/DOM state if assembly fails.
- Clean both locally staged and already assigned App resources after failure, show a concise startup
  error, and leave the lobby or prior usable shell able to start another session.
- Add failure injection late inside Match construction and after Match succeeds while LabPanel is
  constructed. After each failure, assert complete cleanup and a successful subsequent start.
- Preserve normal successful startup, rematch teardown, replay viewer startup, Lab startup, carried
  camera behavior, and Pixi/backend selection.

## Non-goals

- Do not change command interaction completed in Phase 8.
- Do not change `Net` subscriber exception reporting; Phase 10 owns that job.
- Do not broadly split App or Match or introduce a lifecycle framework.
- Do not attempt to recover from arbitrary errors after a successfully running match has begun.

## Likely Touch Points

- `client/src/match.js`
- `client/src/app.js`
- a small construction-cleanup helper only if it reduces duplicated unwind logic
- `tests/client_contracts/match_shell_contracts.mjs` and focused App/session contracts
- `docs/design/client-ui.md` where startup ownership changes

## Verification

- Focused failure-injection test for a late Match construction failure and complete Match-owned
  cleanup.
- Focused failure-injection test for LabPanel failure after Match construction and complete App-owned
  rollback.
- In both tests, start a subsequent session successfully and destroy it cleanly.
- `node scripts/check-client-architecture.mjs`
- `node tests/client_contracts.mjs`
- `node tests/select-suites.mjs --verify`
- `git diff --check`

## Manual Test Focus

Start and leave a normal match, then start a second match. Repeat once for Lab or replay, confirming
the lobby/game screens, pointer lock, settings, audio, and renderer remain usable after teardown.

## Handoff

Mark this phase done in its implementation commit. Report Match-owned and App-owned rollback
boundaries, both injected failure points, complete-cleanup evidence, and subsequent-start evidence.
Tell the Phase 10 agent that it should touch only subscriber diagnostics and avoid lifecycle changes.
