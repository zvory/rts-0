# Tank Movement Baseline

Captured for Phase 0 before further tank movement changes. The authoritative fixtures live in
`server/src/game/services/movement.rs` as `tank_phase0_baseline_*` tests.

Run the measurement fixtures with:

```bash
cd server
cargo test tank_phase0_baseline -- --nocapture
```

## Reference Metrics

Captured on the Phase 0 implementation using the current circular tank body model.

| Fixture | Travel ticks | Path length px | Final error px | Facing rad/s | Stuck ticks | Repaths | Collision displacement px | Oil burned |
|---------|--------------|----------------|----------------|--------------|-------------|---------|---------------------------|------------|
| Open ground | 256 | 512.00 | 0.00 | 0.0000 | 0 | 0 | 0.00 | 1.6667 |
| Building corner | 140 | 271.36 | 1.41 | 0.2275 | 0 | 0 | 0.00 | 0.8833 |
| Two-tile corridor | 544 | 1088.00 | 0.00 | 0.0000 | 0 | 0 | 0.00 | 3.5417 |
| Traffic cluster | 260 | 512.04 | 0.00 | 0.0009 | 0 | 0 | 8.00 | 1.6888 |

## Acceptance Criteria

Later phases should keep or intentionally update these criteria:

- Tank self-motion should come from hull/track intent, not sideways translation.
- A tank with a badly misaligned hull should pivot or slow instead of moving at full speed.
- Hull facing should remain stable while the turret uses `weaponFacing` independently.
- Close goals and blocked corners should produce predictable pivot, reverse, or wait behavior.
- Once tank body geometry is refactored, the physical footprint should not leave a large invisible
  clearance bubble beyond the visible hull.

## Expected Later Test Changes

- Phase 1 may change travel ticks, facing change rate, and oil burned as acceleration and stronger
  hull alignment limits land.
- Phase 2 may change building-corner and corridor path length if route following starts cutting or
  preserving corners differently.
- Phase 3 should update footprint-related expectations when tanks stop using a circular body.
- Phase 4 should update traffic-cluster collision displacement once heavy-vehicle traffic rules
  reduce sideways pushes.

## Replay Baseline

For visual comparison, use the existing tech-to-tanks self-play script and persist a replay
artifact:

```bash
cd server
RTS_SELFPLAY_SAVE_REPLAY=tank_phase0_tech_to_tanks cargo test profile_backed_self_play_exercises_tech_to_tanks
```

Then start the server on an unused port and open:

```bash
open "http://localhost:<port>/dev/selfplay?replay=tank_phase0_tech_to_tanks"
```
