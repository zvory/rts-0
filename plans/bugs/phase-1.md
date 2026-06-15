# Phase 1 - Report Persistence Contract

Status: planned.

## Goal

Define the database and server persistence contract for bug reports without exposing player-facing
UI yet. The result should make `bug_reports` a first-class persisted object that can reference
`match_replays` while staying separate from normal match-history summaries.

## Scope

- Add a migration for a `bug_reports` table.
- Add server-side report data structs and database helpers.
- Decide and document whether non-match/lobby reports use nullable `match_id`/`replay_id` or are
  deferred until a later phase.
- Keep report storage simple: chronological rows plus viewed/resolved booleans.
- Update match-history design documentation if this phase extends the persistence surface there, or
  create a small bug-reporting design section if that is clearer.

## Expected Schema Shape

The exact SQL can change during implementation, but the table should cover:

- `id bigserial primary key`
- `created_at timestamptz not null default now()`
- `match_id bigint null references matches(id)`
- `replay_id bigint null references match_replays(id)`
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

Prefer indexes for newest-first review and replay/match lookup:

- `(created_at desc)`
- `(match_id)`
- `(replay_id)`
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

After implementation, mark this phase done and summarize the final schema, nullable replay/match
decision, and any database helper APIs that Phase 2 should call when it needs to link a report to
durable replay evidence.
