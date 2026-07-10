# Match history

Persisted record and replay artifact for resolved deployed matches, with a filtered Recent Matches
feed on the lobby front page.

Source of truth for: schema, write path, read path, the recording gate, and failure modes.
Mirrors of any of these elsewhere (capsules, CLAUDE.md) point here.

## Why this exists

Players want to see what was played: who won, on which map, how long, and the score screen
breakdown. The server is already the only authoritative actor that knows when a match resolves
and what the scores are, so the server is also the only writer.

## Storage

Supabase Postgres. Schema in `server/migrations/`. Match summaries live in `matches`:

| column          | type            | notes                                           |
| --------------- | --------------- | ----------------------------------------------- |
| `id`            | `bigserial` PK  |                                                 |
| `started_at`    | `timestamptz`   | Wall clock captured at `start_match`.           |
| `ended_at`      | `timestamptz`   | Wall clock at `end_match`. Default `now()`.     |
| `duration_ms`   | `integer`       | Server-computed, clamped to non-negative i32.   |
| `match_run_id`  | `text` nullable | Stable live-match/log correlation id; set for newly recorded matches. |
| `map_name`      | `text`          | `selected_map` at match start.                  |
| `winner_name`   | `text` nullable | Winner display name for wins only; `null` for draws and aborted matches. |
| `outcome`       | `text`          | `'win'`, `'draw'`, or `'aborted'` (CHECK constraint). |
| `participants`  | `text[]`        | Display names in seat order (humans then AI).   |
| `score_screen`  | `jsonb`         | Whole `Vec<PlayerScore>` blob, opaque to SQL.   |
| `human_count`   | `integer`       | Non-AI players at match start.                  |
| `debug_mode`    | `boolean`       | Visibility flag for debug rows. One-human, no-AI sandbox rows write `true`; normal product rows write `false`. |
| `local_only`    | `boolean`       | Hide local developer rows from public servers.  |

Indexes: `(started_at desc)` for the front-page query, a partial `(started_at desc)` index for
public rows, and `(map_name)` for future filtering.

`outcome` is the source of truth for distinguishing no-winner results: `draw` is ordinary match
resolution with no winner, while `aborted` is server-controlled shutdown finalization. `winner_name`
must not be overloaded for that distinction.

The `score_screen` JSONB intentionally stores the full payload from `Game::scores()`. The shape
matches `contract::PlayerScore` (camelCase). Adding fields to `PlayerScore` requires no migration;
old rows simply lack the new fields. Team-capable rows include each player's `teamId`, so grouped
team results are recoverable without adding SQL columns.

Replay artifacts live in `match_replays`, keyed one-to-one by `match_id`:

| column                    | type          | notes                                                |
| ------------------------- | ------------- | ---------------------------------------------------- |
| `id`                      | `bigserial` PK |                                                      |
| `match_id`                | `bigint`      | Unique FK to `matches(id)`, cascade delete.          |
| `artifact_schema_version` | `integer`     | Deterministic replay artifact schema.                |
| `build_sha`               | `text`        | Server build that recorded the replay.               |
| `map_name`                | `text`        | Map name captured in the artifact.                   |
| `map_schema_version`      | `integer`     | Map schema captured in the artifact.                 |
| `map_hash`                | `text`        | Authored map content hash captured in the artifact.  |
| `duration_ticks`          | `integer`     | Replay duration in simulation ticks.                 |
| `artifact_json`           | `jsonb`       | Whole versioned `ReplayArtifactV1` blob.             |
| `created_at`/`updated_at` | `timestamptz` | Default to `now()` at insert.                        |

`matches.score_screen` remains score data only. Replay playback never reads replay payloads from
`score_screen`. New replay rows write `artifactSchemaVersion: 3`, whose `startState` carries the
launch map binding and an embedded tick-zero `GameCheckpointV1` text payload. The authoritative
command stream, `durationTicks`, `winnerId`, `winnerTeamId`, and `finalScores` are attached when
the artifact is finalized at match end. Schema 2 rows have no `startState` and are intentionally
rejected by current replay compatibility checks. The replay `artifact_json` carries
`players[].teamId`, `winnerId`, `winnerTeamId`, and `finalScores[].teamId`; `winner_name` remains
the display-compatible name for the first living player represented by `winnerId`.

Migrations are versioned SQL files run by `sqlx::migrate!` at server boot. Never hand-apply DDL.

## Wire

- **Read**: `GET /api/matches?limit=N` — JSON array, newest first, `limit` clamped server-side to
  `[1, 300]`, defaults to 300. Returns `[]` when no DB is configured (so the client never needs
  to special-case missing-DB). The Recent Matches feed includes only rows with at least one
  human player and `debug_mode = false`, and it explicitly suppresses historical one-human,
  one-participant rows. Solo sandbox rows, AI-only rows, and historical debug rows may be
  persisted with replay artifacts but stay out of the lobby table. Local-only rows are included
  only when the request peer address is loopback; public beta/mainline requests filter them out.
  Each summary includes `replayAvailable` plus `replayUnavailableReason`. Availability is false
  when no replay row exists or its artifact schema, map schema, or map content hash is
  incompatible with the running server. Build-SHA mismatches are warning-compatible:
  `replayAvailable` remains true and `replayUnavailableReason` carries the compatibility warning.
