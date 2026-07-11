# Phase 1 - Report Persistence Contract

Status: planned.

## Goal

Define the database and server persistence contract for bug reports without exposing player-facing
UI yet. The result should make `bug_reports` a first-class persisted object that can store the
server-generated `replay_key` for the match while replay-evidence state separately records whether
the final `match_replays` row is still pending, available, or missing.

## Scope

- Add a migration for a `bug_reports` table.
- Add a stable server-generated `replay_key` UUID to the replay persistence contract. Phase 2 will
  allocate it from room state; this phase defines how persistence stores and resolves it.
- Add `replay_key` storage to `match_replays` with a uniqueness guarantee so review tooling can
  resolve a report to the eventual replay row when it exists.
- Add a replay-evidence table or equivalent DB-backed state keyed by `replay_key`, with explicit
  `pending`, `available`, and `missing` states plus nullable `match_id`, nullable `replay_id`, and
  nullable failure reason.
- Preserve the current one replay row per match model. `replay_key` is an alternate stable lookup
  key for reports, not a reason to create multiple replay artifacts for one match.
- Add server-side report data structs and database helpers behind two conceptual APIs:
  - `ReportStore` for blocking report create/list/update operations
  - `ReplayEvidenceRegistry` for registering expected final replay evidence and resolving it later
- Decide and document whether non-match/lobby reports use nullable `replay_key` or are deferred
  until a later phase.
- Keep report storage simple: chronological rows plus viewed/resolved booleans.
- Update match-history design documentation if this phase extends the persistence surface there, or
  create a small bug-reporting design section if that is clearer.

## Expected Schema Shape

The exact SQL can change during implementation, but the table should cover:

- `id bigserial primary key`
- `created_at timestamptz not null default now()`
- `replay_key uuid null`
- `room_name text null`
- `reporter_player_id integer null`
- `reporter_name text null`
- `reporter_faction_id text null`
- `report_tick integer null`
- `report_time_seconds numeric or integer null`
- `description text not null default ''`
- `client_context jsonb not null default '{}'`
- `network_context jsonb not null default '{}'`
- `server_context jsonb not null default '{}'`
- `viewed boolean not null default false`
- `resolved boolean not null default false`

Avoid duplicating replay resolution fields on every report row. Reports should resolve replay
availability through the evidence row keyed by `replay_key`; if the implementation keeps nullable
`match_id` or `replay_id` on `bug_reports` for query convenience, document them as denormalized
cache fields and define exactly how they stay in sync.

The replay-evidence table should cover:

- `replay_key uuid primary key`
- `state text not null check (state in ('pending', 'available', 'missing'))`
- `match_id bigint null references matches(id)`
- `replay_id bigint null references match_replays(id)`
- `expected_after_report boolean not null default false`
- `failure_reason text null`
- `created_at timestamptz not null default now()`
- `updated_at timestamptz not null default now()`

Add database-enforced consistency where practical:

- `available` evidence requires `replay_id` and must identify the canonical replay row.
- `missing` evidence requires a non-empty `failure_reason`.
- `pending` evidence must not point at a replay row.
- Repeated report submissions for the same `replay_key` upsert or preserve one evidence row
  idempotently instead of creating competing state.
- `match_replays(replay_key)` should be unique when non-null so report review can resolve one
  canonical replay artifact.

Prefer indexes for newest-first review and replay/match lookup:

- `(created_at desc)`
- `bug_reports(replay_key)`
- `replay_evidence(state, updated_at desc)` or equivalent if pending/missing review queries use it
- optional `(reporter_name)` or `(room_name)` only if it falls out naturally from the query shape

## Touch Points

- `server/migrations/*.sql`
- `server/src/db.rs`
- `docs/design/match-history.md` or a new report-focused design subsection
- `docs/context/match-history.md` if the capsule's code seams or invariants change

## Constraints

- Do not implement player UI or review UI in this phase.
- Do not add categories, annotations, auth, screenshots, spam controls, or advanced search.
- Keep clients as non-writers at the database layer. The HTTP/WebSocket server remains the only DB
  writer.
- Treat `server_context` as server-owned. Browser payloads must not write it; later API phases stamp
  it from `RoomReportContext` and request metadata after validating or resolving the reported
  context.
- Do not require a strict replay foreign key for mid-match reports. The whole point of `replay_key`
  is to let reports exist before the final replay row exists.
- Do not overload the existing log-and-drop match-history write helper for report creation or
  evidence registration. `ReportStore` and `ReplayEvidenceRegistry` methods must return errors to
  their callers.
- Do not regress current replay-history behavior: deployed resolved solo, player-vs-AI, and AI-only
  normal matches can already persist replay-backed rows when `RTS_RECORD_MATCHES` is enabled.
- Do not change ordinary `/api/matches` response shape unless required for the report linkage.

## Verification

- Add focused Rust/database helper tests where existing DB tests make that practical.
- At minimum, run formatting/checking relevant to touched Rust and SQL files.
- If SQLx compile-time checks are not available for migrations, include a manual migration review in
  the handoff.

## Manual Testing Focus

- Boot with `DATABASE_URL` configured and verify migrations apply.
- Confirm existing match-history reads still return normally.
- Confirm no report endpoints or UI are accidentally exposed before later phases.

## Handoff

After implementation, mark this phase done and summarize the final schema, how `replay_key` is
stored, nullable lobby-report behavior, and the exact `ReportStore` /
`ReplayEvidenceRegistry` helper APIs that later phases should call.
