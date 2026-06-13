# Phase 6 - Team-Aware Starts on Authored Maps

Status: planned.

## Goal

Bias start placement so teammates spawn near each other on authored maps. The algorithm should stay
generic over team sizes and map layouts, not hardcode 1v2, 1v3, 2v2, or four fixed corners.

## Scope

- Extend map generation/loading to accept ordered player/team assignments.
- Preserve `Map::generate(player_count, seed)` or a compatibility wrapper for singleton-team FFA
  tests and callers.
- For authored maps, select candidate layouts matching player count, then assign layout slots to
  players using team-aware scoring.
- Keep FFA behavior as close as practical to today's seeded layout/slot shuffle.
- For team games, prefer:
  - lower teammate spread
  - higher nearest enemy-team distance
  - balanced team exposure where possible
  - deterministic seed-influenced tie breaks
- Preserve authored main/natural pairings and avoid duplicate expansion assignment.
- Add a synthetic larger authored layout test so the implementation does not assume exactly four
  starts or two teams.

## Expected Touch Points

- `docs/design/server-sim.md`
- `docs/design/testing.md`
- `server/crates/sim/src/game/map.rs`
- `server/crates/sim/src/game/map/authored.rs`
- `server/crates/sim/src/game/setup.rs`
- `server/crates/sim/src/game/replay.rs`
- `server/crates/ai/src/selfplay/` fixtures if explicit FFA teams are required
- authored map tests and fixtures
- `tests/team_integration.mjs`

## Verification

```bash
cd server && cargo test map --workspace
cd server && cargo test setup --workspace
node tests/team_integration.mjs
```

Required automated scenarios:

- FFA assignment remains deterministic for a seed.
- 2v2 places teammate pairs closer than a random opposite-corner baseline on current authored maps.
- 1v2 places the two-player team together when the layout supports it.
- 1v3 is deterministic and best-effort on four-start maps.
- A synthetic map with more than four starts can assign arbitrary team sizes without fixed arrays.
- Start payload reports correct `teamId` next to assigned start tiles.

## Acceptance Criteria

- Team starts are together-biased on current authored maps.
- FFA start assignment remains compatible enough for existing tests and player expectations.
- Map assignment accepts vector player/team data and does not encode lobby preset names.
- Replay capture/playback preserves start assignment through player/team data and seed.

## Manual Testing Focus

Use a dev/scenario or automated start payload dump to inspect one 2v2 start assignment visually if
the automated distance assertions are hard to interpret.

## Handoff Requirements

The phase handoff must describe the scoring algorithm, note any FFA behavior differences, and list
the authored maps covered by tests.
