# Phase 2 - Path Simplification After A*

Goal: convert noisy tile-center waypoint lists into shorter legal straight-route segments while
preserving tile A* as the reachability solver.

## Design

After A* returns world waypoints, simplify them using the Phase 1 static segment helper. The
simplifier should drop intermediate waypoints when the unit body can legally travel straight from
the current kept point to a later waypoint.

This is often called string pulling or line-of-sight path smoothing. Keep this implementation
simple and deterministic.

## Where to Put It

Prefer a helper near pathing, for example `server/src/game/services/pathing.rs` or a small sibling
module if pathing is getting too large.

Do not put smoothing in client code.

## Algorithm

Given:

- Unit kind.
- Current world position.
- Reverse-ordered waypoint list, where `last()` is the next waypoint.
- Static map and occupancy.

Produce:

- A reverse-ordered waypoint list with unnecessary intermediate waypoints removed.
- The exact final command goal preserved.

One straightforward algorithm:

1. Convert the reverse-ordered path to forward order from current position to final goal.
2. Start at the current position.
3. Find the farthest later waypoint reachable by a legal static segment.
4. Keep that waypoint.
5. Repeat from the kept waypoint until the final goal is kept.
6. Convert back to the existing reverse-ordered storage format.

## Final Goal Handling

`MoveCoordinator::request_path` currently snaps `waypoints[0]` to the exact requested goal because
paths are stored reversed. Preserve this behavior.

Important:

- Smooth after final-goal snapping if possible, so the actual command point participates in segment
  legality.
- If the exact goal is not segment-legal from the previous kept point, the simplifier must keep the
  necessary earlier waypoint instead of forcing a clip.
- If smoothing cannot prove a segment legal, keep the original waypoint.

## Tests

Add tests for:

- Open diagonal route collapses to one final waypoint.
- Route around a blocker keeps a corner waypoint.
- Smoothing preserves the exact final command goal.
- Smoothing never increases waypoint count.
- Smoothing is deterministic across repeated calls.
- Tank smoothing is stricter than infantry when radius matters.

## Acceptance Criteria

- Existing path requests still return `MovePhase::Moving` or `PathFailed` as before.
- Smoothed paths do not allow static clipping.
- No client changes.
- No tank turn-rate or speed tuning yet.
- `DESIGN.md` is updated if this changes the documented pathing/movement contract.

## Common Mistakes

- Dropping a required corner because only the center line was checked.
- Forgetting paths are stored reversed.
- Losing the exact clicked destination.
- Smoothing build/gather staging paths in a way that breaks interaction range. If uncertain, start
  smoothing only normal move/attack-move/chase paths and document the exclusion.

