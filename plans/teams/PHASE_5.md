# Phase 5 - Team-Together Start Positions

Goal: bias start assignment so teammates spawn near each other while keeping the map algorithm
generic.

The current map only has four starts. This phase should support current short-run shapes without
encoding those shapes into the map model.

## Map API

Update `server/src/game/map.rs` and `Game::new_inner`.

Recommended API:

- Keep `Map::generate(player_count, seed)` as a compatibility wrapper for singleton-team FFA tests.
- Add a team-aware generation entry point that accepts player/team assignments in match order.
- Return starts in the same order as match players.

Do not make `Map` depend on lobby presets. It should only know start sites and team ids.

## Assignment Algorithm

The assignment should be deterministic for a seed and generic over team sizes.

Required behavior:

- FFA uses the existing randomized start behavior as closely as possible.
- Team games minimize teammate spread before optimizing enemy distance.
- Ties are deterministic and seed-influenced.
- The algorithm works on a `Vec` of authored start sites, not a fixed four-player array.

One acceptable scoring approach:

- Group players by `teamId` in match order.
- Generate candidate assignments over available authored starts for the current map capacity.
- Score each candidate by:
  - lower same-team pair distance is better,
  - higher nearest enemy-team distance is better,
  - lower imbalance between teams is better,
  - stable seeded tie-breaker.
- For the current four-start map, brute force is acceptable. For future larger maps, guard with a
  greedy fallback if candidate count would explode.

## Expansion Sites

Preserve existing expansion behavior where possible:

- Starting sites keep their authored paired expansion unless existing two-player symmetry logic
  applies.
- Team games should not accidentally assign the same expansion to two active starts.
- Neutral unclaimed expansions still receive resource clusters.

## Current-Map Expectations

For the current map:

- 2v2 should place teammates on closer/together starts rather than fully random opposite corners.
- 1v2 should place the two-player team together when possible.
- 1v3 is best effort because three players cannot all be equally close on a four-corner map.
- FFA should retain the existing shuffled feel.

## Files to Touch

- `docs/design/*.md`
- `server/src/game/map.rs`
- `server/src/game/mod.rs`
- `server/src/game/selfplay.rs` if fixtures need explicit FFA teams
- `server/src/game/replay.rs`
- map assignment tests in `server/src/game/map.rs`

## Tests

Add Rust tests:

- FFA assignment remains deterministic for a seed.
- 2v2 assignment places each teammate pair closer than the farthest enemy-pair baseline.
- 1v2 assignment places the two-player team on nearby starts.
- Start payload reports team ids with assigned start tiles.
- Future-style synthetic map with more than four starts can run team-aware assignment without fixed
  array assumptions.

Run:

```bash
cd server && cargo test
```

## Acceptance Criteria

- Team games get together-biased starts.
- FFA behavior remains compatible.
- Map generation accepts arbitrary team sizes through vector data.
- Current `MAX_PLAYERS` remains map-cap driven, not team-model driven.
