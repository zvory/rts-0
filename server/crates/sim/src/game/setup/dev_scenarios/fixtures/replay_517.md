# Replay 517: first mortar beside the water

Source: mainline match 517, Soupman vs Alex, Classic, started 2026-11-14
21:40:19 UTC. Recorded build `8274fb69e7c0`, seed 654567149, duration 4750 ticks.
Alex is player 4. Download: `GET /api/matches/517/replay-artifact`.

## Finding and limits

The reported first-mortar lock is **not reproduced** by this artifact. Re-simulation on the
recorded commit matches the recorded final-state hash exactly, after applying the recorded
elimination at tick 4750. Mortar Team 265 spawns at `(1456, 1072)` on tick 4126, moves on tick
4127, and follows all six Move commands at ticks 4162, 4167, 4173, 4178, 4184 and 4189. It is
at `(1625.406005859375, 1029.0118408203125)` on tick 4225. The Mortar Fire ability on tick 4226
then deliberately ends its movement. Another Move at tick 4425 starts movement again. Its only
multi-tick stationary active order is an explicit Attack at ticks 4701–4730, away from the
producer. There are no failed escape orders at the Gun Works in this replay.

The preceding match, 516 (alex vs soupman, Classic, same build, seed 315718799), was also
re-simulated to its final tick 20086 and matched its recorded final-state hash. Its first
Mortar Team 258 spawns on tick 4418 at `(2192, 368)`, follows the rally, then the two recorded
group moves at ticks 4435 and 4439, and leaves successfully. This does not establish what the
player saw in the client or rule out a lock elsewhere; it rules out claiming this particular
first-mortar spawn as an authoritative simulation lock in either of today's two recordings.

Full-match playback must use the recorded commit. Subsequent Resource Depot price changes on
main alter economy, production timing and entity ids; replaying the complete command stream
on that newer balance can even prevent the mortar's production. A build-warning replay is
not interchangeable with the historical source for this diagnosis.

## Minimal comparison scene

Open `/dev/scenarios?id=replay_517_mortar_water_exit&unit=mortar_team&count=1`, or Interact
`dev-scenario open` with that id, unit and count.

- `replay_517_gunworks.json`: Gun Works 243 extracted at tick 4125, one tick before completion.
  Its paid production is at 459/460 ticks. Repeat production is removed to keep one mortar.
- `replay_517_mortar_orders.json`: the six original single-unit Move commands, unchanged.
- `replay_517_mortar_positions.json`: all 100 post-tick source positions, 4126–4225, extracted
  independently from the complete, hash-verified match on its recorded commit.

Only the producer is present initially. Production creates mortar id 265 through the normal
spawn and rally code. The original Classic terrain and terrain metadata remain in tiles
x=38–52, y=26–40, including roads and water. Outside this patch the map is grass; unrelated
entities, base sites and resources are removed. The scene starts at source tick 4125. The
regression checks every resulting position exactly against the full source replay, including
spawn and the six follow-up commands. No unit is immobilized or teleported. The command schedule
ends before the Mortar Fire ability; the final Move can complete normally in the scene.

## Collision diagnosis and fix candidates

The nominal stat radius is 18 pixels, but the effective physical body is a circle of radius
13: half the 24-pixel support-weapon width plus one pixel of clearance. It is 26 pixels wide
on 32-pixel tiles, independent of facing. A straight one-tile lane has six pixels of total
spare width. Rotation clearance cannot explain a mortar lock by itself.

Relevant code: `services/geometry.rs::unit_body_with_facing`, rules `balance/bodies.rs`,
`services/standability.rs`, and `services/move_coordinator.rs::find_spawn_point`.

The spawn search checks standability and unit overlap at the candidate point and prefers a gap
from the producer; it does not prove that the candidate connects to the rally or open ground.
Thus legal-but-trapped exits remain a plausible general failure mode, **not an observed cause
of this report**. Body width also does not prove that a diagonal pinch or turn is traversable.

Options once the actual lock is available:

1. **Validate producer egress.** Require a body-traversable connection from the spawn to usable
   ground/rally, search another producer edge when unavailable, and retain the completed queue
   item if no exit exists. This targets trapped spawns without changing unit size. Use bounded
   searches and cache static connectivity so a blocked producer does not perform an expensive
   flood-fill every tick.
2. **Repair planner/executor disagreement.** If a valid physical route exists but orders stall,
   compare the planned segments with the same body-aware standability used during movement;
   cover start-tile escape, diagonal corners and recovery. This preserves physical dimensions.
3. **Reduce the effective collision diameter consistently.** Appropriate only if the intended
   design allows narrower passages. It changes crowding and terrain access globally; changing
   the nominal 18-pixel stat alone does not change this mortar's 13-pixel physical body.
4. **Bounded squeezing.** Define a smaller minimum body and apply it consistently to planning,
   spawn, execution and collision. This is a larger gameplay/geometry change with clipping and
   determinism risks; ad hoc permission to cross water/buildings is not a sound recovery rule.
5. **Placement/feedback safeguards.** Detect missing exits when placing a producer, or show a
   blocked-production reason. This improves prevention/diagnosis but cannot fix a movement bug
   in an otherwise valid exit.

Recommendation: keep collision dimensions unchanged until a source replay/time/entity actually
shows the lock. Then prefer egress validation for an isolated pocket or planner/executor repair
for a navigable passage. The current comparison fixture supplies a known-good shoreline case
that any proposed fix must preserve.

Focused check:

```sh
cargo test --release --manifest-path server/Cargo.toml -p rts-sim replay_517
```
