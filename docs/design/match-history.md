# Match history

Persisted record of every resolved multi-player match, displayed on the lobby front page.

Source of truth for: schema, write path, read path, the recording gate, and failure modes.
Mirrors of any of these elsewhere (capsules, CLAUDE.md) point here.

## Why this exists

Players want to see what was played: who won, on which map, how long, and the score screen
breakdown. The server is already the only authoritative actor that knows when a match resolves
and what the scores are, so the server is also the only writer.

## Storage

Supabase Postgres. Schema in `server/migrations/`. Single table:

| column          | type            | notes                                           |
| --------------- | --------------- | ----------------------------------------------- |
| `id`            | `bigserial` PK  |                                                 |
| `started_at`    | `timestamptz`   | Wall clock captured at `start_match`.           |
| `ended_at`      | `timestamptz`   | Wall clock at `end_match`. Default `now()`.     |
| `duration_ms`   | `integer`       | Server-computed, clamped to non-negative i32.   |
| `map_name`      | `text`          | `selected_map` at match start.                  |
| `winner_name`   | `text` nullable | `null` ⇔ draw.                                  |
| `outcome`       | `text`          | `'win'` or `'draw'` (CHECK constraint).         |
| `participants`  | `text[]`        | Display names in seat order (humans then AI).   |
| `score_screen`  | `jsonb`         | Whole `Vec<PlayerScore>` blob, opaque to SQL.   |

Indexes: `(started_at desc)` for the front-page query, `(map_name)` for future filtering.

The `score_screen` JSONB intentionally stores the full payload from `Game::scores()`. The shape
matches `contract::PlayerScore` (camelCase). Adding fields to `PlayerScore` requires no migration;
old rows simply lack the new fields.

Migrations are versioned SQL files run by `sqlx::migrate!` at server boot. Never hand-apply DDL.

## Wire

- **Read**: `GET /api/matches?limit=N` — JSON array, newest first, `limit` clamped server-side to
  `[1, 100]`, defaults to 20. Returns `[]` when no DB is configured (so the client never needs
  to special-case missing-DB).
- **Write**: none. Clients cannot write history. Period.

## Code seams

- `server/src/db.rs` — `Db` (pool + migrate), `record_match`, `recent_matches`, `MatchRecord`,
  `MatchSummary`.
- `server/src/main.rs` — `.env` loading, pool construction, `/api/matches` handler, the
  `RTS_RECORD_MATCHES` gate.
- `server/src/lobby/mod.rs` — `Lobby::with_db()` injects an `Option<Arc<Db>>` into spawned rooms.
- `server/src/lobby/room_task.rs` — captures `match_started_at`, `match_map_name`,
  `match_participants` at `start_match`; writes one row in `end_match` via a **detached**
  `tokio::spawn`. Detachment is load-bearing: a slow Supabase write must never stall the room
  transitioning back to lobby.
- `client/src/match_history.js` — fetches and renders the lobby table; row click expands the
  score screen.
- `client/src/app.js` — mounts `MatchHistory` when the lobby shows; `refresh()` is called from
  `onBackToLobby` so the freshly-written row appears without a page reload.

## What gets recorded

A row is written when **all** of these are true:

1. The lobby reached `Phase::InGame` (so `match_started_at` was captured).
2. `match_human_count >= 2` — at least two human (non-AI) players. Human-vs-AI, AI-only, and
   1-player sandboxes never record.
3. `is_dev_watch()` is false — dev self-play, scenario, and replay rooms never record.
4. The server was started with **both** a working DB connection and `RTS_RECORD_MATCHES` truthy.

Anything else (DB failures, env gate off, dev rooms) silently skips the write. The simulation
and lobby flow are unaffected.

## Recording gate (`RTS_RECORD_MATCHES`)

The gate exists because the developer runs many local matches and does not want them polluting
the shared production DB.

| env state                            | reads work?    | writes happen? |
| ------------------------------------ | -------------- | -------------- |
| no `DATABASE_URL`                    | no (returns `[]`) | no          |
| `DATABASE_URL` set, gate off / unset | yes            | no             |
| `DATABASE_URL` set, gate on          | yes            | yes            |

Truthy values: `1`, `true`, `yes`, `on` (case-insensitive). Anything else, including unset, is
off. Beta and mainline deploys must set it to `1`. Local `cargo run` reads but does not write.

The implementation: `main.rs` connects the pool once, hands the pool to `AppState` for reads, and
conditionally passes it to `Lobby::with_db(...)`. The lobby propagates the option to each room.
If a room receives `None`, the `end_match` write branch never fires.

## Failure modes

- **DB unreachable at boot**: `try_connect_from_env()` logs and returns `None`. Server runs
  without history (reads return `[]`, no writes attempted).
- **DB drops mid-run**: write attempts log an error and continue. No retry, no outbox. This is
  acceptable; match history is non-critical.
- **Migration fails at boot**: `Db::connect` returns `Err`, server runs without history. Check
  `migrations/` filenames are timestamp-prefixed and sequential.
- **Slow write**: detached task means the room is unblocked. Worst case the row appears seconds
  later in `/api/matches`.

## Secrets and rotation

- `DATABASE_URL` must include `?sslmode=require` (Supabase rejects un-TLS connections).
- Pool capped at 5 connections (Supabase free-tier safety margin), 5s acquire timeout.
- `.env` is gitignored. `.env.example` documents the required vars.
- If the password leaks, rotate via Supabase dashboard and `flyctl secrets set DATABASE_URL=...`.

## Display-name identity caveat

Identity is just the display name a player typed in the lobby. Names can collide and there's no
dedup. Per-player stats and W/L are not derivable from the current schema. When (if) accounts
become a real feature, add a `player_id`-keyed table and a join column on `matches`; the
existing JSONB blob is unaffected.

## Future evolution

- **Pagination / filter by player/map**: add LIMIT/OFFSET to `recent_matches`, expose as query
  params. Index on `map_name` already exists.
- **Leaderboard**: a separate aggregate query is fine; do not denormalize into `matches` until
  there's a real perf reason.
- **Crash-safety**: if matches start dropping due to DB outages, add a small bounded
  in-memory outbox in `Db`. Not worth it today.
