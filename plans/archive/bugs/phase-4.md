# Phase 4 - Client Report Capture UI

Status: planned.

## Goal

Add the player-facing report affordance and evidence capture in match and replay contexts. The UI
should be low-friction: report button, optional description/reproduction hints, blocking submit,
and a clear confirmation containing the report id. The client work should introduce a
`ClientReportService` that composes small snapshot methods instead of making the report UI scrape
match, replay, network, or diagnostics internals.

## Scope

- Add `Report bug` to the existing gear/settings menu for live matches and replay viewer.
- Implement a `ClientReportService` that owns bounded payload construction, calls the Phase 3 API,
  handles success/failure state, and exposes a small submit API to UI components.
- Classify new client files deliberately in `scripts/check-client-architecture.mjs`. Prefer a split
  where report form/dashboard modules are UI, the report service is an injected app-shell facade,
  and neither UI code nor report diagnostics import `Match`, `ReplayViewer`, or `Net` directly.
- Add a settings-menu action builder such as `buildReportBugAction` rather than putting
  bug-report-specific behavior into generic settings tab rendering.
- Build a compact modal/panel for optional description text.
- Prompt with practical hints such as what happened, what the player expected, and what they just
  did before the issue.
- Capture client context:
  - browser user agent and platform where available
  - viewport size and device pixel ratio
  - current URL/context
  - client build/version from `/version` or existing app state
  - current replay state when in replay viewer
  - current visible tick/snapshot tick when known
- Capture network/prediction diagnostics from existing health reporting surfaces:
  - latency/rtt summary
  - jitter/snapshot gap summary
  - WebSocket buffered amount if available
  - prediction mode and correction counters where available
  - recent client-side errors/log tail if a bounded buffer exists or can be introduced cleanly
- Add narrow snapshot methods where needed, for example `Match.reportContextSnapshot()`,
  `ReplayViewer.reportContextSnapshot()`, `MatchHealth.reportSnapshot()`, and a diagnostics
  `reportSnapshot()` that returns bounded, sanitized rows/counts.
- Submit through the Phase 3 API and block the form until success or failure.
- Show success text with the report id and instruction to let Alex know it was submitted.

## Touch Points

- `client/src/settings_container.js`
- `client/src/settings_panels.js`
- a new client report service/module, for example `client/src/bug_report_service.js`
- a small report form/action module, for example `client/src/bug_report_form.js`
- `client/src/match.js`
- `client/src/replay_viewer.js`
- `client/src/replay_controls.js` only if report context needs replay tick state not already exposed
- `client/src/match_health.js`
- `client/src/net.js` or a new app-shell/platform helper for HTTP report submission
- `client/src/app.js` for dependency injection and lobby/lifecycle plumbing if needed
- `client/index.html` and client CSS for the modal/panel
- `scripts/check-client-architecture.mjs` if new modules need area classification
- `docs/design/client-ui.md` if exported client module contracts change

## Constraints

- Do not implement screenshots.
- Do not implement pause.
- Do not add categories or required fields.
- Keep the modal usable while the live game continues. The V1 report UI may obstruct local input,
  but it must not imply an authoritative pause.
- Preserve teardown. Any new event listeners, timers, log buffers, or DOM nodes must be destroyed
  between matches/replays.
- Keep modules composed through existing dependency injection patterns. Avoid non-shell cross-area
  imports unless the architecture checker allowlist is intentionally updated with a reason.
- Do not make `settings_panels.js` own report payload construction. Settings should host actions;
  report payload construction belongs in `ClientReportService`.
- Do not store unbounded console/log history. Any error/log tail must have a fixed entry count and
  string-length cap before it can be included in report payloads.

## Verification

- Add focused JS tests or DOM contract checks for report payload creation and button visibility
  where the existing test style supports it.
- Add service-level tests for payload bounding, empty-description submit, API failure handling, and
  live/replay context snapshot composition.
- Run `node scripts/check-client-architecture.mjs` if new modules/imports are added.
- Run the smallest relevant client/server integration test for the report submission path.

## Manual Testing Focus

- Live match: open gear menu, submit an empty-description report, see confirmation with id.
- Live match: submit a description and confirm it appears in the database/API response.
- Replay viewer: submit a report and confirm replay tick/context are included.
- Failure path: simulate a failed API response and confirm the player gets a clear failure without
  a false success.

## Handoff

After implementation, mark this phase done and summarize the `ClientReportService` API, the
snapshot methods added, the bounded client context fields captured, where the report UI is mounted,
and which report contexts still need Phase 5 review-page treatment.
