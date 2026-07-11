# Phase 5 - Review Dashboard and Replay Launch

Status: planned.

## Goal

Build the simple public review workflow for the developer. The page should show chronological bug
reports, expose key context, allow `viewed`/`resolved` toggles, and open the linked replay near the
reported tick. Replay launch should be implemented as a reusable `ReplayReviewLaunch` result, not
as dashboard-only glue that bypasses existing replay compatibility paths.

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
  - return a `ReplayReviewLaunch` payload containing replay room, initial seek tick, report id,
    replay availability state, and report context
  - create/join a replay room through the same room-creation path as match-history replay launch
  - seek to roughly twenty seconds before `report_tick`, clamped to zero, using existing replay
    seek plumbing after the viewer joins
  - display report context alongside playback through an app/replay-viewer injected context value
- Use a report-specific replay lookup such as `replay_artifact_for_report(replay_key/report_id)`.
  It may reuse existing replay compatibility validation, but it must not inherit Recent Matches
  visibility filtering; hidden AI/debug rows with persisted artifacts remain reviewable when the
  evidence registry says they are available.
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
- Do not duplicate persisted replay loading rules outside the existing match-history/replay-launch
  compatibility checks; factor a shared helper if needed.
- Do not make replay-room creation know about dashboard DOM concerns. It should consume a replay
  artifact and return a room, while the dashboard/app layer carries report context separately.
- Do not make report status toggles required for review. They are convenience booleans only.

## Verification

- Add API tests for report list/detail/status update and replay launch near report tick.
- Add server tests for pending, available, missing, incompatible, and hidden-from-Recent-Matches
  replay-evidence states.
- Add focused client tests for dashboard rendering if existing DOM test patterns are available.
- Run relevant Node live-server suite if the review launch crosses server/client behavior.

## Manual Testing Focus

- Open the public report dashboard and confirm reports appear newest first.
- Toggle viewed/resolved and refresh to confirm persistence.
- Open a report replay and confirm playback starts near twenty seconds before the report tick.
- Confirm a replay incompatibility produces a clear message instead of a broken viewer.

## Handoff

After implementation, mark this phase done and summarize the dashboard URL, `ReplayReviewLaunch`
response shape, replay launch URL/query behavior, status-toggle behavior, and any review UX rough
edges left for Phase 6 hardening.
