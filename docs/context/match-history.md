# Capsule: match history

Use when changing match-history schema, persistence, the `/api/matches` endpoint, the recording
scope, or the lobby front-page table.

## Read first
- [docs/design/match-history.md](../design/match-history.md) — full design source of truth

## Code seams
- `server/src/db.rs` — pool, migrations, `record_match`, `recent_matches`,
  `replay_artifact_for_match`.
- `server/src/main.rs` — env loading, `/api/matches`, `POST /api/matches/{id}/replay`,
  replay compatibility checks, `RTS_RECORD_MATCHES` public/local scope.
- `server/src/lobby/mod.rs` — `Lobby::with_match_history()` injects pool/scope into rooms;
  replay launch creates spectator replay rooms.
- `server/src/lobby/room_task.rs` — capture metadata at `start_match`, detached write at
  `end_match`.
- `server/migrations/*.sql` — versioned schema. Never hand-apply DDL.
- `client/src/match_history.js` — lobby table renderer and replay launch action.
- `client/src/app.js` — mounts/refreshes the table on lobby show / back-to-lobby; auto-joins
  `?replayRoom=...` launch pages.

## Invariants
- **Server is the only writer.** Clients never write history. `/api/matches` is read-only.
- **Detached write at `end_match`.** A slow Supabase write must never stall the room. Errors
  log and are dropped.
- **Recording scope.** Beta/mainline writes are enabled only when `RTS_RECORD_MATCHES` is truthy.
  Deployed player-vs-AI and AI-only matches get replay-backed rows unless they are
  dev/scenario/replay rooms or automated test fingerprints. One-human, no-AI sandbox matches are
  recorded as debug sessions so their replays stay available for diagnostics without appearing in
  Recent Matches. Local `cargo run` with the gate off can read history but does not upload rows or
  replay artifacts.
- **Recent Matches visibility.** `/api/matches` returns only rows with `human_count >= 1` and
  `debug_mode = false`, and explicitly suppresses historical one-human, one-participant rows. Solo
  sandbox rows, AI-only rows, and historical debug rows can be stored for replay launch without
  appearing in the lobby table. New live product rows write `debug_mode = false` except one-human,
  no-AI sandbox rows, which write `true`.
- **Score-screen schema.** `score_screen` is JSONB holding `Vec<PlayerScore>` from
  `contract::PlayerScore`. Adding fields requires no migration.
- **Replay storage.** `match_replays.artifact_json` stores `ReplayArtifactV1`; summaries and
  launch strictly check artifact schema, map schema, map hash, and faction/loadout validity.
  Build-SHA mismatches stay launchable with a warning because replay playback is attempted across
  build drift.
- **TLS to Supabase.** `DATABASE_URL` must include `?sslmode=require`.

## Cross-capsule triggers
- Changing `PlayerScore` → also update [protocol.md](protocol.md) and the score-screen renderer.
- New deploy/env var → update [deployment.md](deployment.md) and `docs/fly.md`.
- Changing the lobby DOM around the history table → update [client-ui.md](client-ui.md).
