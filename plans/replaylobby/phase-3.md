# Phase 3 - Integration Coverage and Resume-Ready Polish

Status: Not started.

## Goal

Lock the group replay lobby flow with integration coverage and leave the code/docs ready for future
resume-from-replay work.

## Scope

- Add live integration coverage for replay lobby creation, browser visibility, spectator joins,
  host start, shared playback, and viewer detach.
- Cover that normal lobbies, full-lobby spectator joins, in-game spectator joins, internal lab/dev
  rooms, saved replay artifacts, post-match replay, and replay branch staging keep their expected
  behavior.
- Review cleanup and disposal behavior so empty replay staging rooms release their registry entry.
- Tighten lobby browser and joined-lobby copy if Phase 2 left rough edges.
- Document the explicit future seam for playable replay resume: seat claiming, original-player
  loadout validation, and transition from spectator replay to playable branch/resume must remain a
  separate contract.

## Expected Touch Points

- `tests/lobby_browser_integration.mjs`
- `tests/client_contracts/lobby_contracts.mjs`
- Additional focused Rust tests under `server/src/lobby/**` if gaps remain
- `docs/design/match-history.md`
- `docs/design/client-ui.md`
- `docs/design/protocol.md` only if Phase 3 changes contract details

## Verification

- Targeted live Node integration suite that covers lobby browser behavior.
- Targeted client contract suite.
- Any focused Rust tests added or affected by replay lobby lifecycle changes.

## Manual Testing Focus

Run the full group-watch path in two browser tabs: create a replay lobby from Recent Matches, join
from the public browser, start playback, use replay controls, and leave one viewer while the other
continues watching.

## Handoff Expectations

Summarize final shipped behavior, exact tests run, any residual risk around match-history
availability or replay compatibility, and the recommended first phase for future playable
resume-from-replay work.