- **Replay launch**: `POST /api/matches/{id}/replay` — read-only launch request. The server loads
  the persisted artifact only if the match is visible to the request scope, validates it against
  the running map metadata and the shared replay faction/loadout validator used by replay rooms,
  creates a spectator-only replay staging lobby, and returns `{ "room": "..." }`. Once the first
  viewer joins, that room appears in `/api/lobbies` with `kind: "replay"` and safe room metadata
  only; the stored artifact JSON is never exposed through the lobby browser. The host starts
  playback with the normal WebSocket `start` message, without ready/team/map/AI setup checks.
  Build-SHA mismatches log a warning and remain launchable. Schema, map, faction/loadout, or
  missing replay failures return a clear JSON `{ "error": "..." }` instead of trying partial
  playback.
- **AI observation lookup**: `GET /api/observations/{matchRunId}` — read-only recovery for an
  AI-only watched match that is intentionally hidden from Recent Matches. The run id is shown on
  the completed-match score screen and is the exact `match_run_id` in the structured server logs.
  The response is a normal `MatchSummary`; use its `id` with the existing replay-launch endpoint.
  The id accepts only `[A-Za-z0-9_-]` and is bounded to 96 bytes. A just-completed observation can
  return 404 briefly while the detached history/replay write finishes.
- **Write**: none. Clients cannot write history. Period.

## Code seams

- `server/src/db.rs` — `Db` (pool + migrate), `record_match`, `recent_matches`,
  `observation_by_run_id`, `replay_artifact_for_match`, `MatchRecord`, `MatchSummary`.
- `server/src/main.rs` — `.env` loading, pool construction, `/api/matches` handler, the
  `POST /api/matches/{id}/replay` launch handler, replay compatibility checks, and the
  `RTS_RECORD_MATCHES` gate.
- `server/src/lobby/mod.rs` — `Lobby::with_match_history()` injects an `Option<Arc<Db>>` into
  spawned rooms and can create persisted replay rooms from launch-approved artifacts. The
  lobby/drain state also owns the bounded match-history write tracker, exposes the shutdown
  wait primitive for pending replay/history writes, and sends `FinalizeForShutdown` requests to
  room tasks after the natural deploy-drain window expires.
- `server/src/lobby/room_task.rs` — captures `match_started_at`, `match_map_name`,
  `match_participants`, and the launch-time replay start composition at `start_match`; finalizes
  `ReplayArtifactV1` from that stored start plus the ending command log/scores; writes the match
  row and optional replay row in `end_match` via a tracked **detached** task.
  Normal match completion writes explicit `win` or `draw` outcomes; deploy-drain abort
  finalization captures the current active `Game`, writes `aborted`, marks the room's drain
  tracking finished, and does not transition clients into post-match replay playback because the
  process is exiting. Detachment is load-bearing: a slow Supabase write must never stall the room
  transitioning back to lobby. The tracker snapshots pending writes at shutdown wait start, so
  writes started later do not extend that wait forever.
- `client/src/match_history.js` — fetches and renders the lobby table; row click expands the
  score screen and, when compatible, exposes a replay launch action.
- `client/src/app.js` — mounts `MatchHistory` when the lobby shows; `refresh()` is called from
  normal-room `onBackToLobby` so the freshly-written row appears without a page reload.
  `?replayRoom=...` auto-joins a server-created replay staging lobby through the normal WebSocket
  join flow, and after playback starts its back-to-lobby action navigates to `/` so only that
  viewer leaves the replay room.

## What gets recorded

A row is written when **all** of these are true:

1. The lobby reached `Phase::InGame` (so `match_started_at` was captured).
2. At least one active participant was present at match start. Player-vs-AI and AI-only deployed
   matches record when they resolve. One-human, no-AI sandbox matches also record if they resolve,
   but write `debug_mode = true` so they are treated as debug sessions and stay out of Recent
   Matches.
3. `is_dev_watch()` is false — dev scenario rooms never record.
4. The room/participants do not match automated smoke/integration/regression test fingerprints:
   `itest-*`, `ai-itest-*`, `client-smoke-*`, `reg-*`, `smoke`, or the `Alpha`/`Bravo`
   integration pair. AI profile-label participants such as `AI 1.2` are allowed so
   player-vs-AI matches record.
5. The server was started with a working DB connection and `RTS_RECORD_MATCHES` is truthy.

Anything else (local gate off, DB failures, dev rooms, test rooms, missing DB) silently skips the
write. The simulation and lobby flow are unaffected. Replay artifacts use the same eligibility as
match rows: if a match row is skipped, no replay row is written. Stored historical debug and
AI-only rows, plus solo sandbox rows, are filtered from `/api/matches`, but the replay row remains
linked to the owning `matches` row.

## AI observation sessions

