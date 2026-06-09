# Capsule: match history

Use when changing match-history schema, persistence, the `/api/matches` endpoint, the recording
gate, or the lobby front-page table.

## Read first
- [docs/design/match-history.md](../design/match-history.md) — full design source of truth

## Code seams
- `server/src/db.rs` — pool, migrations, `record_match`, `recent_matches`.
- `server/src/main.rs` — env loading, `/api/matches`, `RTS_RECORD_MATCHES` gate.
- `server/src/lobby/mod.rs` — `Lobby::with_db()` injects pool into rooms.
- `server/src/lobby/room_task.rs` — capture metadata at `start_match`, detached write at
  `end_match`.
- `server/migrations/*.sql` — versioned schema. Never hand-apply DDL.
- `client/src/match_history.js` — lobby table renderer.
- `client/src/app.js` — mounts/refreshes the table on lobby show / back-to-lobby.

## Invariants
- **Server is the only writer.** Clients never write history. `/api/matches` is read-only.
- **Detached write at `end_match`.** A slow Supabase write must never stall the room. Errors
  log and are dropped.
- **Recording gate.** Only matches with `RTS_RECORD_MATCHES` truthy AND `match_player_count >= 2`
  AND not a dev/scenario/replay room get a row. Local `cargo run` reads but never writes.
- **Score-screen schema.** `score_screen` is JSONB holding `Vec<PlayerScore>` from
  `contract::PlayerScore`. Adding fields requires no migration.
- **TLS to Supabase.** `DATABASE_URL` must include `?sslmode=require`.

## Cross-capsule triggers
- Changing `PlayerScore` → also update [protocol.md](protocol.md) and the score-screen renderer.
- New deploy/env var → update [deployment.md](deployment.md) and `docs/fly.md`.
- Changing the lobby DOM around the history table → update [client-ui.md](client-ui.md).
