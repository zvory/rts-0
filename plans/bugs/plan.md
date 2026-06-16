# Bug Reports - Multi-Phase Plan

This plan builds a lightweight pre-alpha bug reporting system for playtesters and the developer.
The goal is not a production ticket tracker; the goal is one-click, low-friction reports that
produce enough durable evidence to reproduce issues through authoritative replays and client-side
diagnostics.

## Product Contract

- Bug reporting should eventually be reachable from in-match, replay viewer, and lobby contexts.
  In-match and replay viewer support are the V1 priority.
- Report text is optional. The UI should prompt for useful reproduction hints, but submission must
  remain low friction.
- Screenshot capture is explicitly out of scope for V1.
- Pause is explicitly out of scope for V1. Any later pause support must be server-authoritative,
  not client-only, and the V1 architecture must not make that harder.
- Each match gets a server-generated stable `replay_key` UUID at match start. Reports and replay
  rows both store that key so mid-match reports can be associated with a replay before the final
  replay artifact exists.
- Reports live in the database and reference `replay_key` immediately. A strict foreign key to a
  replay row is not required because the report can exist before the final replay row is written.
- Current replay history already persists replay-backed rows for resolved deployed normal matches
  with at least one active participant, including solo, player-vs-AI, and AI-only matches, when
  `RTS_RECORD_MATCHES` is enabled. AI-only and Debug/quickstart rows may be hidden from the lobby
  Recent Matches feed while still retaining their replay artifacts for diagnostics.
- Reports should be stored indefinitely for now. Manual database cleanup is acceptable later.
- Anyone can submit and anyone can view the admin/review page. No authentication, privacy layer,
  spam controls, or rate limiting are required for this pre-alpha utility.
- The admin/review page is a simple chronological table with basic `viewed` and `resolved`
  booleans. Reports are not investigation records and do not need annotations, categories, unit-type
  indexing, or full-text search.
- A successful submission must show a report id and tell the tester to let Alex know they submitted
  it.
- Report submission is blocking. The client should not claim success until the server has persisted
  the report and registered that the report expects a replay for the report's `replay_key`.
- If the player disconnects during report submission, losing the report is acceptable. If the match
  ends after the player opened the report form but before they submit, submission should still work.
- Server shutdown/deploy drain must not exit before report/replay persistence needed by submitted
  reports has completed or reached the existing drain deadline policy.

## Durable Architecture Primitives

The implementation should build these primitives explicitly before adding UI workflows. Later
phases should compose these seams instead of scraping room internals, reaching across client
modules, or overloading match-history helpers with incompatible reliability rules.

- `ReplayIdentity`: a stable server-owned `replay_key` allocated when a live match or persisted
  replay-review room is created. This is identity only; it is not a replay row and should not imply
  that a final artifact already exists.
- `ReplayEvidenceRegistry`: a DB-backed replay-evidence state keyed by `replay_key`, with
  `pending`, `available`, and `missing` states plus optional `match_id`, `replay_id`, and failure
  reason. This is the only place that tracks whether a submitted report expects a final replay.
- `ReportStore`: a blocking persistence API for creating, listing, and updating bug reports. It
  returns errors to the API layer; it must not inherit match history's log-and-drop write behavior.
- `RoomReportContext`: a bounded authoritative snapshot produced by the room task through an
  explicit request/reply seam. It should include only report-safe facts such as room, phase, map,
  current tick, reporter seat, `replay_key`, and replay playback state.
- `ClientReportService`: a browser-side service that owns bounded report payload construction and
  HTTP submission. Match, replay, settings, health, and diagnostics modules should expose small
  snapshot methods to this service rather than importing report UI code.
- `ReplayReviewLaunch`: a server API result that resolves one report to a compatible replay room,
  an initial seek tick, replay availability state, and report context for display alongside
  playback.

## Boundary Invariants

- Report creation is a server-stamped evidence write. Browser-supplied ids, ticks, replay ids,
  room names, and server-ish context are hints only unless validated through `RoomReportContext`;
  the browser must not populate `server_context`.
- Clients may submit only bounded report text, client diagnostics, and network diagnostics. The
  server stamps authoritative room, match, replay, player, faction, tick, build, receipt-time, and
  replay-evidence fields from `RoomReportContext` and request metadata.
- `ReportStore` and `ReplayEvidenceRegistry` are required-write APIs. They may share lower-level
  SQL helpers with match history, but they must not call a log-and-drop `record_match` style API or
  hide persistence errors from the report API.
- Replay evidence is a state machine keyed by `replay_key`: `pending` means no replay row is known,
  `available` requires a replay row, and `missing` requires a recorded reason. Transitions must be
  idempotent for repeated reports against the same key and must not regress `available` evidence to
  `pending`.
- Public report handlers should live behind a small reporting route/module such as
  `server/src/reporting.rs` or `server/src/routes/bug_reports.rs`; `main.rs` wires routes and shared
  state only.
- Client reporting UI is injected through app/match/replay seams. UI modules must not import
  `Match`, `ReplayViewer`, or `Net` directly to scrape state.

## Phase Summaries

Phase 1 establishes the persistence contract and durable DB primitives for bug reports and
report-backed replay evidence. It adds `bug_reports`, replay-evidence state keyed by
`replay_key`, and storage for `replay_key` on canonical replay rows without duplicating replay
resolution fields on each report. It also documents the split between blocking `ReportStore`
operations and best-effort match-history writes.

