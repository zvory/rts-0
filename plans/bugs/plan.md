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
- Reports live in the database and reference persisted replay rows when a replay exists.
- Reports should be stored indefinitely for now. Manual database cleanup is acceptable later.
- Anyone can submit and anyone can view the admin/review page. No authentication, privacy layer,
  spam controls, or rate limiting are required for this pre-alpha utility.
- The admin/review page is a simple chronological table with basic `viewed` and `resolved`
  booleans. Reports are not investigation records and do not need annotations, categories, unit-type
  indexing, or full-text search.
- A successful submission must show a report id and tell the tester to let Alex know they submitted
  it.
- Report submission is blocking. The client should not claim success until the server has persisted
  the report and guaranteed the linked replay evidence is durable.
- If the player disconnects during report submission, losing the report is acceptable. If the match
  ends after the player opened the report form but before they submit, submission should still work.
- Server shutdown/deploy drain must not exit before report/replay persistence needed by submitted
  reports has completed or reached the existing drain deadline policy.

## Phase Summaries

Phase 1 establishes the persistence contract for bug reports and report-backed replays. It adds the
database schema, server-side data model, and the rules for linking reports to `match_replays`
without exposing a player UI yet. It also documents how report writes differ from ordinary
match-history writes: reports are blocking evidence capture, while normal match history remains a
non-critical summary.

Phase 2 makes replay persistence usable for reportable matches, including human-vs-AI and
in-progress/live matches. It should ensure a report can force or reuse a durable replay artifact
even when the match would not normally qualify for match history. The phase ends with server-side
tests proving that a report can be anchored to a replay before the match naturally ends.

Phase 3 adds the server API for creating bug reports and serving a reviewable report list. It
validates and bounds untrusted client payloads, captures authoritative server context, writes the
report transactionally with replay evidence, and returns the created report id. It also exposes the
minimal read/update endpoints needed by the public review dashboard.

Phase 4 adds client-side evidence capture and the report form in match and replay contexts. It
mounts the report affordance through the existing settings/gear menu, collects optional text plus
client/browser/network/prediction diagnostics, and blocks the UI until the submission result is
known. It should preserve current match and replay lifecycle teardown behavior.

Phase 5 builds the public bug-review page and report-to-replay launch flow. The reviewer can scan
reports chronologically, see the optional description and key metadata, mark viewed/resolved, and
open a persisted replay near the report tick. The replay review route should start around twenty
seconds before the report tick and display the report context alongside playback.

Phase 6 hardens the end-to-end workflow and updates documentation. It adds focused integration
coverage for normal match, replay viewer, and human-vs-AI report flows; verifies deploy drain waits
for required report writes; and updates the relevant design docs and context capsules. This phase is
where lobby-only reporting can be enabled if the earlier phases left a clear nullable-replay path.

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
  metadata, replay persistence, and database writes.
- Treat clients as untrusted. Bound text length and JSON diagnostics, validate report context, and
  never trust client-supplied player ids, room state, replay ids, or ticks without checking server
  state where possible.
- Keep wire protocol mirrors synchronized whenever a WebSocket message changes:
  `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, `client/src/protocol.js`, and
  `docs/design/protocol.md`.
- Prefer HTTP API endpoints for database-backed report creation/listing unless a phase proves the
  live room task must participate through WebSocket state. If a WebSocket message is added, keep the
  payload small and bounded.
- Keep report persistence separate from normal match-history policy. Normal match history may remain
  eligibility-gated; report-backed replay persistence must work for human-vs-AI and other reportable
  playtest contexts.
- Do not make normal room transitions wait on non-report match history writes. Blocking behavior is
  required only for submitted report evidence.
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
