# Phase 1 - Static Line-of-Sight and Swept-Body Checks

Goal: add a reusable server helper that answers: "Can this unit body travel in a straight line from
point A to point B without hitting static blockers?"

This is the foundation for path simplification and long lookahead. Do not simplify paths until this
helper is reliable.

## Design

Add a small static segment legality helper in or near `server/src/game/services/standability.rs`.
Use the existing standability rules as the final authority.

The helper should be conceptually:

```rust
pub(crate) fn unit_static_segment_standable(
    map: &Map,
    occ: &Occupancy,
    kind: EntityKind,
    from: (f32, f32),
    to: (f32, f32),
) -> bool
```

The exact name can change, but keep the responsibility narrow.

## Required Behavior

- Return true when the unit can stand at sampled points along the segment.
- Return false if any sampled point violates `unit_static_standable`.
- Always check the exact start and end points.
- Clamp nothing inside the helper. A caller that passes out-of-bounds coordinates should get false.
- Treat dynamic unit bodies as irrelevant in this phase.

## Sampling Guidance

Use deterministic fixed spacing. A conservative spacing is one quarter tile or smaller:

- `step_px <= TILE_SIZE / 4.0`
- `steps = ceil(distance / step_px)`
- sample `i / steps` for `i = 0..=steps`

This is intentionally simple. It is acceptable because paths are short enough and this helper runs
only on freshly computed paths, not every entity every tick.

If an implementer wants a more exact grid traversal later, keep it as a follow-up. Do not combine a
new algorithm and path simplification in one phase.

## Tests

Add tests for:

- Straight open segment succeeds.
- Segment crossing water/rock fails.
- Segment crossing a building footprint fails.
- Segment ending out of bounds fails.
- Tank segment near a blocker fails when tank radius would clip, even if the center line is open.
- Rifleman segment can pass where a tank cannot, if such a fixture is easy to construct.

## Acceptance Criteria

- Helper has no side effects and does not mutate entities.
- Tests cover both pass and fail cases.
- The helper uses existing static standability semantics.
- No movement behavior changes yet.

## Common Mistakes

- Checking only tile centers instead of body standability.
- Skipping the endpoint.
- Letting out-of-bounds points pass because the map clamps elsewhere.
- Querying or reserving dynamic unit occupancy.