Phase 2 introduces `ReplayIdentity` and `ReplayEvidenceRegistry` into the room and match-end replay
write path. Since resolved deployed normal matches already persist replay-backed rows for solo,
player-vs-AI, and AI-only games, this phase writes the final replay under the existing
`replay_key` and transitions evidence from pending to available or missing. The phase ends with
server-side tests proving that report-backed replay evidence can be registered before match end and
resolved by the existing canonical replay upload.

Phase 3 adds the server API for creating bug reports and serving a reviewable report list. It
adds the explicit `RoomReportContext` request/reply seam so HTTP report creation can validate live
or replay context without owning room internals. It bounds untrusted client diagnostics, writes the
report through `ReportStore`, registers replay evidence through the registry, and exposes minimal
read/update endpoints for the public review dashboard.

Phase 4 adds client-side evidence capture and the report form in match and replay contexts. It
introduces `ClientReportService`, small report-context snapshot methods on match/replay/health
surfaces, and a settings-menu action that opens the report form. It collects optional text plus
bounded browser/network/prediction diagnostics and blocks the form until the submission result is
known. It should preserve current match and replay lifecycle teardown behavior.

Phase 5 builds the public bug-review page and report-to-replay launch flow. The reviewer can scan
reports chronologically, see the optional description and key metadata, mark viewed/resolved, and
open a persisted replay through `ReplayReviewLaunch`. The replay review route should start around
twenty seconds before the report tick and display report context alongside playback without
bypassing existing replay compatibility checks.

Phase 6 hardens the end-to-end workflow and updates documentation. It adds focused integration
coverage for normal match, replay viewer, human-vs-AI, solo, and hidden-row report flows; verifies
deploy drain waits for required report writes; and updates the relevant design docs and context
capsules. This phase is where lobby-only reporting can be enabled if the earlier phases left a clear
nullable-replay path.

## Phase Index

1. [Phase 1 - Report Persistence Contract](phase-1.md)
2. [Phase 2 - Report-Backed Replay Durability](phase-2.md)
3. [Phase 3 - Report API and Server Context](phase-3.md)
4. [Phase 4 - Client Report Capture UI](phase-4.md)
5. [Phase 5 - Review Dashboard and Replay Launch](phase-5.md)
6. [Phase 6 - End-to-End Hardening and Documentation](phase-6.md)

## Overall Constraints

- Keep the system scoped to pre-alpha playtesting. Do not add categories, annotations, advanced
  search, auth, privacy controls, rate limiting, or screenshots unless a later product decision
  explicitly expands scope.
- Preserve the server-authoritative architecture. The server owns report ids, authoritative match
  metadata, `replay_key` allocation, replay persistence, and database writes.
- Treat clients as untrusted. Bound text length and JSON diagnostics, validate report context, and
  never trust client-supplied player ids, room state, replay ids, ticks, or `server_context`
  without checking server state where possible.
- Keep wire protocol mirrors synchronized whenever a WebSocket message changes:
  `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, `client/src/protocol.js`, and
  `docs/design/protocol.md`.
- Prefer HTTP API endpoints for database-backed report creation/listing unless a phase proves the
  live room task must participate through WebSocket state. If a WebSocket message is added, keep the
  payload small and bounded.
- Keep report persistence separate from Recent Matches visibility. Report-backed replay lookup
  should be able to resolve hidden but persisted replay rows, including AI-only and Debug/quickstart
  rows that are omitted from the lobby table.
- Keep blocking report writes separate from best-effort match-history writes. Shared lower-level SQL
  helpers are acceptable, but `ReportStore` and evidence registration must return errors and must not
  call a log-and-drop `record_match` style API.
- A submitted report must mark its `replay_key` as requiring final replay evidence. For deployed
  normal matches this should compose with the existing final replay upload path rather than adding a
  second artifact. The review dashboard must handle the period where a report exists but the replay
  artifact is still pending, and must show a clear missing-replay state if final replay persistence
  fails or the room was intentionally excluded from replay upload.
- Do not make normal room transitions wait on non-report match history writes. Blocking behavior is
  required only for submitted report evidence.
- Do not let HTTP handlers read mutable room state directly. If authoritative live/replay context is
  needed, request a bounded `RoomReportContext` from the room task through an explicit event/reply
  API.
- Do not make client report UI scrape arbitrary module internals. Add narrow report snapshot methods
  or injected collaborators, and keep non-shell cross-area imports out unless the architecture check
  allowlist is intentionally updated with a reason.
- The review route may warn about build/map incompatibility just like match-history replay launch.
  It should not attempt best-effort playback across incompatible replay artifacts.
- Any client module that adds DOM/window listeners, timers, or GPU resources must implement
  `destroy()` and be torn down through the existing app/match lifecycle.
- Use focused verification during phase work. Let the final commit hook provide broad coverage when
  the phase is ready to merge.

## Implementation Process

Implement one phase at a time. Each phase should be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit for that phase.

After each phase, the implementing agent must provide a handoff message describing what the next
agent should do, any constraints or decisions discovered during implementation, and the core
features that should be manually tested. Manual testing notes should name the essential flows, not a
comprehensive test matrix.
