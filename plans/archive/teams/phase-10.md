# Phase 10 - Team-Aware Starts on Authored Maps

Status: implemented.

## Goal

Bias start placement so teammates spawn near each other on authored maps. The algorithm should stay
generic over team sizes and map layouts, not hardcode 1v2, 1v3, 2v2, or four fixed corners.

## Scope

- Extend map generation/loading to accept ordered player/team assignments.
- Preserve `Map::generate(player_count, seed)` or a compatibility wrapper for singleton-team FFA
  tests and callers.
- Define start assignment order explicitly: the simulation receives an ordered match-player vector
  with team ids, and map assignment returns starts for that same order.
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
- Do not encode lobby preset names into simulation or map code.

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

- FFA assignment remains deterministic for a seed, with documented differences if any.
- 2v2 places teammate pairs closer than a random opposite-corner baseline on current authored maps.
- 1v2 places the two-player team together when the layout supports it.
- 1v3 is deterministic and best-effort on four-start maps.
- A synthetic map with more than four starts can assign arbitrary team sizes without fixed arrays.
- Start payload reports correct `teamId` next to assigned start tiles.
- Replay capture/playback preserves start assignment through player/team data and seed.

## Acceptance Criteria

- Team starts are together-biased on current authored maps.
- FFA start assignment remains compatible enough for existing tests and player expectations.
- Map assignment accepts vector player/team data and does not encode lobby preset names.
- Tests no longer rely on lobby seat order as an implicit team-start algorithm.

## Manual Testing Focus

Use a dev/scenario or automated start payload dump to inspect one 2v2 start assignment visually if
the automated distance assertions are hard to interpret.

## Handoff Requirements

The phase handoff must describe the scoring algorithm, note any FFA behavior differences, and list
the authored maps covered by tests.

## Implementation Handoff

- Authored maps now accept an ordered player/team vector for start assignment.
- Singleton-team FFA keeps the previous behavior exactly: choose the matching authored layout by
  `seed % layout_count`, shuffle complete main/natural slots with the seed, and assign in player
  order.
- Team games evaluate every matching authored layout and every slot assignment. The score prefers
  lower teammate distance spread first, then higher nearest enemy-team distance, then lower exposure
  imbalance, with a deterministic seed/player/layout hash as the final tie-break.
- Slot assignment moves whole authored slots, so main/natural pairings stay intact and expansions
  are not duplicated.
- Automated map coverage includes the bundled `Default` map for 2v2, 1v2, 1v3, FFA compatibility,
  start payload team ids, and replay reconstruction; `Low Econ` remains covered by the existing
  adjacent-layout natural pairing test. A synthetic six-start authored map verifies arbitrary team
  sizes without fixed four-corner arrays.

## Patch Notes

- Team matches on authored maps now bias teammates to spawn near each other instead of relying on
  lobby seat order plus random slot shuffle.
- FFA authored start randomization remains compatible with the previous seed behavior.
