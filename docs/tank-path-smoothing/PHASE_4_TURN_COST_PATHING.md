# Phase 4 - Optional Tank Turn-Cost Pathing

Goal: make tank routes prefer fewer bends when multiple tile paths are otherwise similar.

This phase is optional. Do it only if Phase 2 and Phase 3 still leave tanks taking ugly zigzag paths
in open or semi-open terrain.

## Design

Tile A* currently scores movement by cardinal and diagonal distance. It does not remember incoming
direction, so paths with many direction changes can tie with smoother paths.

For tanks, add a direction-change penalty so A* prefers straighter routes:

- State includes tile position and incoming direction.
- Moving in the same direction has no turn penalty.
- Changing direction adds a small deterministic cost.
- The penalty should be low enough that tanks still take a necessary shorter route around obstacles.

## Scope Control

Do not change infantry pathing unless there is a clear reason. Tanks are the unit kind with the
visual hull problem and large radius.

Prefer a path request flag or kind-based branch inside the pathing service over duplicating the
entire pathfinder.

## Suggested Cost Shape

Start conservative:

- Cardinal move: existing 10.
- Diagonal move: existing 14.
- Direction change: add 2-5.
- Reversing direction: add more only if tests prove it matters.

Do not tune by feel alone. Compare route lengths and turn counts in tests.

## Tests

Add tests for:

- In open terrain, tank path between two points has fewer heading changes than the old route or an
  infantry route.
- Around an obstacle, the tank still finds a valid route.
- If a bend is required, the path keeps it.
- Repeated path requests return identical waypoint sequences.
- Path expansion budget behavior remains bounded.

## Acceptance Criteria

- Tank paths prefer fewer turns without losing reachability.
- Determinism is preserved.
- Existing pathing cache keys include any new route-shaping inputs. A tank path and infantry path
  must not incorrectly share a cache entry if their costs/passability differ.
- Pathing performance remains acceptable under `cargo test` and integration tests.

## Common Mistakes

- Adding direction to search state but not to visited/g-score keys.
- Reusing cached paths across different movement cost models.
- Making the turn penalty so high that tanks take absurd detours.
- Changing all unit pathing when only tanks need it.

