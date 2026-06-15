# Phase 5 - Review Dashboard and Replay Launch

Status: planned.

## Goal

Build the simple public review workflow for the developer. The page should show chronological bug
reports, expose key context, allow `viewed`/`resolved` toggles, and open the linked replay near the
reported tick.

## Scope

- Add a public review page or route for bug reports.
- Render a newest-first table with:
  - report id
  - created time
  - room/map/player summary
  - report tick/time
  - optional description
  - replay availability
  - viewed/resolved booleans
  - open replay action
- Add a report detail area if the table cannot comfortably show diagnostic JSON.
- Add report replay launch flow:
  - resolve the report's `replay_key` to the persisted replay row when it exists
  - show a clear pending state when the report exists before the final replay artifact has been
    written
  - show a clear missing state when expected replay upload failed, was gated off, or never produced
    an artifact
  - load persisted replay through existing compatibility checks
  - create/join a replay room
  - seek to roughly twenty seconds before `report_tick`, clamped to zero
  - display report context alongside playback
- Keep the dashboard public and simple. This is an internal pre-alpha utility, not a multi-user
  triage app.

## Touch Points

- `server/src/main.rs`
- `server/src/db.rs`
- `client/src/app.js`
- a new client module such as `client/src/bug_reports.js`
- client HTML/CSS
- `client/src/replay_viewer.js` or replay launch plumbing if report context must be displayed
  inside replay playback
- `docs/design/client-ui.md` if new app-shell routes or module contracts are added
- `docs/design/match-history.md` if replay launch/read endpoints are extended

## Constraints

- No auth.
- No annotations.
- No categories.
- No advanced search or filters beyond what falls out of the chronological API.
- Do not bypass replay compatibility validation.
- Do not make report status toggles required for review. They are convenience booleans only.

## Verification

- Add API tests for report list/detail/status update and replay launch near report tick.
- Add focused client tests for dashboard rendering if existing DOM test patterns are available.
- Run relevant Node live-server suite if the review launch crosses server/client behavior.

## Manual Testing Focus

- Open the public report dashboard and confirm reports appear newest first.
- Toggle viewed/resolved and refresh to confirm persistence.
- Open a report replay and confirm playback starts near twenty seconds before the report tick.
- Confirm a replay incompatibility produces a clear message instead of a broken viewer.

## Handoff

After implementation, mark this phase done and summarize the dashboard URL, replay launch URL shape,
status-toggle behavior, and any review UX rough edges left for Phase 6 hardening.
