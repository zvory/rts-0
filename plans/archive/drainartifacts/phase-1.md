# Phase 1 - Aborted Outcome Contract

Status: Done.

## Goal

Make `aborted` a first-class match-history outcome everywhere a persisted match row is stored,
served, and displayed, without changing deploy drain lifecycle behavior yet.

## Scope

- Add a migration that updates the `matches.outcome` CHECK constraint to accept `aborted` in
  addition to `win` and `draw`.
- Replace the current `MatchRecord::outcome()` derivation from `winner_name` with an explicit
  server-side outcome value or enum.
- Preserve existing normal outcomes:
  - winner present -> `win`
  - no winner after ordinary match resolution -> `draw`
  - deploy-shutdown finalization in later phases -> `aborted`
- Keep `winner_name = null` for both draw and aborted rows, with `outcome` distinguishing them.
- Update `/api/matches` summary compatibility so older `win` and `draw` rows still serialize
  unchanged, and aborted rows serialize as `outcome: "aborted"`.
- Update `client/src/match_history.js` so the Winner column displays `Aborted` for
  `row.outcome === "aborted"` and does not display a winner.
- Update match-history design docs to describe the new outcome vocabulary and the winner-name
  distinction.

## Expected Touch Points

- `server/migrations/*.sql`
- `server/src/db.rs`
- `server/src/main.rs` test fixtures using `MatchSummary`
- `server/src/lobby/room_task/lifecycle.rs` normal end-match record construction
- `client/src/match_history.js`
- focused client contract test, either new or near existing client contract coverage
- `docs/design/match-history.md`
- `docs/context/match-history.md` if the capsule summary needs refreshing

## Implementation Notes

- Prefer a small `MatchOutcome` type in Rust over passing raw string literals throughout the write
  path. Keep serde/API output as the existing lowercase strings.
- The migration should be compatible with existing data. Existing `win` and `draw` rows must remain
  valid.
- Do not add shutdown-generated aborted rows in this phase. Tests can use direct fixtures or row
  mapping helpers to prove the contract.

## Verification

- Focused Rust tests for `MatchRecord` outcome handling and API summary serialization.
- Focused JS/client test for Recent Matches winner-label rendering of `win`, `draw`, `aborted`, and
  unknown/empty outcome rows.
- `git diff --check`.

## Manual Testing Focus

Manually exercise only the Recent Matches rendering if convenient by stubbing or serving a local
aborted row payload. Confirm the table says `Aborted`, shows no winner name, and still exposes replay
controls according to `replayAvailable`.

## Handoff Expectations

State the exact Rust outcome type/string contract, migration filename, and client display rule. Call
out that later phases must pass `aborted` explicitly when server shutdown finalizes a match, instead
of relying on `winner_name = null`.
