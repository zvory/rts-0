# Phase 5 - Persistence and Match History Entry Points

## Objective

Persist replay artifacts for eligible matches and let players launch a recent match replay from the
match-history UI when it is compatible with the current server build.

## Database Work

- Add a migration for replay storage. Recommended shape:
  - `match_replays`
  - `match_id`
  - `artifact_schema_version`
  - `build_sha`
  - `map_name`
  - `map_schema_version`
  - `map_hash`
  - `duration_ticks`
  - `artifact_json`
  - timestamps
- Keep `matches.score_screen` as score data, not replay storage.
- Insert replay rows from the same detached end-match write path or a second detached task.
- Keep local-only/public visibility consistent with match history.

## API Work

- Extend `/api/matches` summaries with a replay availability flag and incompatibility reason.
- Add a read-only replay launch endpoint or room token flow.
- Validate build SHA and map compatibility before creating a replay room.
- Return clear errors for incompatible replays instead of trying to partially play them.

## Client Work

- Add a "Watch replay" action to expanded match-history rows when available.
- Surface incompatible replay reasons in the row detail.
- Join the replay viewer through the normal WebSocket flow using a server-provided room id or token.

## Verification

- Migration test against empty and existing match-history schemas.
- API test that local-only replay availability follows local-only match visibility.
- API test that incompatible build SHA/map hash prevents replay launch.
- Client test that a match-history replay action opens replay mode.

## Player-Facing Outcome

Players can open recent compatible match replays from match history instead of only watching the
automatic post-match replay.