An all-AI normal-room match with at least two active seats is an observation session. It is still
the ordinary server-authoritative live game, so the watcher sees the normal live stream followed
by the automatic in-memory post-match replay. Unlike a human match, it has a fixed 25,000-tick
horizon: a winning primary-base elimination on tick 25,000 wins; otherwise the match records a
draw at that tick. This prevents an inconclusive strategy matchup from running indefinitely.

At live start the server assigns `match_run_id`. Once the watched match resolves, it sends
`observationReady` before the normal score/replay transition; the browser retains that id through
automatic post-match replay and prints it as **Observation ID** in the score screen. Persisted rows
store the same value in `matches.match_run_id`, and `GET /api/observations/{matchRunId}` returns
that hidden AI-only row. The returned numeric match id launches the replay with
`POST /api/matches/{id}/replay`; Fly/server logs can be filtered by the identical
`match_run_id`. This is intentionally a narrow lookup rather than adding all AI-only rows to the
player-facing Recent Matches table.

Outcome vocabulary:

- `win`: normal match resolution produced a winner; `winner_name` is the display name of the first
  living winner represented by the replay winner id.
- `draw`: normal match resolution produced no winner; `winner_name` is `null`.
- `aborted`: server-controlled shutdown finalization captured the match before natural
  resolution; `winner_name` is `null`.

Public reads also suppress historical bot/test rows that were written before this eligibility
filter existed, and migration `20260609000002_suppress_automated_match_history.sql` tags those
rows `local_only` instead of deleting them. Migration
`20260628000001_suppress_solo_match_history.sql` marks historical one-human, no-AI sandbox rows
as debug instead of deleting them, preserving replay artifacts while removing them from Recent
Matches.

## Recording gate (`RTS_RECORD_MATCHES`)

The gate controls whether the server writes match rows and replay artifacts. It exists because the
developer runs many local matches and local dev must not upload replay data into the shared
beta/mainline database.

| env state                            | reads work?       | writes happen?      | public servers show row? |
| ------------------------------------ | ----------------- | ------------------- | ------------------------ |
| no `DATABASE_URL`                    | no (returns `[]`) | no                  | no                       |
| `DATABASE_URL` set, gate off / unset | yes               | no                  | no                       |
| `DATABASE_URL` set, gate on          | yes               | yes, public row     | yes                      |

Truthy values: `1`, `true`, `yes`, `on` (case-insensitive). Anything else, including unset, is
off. Beta and mainline deploys must set it to `1`. Local `cargo run` with `DATABASE_URL` can read
history, but with the gate off it does not write rows or replay artifacts.

The implementation: `main.rs` connects the pool once, hands the pool to `AppState` for reads, and
passes it to `Lobby::with_match_history(...)` only when `RTS_RECORD_MATCHES` is truthy. If a room
receives `None`, the `end_match` write branch never fires. `/api/matches` decides whether to
include historical local-only rows from the request peer address. Only loopback peers
(`127.0.0.1` / `::1`) can see those rows.

## Failure modes

- **DB unreachable at boot**: `try_connect_from_env()` logs and returns `None`. Server runs
  without history (reads return `[]`, no writes attempted).
- **DB drops mid-run**: write attempts log an error and continue. No retry, no outbox. This is
  acceptable; match history is non-critical.
- **Migration fails at boot**: `Db::connect` returns `Err`, server runs without history. Check
  `migrations/` filenames are timestamp-prefixed and sequential.
- **Slow write**: tracked detached task means the room is unblocked. Worst case the row appears
  seconds later in `/api/matches` or the run-id observation lookup. During graceful shutdown, the 295 second drain budget reserves
  20 seconds for writes after the forced-abort phase. Logs distinguish all writes completing from
  timeout with the remaining pending count.
- **Deploy drain overran natural completion**: after 260 seconds of natural drain, the lobby asks
  active room tasks to finalize for shutdown. Eligible normal live rooms queue an `aborted`
  replay-backed match row before the server closes WebSocket connections. Operator logs include
  `shutdown natural drain timeout reached; forcing remaining matches`, per-room
  `shutdown finalized active match as aborted`, aggregate `shutdown forced finalization complete`
  or `shutdown forced finalization incomplete`, and then the match-history write wait result.
  A successful beta/mainline validation should also show `match recorded` with `outcome=aborted`
  and `replay=true`, then Recent Matches should show `Aborted` with no winner and a working replay
  launch. `shutdown match-history write wait timed out`, `shutdown forced finalization incomplete`,
  or `failed to record match` means that drain event needs operator follow-up because a hard
  platform kill can still prevent the replay upload.
- **Replay incompatible with current schema/map/faction/loadout**: summaries show
  `replayAvailable: false` with a reason, and launch returns `409` with the same class of
  explanation. Build-SHA drift is warning-compatible (`replayAvailable: true` with a warning);
  schema, map, faction, and loadout drift are rejecting.

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
- **Playable resume from replay**: keep this separate from replay staging. A resume contract needs
  explicit original-seat claiming, faction/loadout validation for those seats, and a transition
  from spectator replay playback to a playable branch/resume room. Group replay lobbies stay
  spectator-only until that contract exists.
- **Crash-safety**: if matches start dropping due to DB outages, add a small bounded
  in-memory outbox in `Db`. Not worth it today.
