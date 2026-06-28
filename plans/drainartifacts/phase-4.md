# Phase 4 - End-to-End Coverage and Deploy Docs

Status: Not started.

## Goal

Verify and document the full deploy-drain abort path from shutdown signal to Recent Matches display
and replay launch.

## Scope

- Add or extend end-to-end tests around `/api/matches`, replay launch, and Recent Matches rendering
  for `outcome = aborted`.
- Add a bounded local integration or regression scenario that starts a live match, triggers the
  shortened drain path, and verifies an aborted match-history/replay record through test doubles or
  a safe configured persistence path.
- Update deploy and match-history docs with the operational behavior:
  - natural matches can finish during the first drain window
  - remaining eligible live matches are recorded as aborted
  - replay artifacts are uploaded before connection shutdown when the bounded write wait succeeds
  - timed-out writes are logged as failures/blockers for that drain event
- Update log-check guidance so operators can verify a beta/mainline deploy recorded or skipped
  aborted matches intentionally.
- Review user-facing copy in Recent Matches so "Aborted" is clear and does not imply a winner.

## Expected Touch Points

- `tests/regression.mjs` or a focused new Node/Rust integration test if practical
- `tests/client_contracts/*` for match-history UI rendering
- `server/src/main.rs` tests if additional drain assertions are needed
- `client/src/match_history.js` only if phase 1 copy needs polish
- `docs/design/match-history.md`
- `docs/design/hardening.md`
- `docs/fly.md`
- `docs/context/match-history.md`
- `docs/context/deployment.md` if capsule pointers or invariant summaries shift

## Implementation Notes

- Prefer deterministic local tests over a test that requires real Fly or Supabase. Beta validation
  can be manual/operational after the PR lands.
- If a full DB-backed integration test is impractical, keep DB behavior covered at the Rust unit
  layer and use an in-process/fake persistence sink for drain lifecycle coverage.
- Do not broaden the local full-suite requirement. Use targeted tests and let the PR gate run
  `./tests/run-all.sh`.

## Verification

- Focused client contract test for `Aborted` display and replay action visibility.
- Focused server drain lifecycle test or regression test for the forced-abort path.
- Focused replay-launch compatibility test proving an aborted row with a valid replay artifact can
  still launch replay.
- `node tests/protocol_parity.mjs` only if protocol shape changed.
- `git diff --check`.

## Manual Testing Focus

On beta or a local deploy-like environment, start a human match, trigger deploy drain before it
resolves, and wait for the server to exit. Confirm Recent Matches shows `Aborted` with no winner,
the row expands with a score screen, and Watch Replay launches the captured replay to the shutdown
tick. Check logs for the forced abort request, room ack, match recorded with `replay=true`, write
wait completion, and connection shutdown.

## Handoff Expectations

Summarize the final tested behavior, exact commands/tests run, and any beta validation evidence.
Call out residual risks, especially write-timeout behavior or any gap where a hard platform kill can
still prevent upload.
