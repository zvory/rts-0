# Phase 0 - Weighted Collision and Footing

Goal: replace symmetric 50/50 unit overlap resolution with resistance-weighted collision while
preserving the current pass-through worker exceptions.

This is the first code phase because it changes local occupancy without changing pathfinding,
commands, snapshots, or the client.

## Scope

In scope:

- Add a small footing/profile model inside `server/src/game/services/movement.rs`.
- Keep `is_collision_anchored` as the public "ghost/pass-through" predicate used by invariants.
- Change `resolve_collisions` so non-ghost pairs split overlap by resistance instead of always
  50/50.
- Preserve passability checks and deterministic pair iteration.
- Add focused Rust tests for each profile interaction.

Out of scope:

- No pathfinding changes.
- No formation goal changes.
- No protocol or client changes.
- No local steering beyond the existing sidestep behavior.
- No new hold-position command.

## Files To Touch

- `server/src/game/services/movement.rs`
- `server/src/game/invariants.rs` only if the anchored helper signature must change. Prefer not to.
- `DESIGN.md` hardening section if the collision behavior description changes.

## Implementation Steps

1. Add a local enum near the collision helpers:

   ```rust
   #[derive(Clone, Copy, Debug, PartialEq, Eq)]
   enum FootingProfile {
       Ghost,
       Soft,
       Firm,
       Braced,
       Heavy,
   }
   ```

2. Add helpers in `movement.rs`:

   ```rust
   fn footing_profile(e: &Entity) -> FootingProfile
   fn footing_resistance(profile: FootingProfile) -> f32
   ```

3. Keep this initial classification:

   - `Ghost`: worker in `GatherPhase::Harvesting`; worker in `BuildPhase::Constructing`.
   - `Braced`: machine gunner in `WeaponSetup::SettingUp` or `WeaponSetup::Deployed`.
   - `Heavy`: tank.
   - `Firm`: non-tank, non-MG combat unit with `target_id().is_some()` and `path_is_empty()`.
     This captures units standing to fire without making all idle army units firm.
   - `Soft`: all other mobile units.

4. Implement resistance values as local constants. Suggested starting values:

   - `Soft`: `1.0`
   - `Firm`: `3.0`
   - `Braced`: `8.0`
   - `Heavy`: `12.0`

   `Ghost` must never reach resistance math.

5. Update `is_collision_anchored(e)` to return `footing_profile(e) == FootingProfile::Ghost`.
   This preserves the invariant meaning: anchored units are skipped entirely and can be passed
   through.

6. In `resolve_collisions`, collect radius plus profile for both entities. Skip pairs if either
   profile is `Ghost`.

7. Replace the symmetric half-overlap split with:

   ```text
   total = resistance_a + resistance_b
   a_share = resistance_b / total
   b_share = resistance_a / total
   ```

   This means the lower-resistance unit moves more.

8. Preserve the existing blocked-push transfer behavior:

   - If both weighted targets are passable, apply both.
   - If one weighted target is blocked, try assigning that side's remaining push to the other unit.
   - If neither side can move, leave the bounded residual overlap.

9. Keep pair iteration deterministic. Do not add unordered collection iteration to the movement
   tick path.

10. Update collision comments and `DESIGN.md` hardening text so "anchored" is described as ghost
    pass-through and non-ghost collision is described as resistance-weighted.

## Tests

Add or update Rust tests in `server/src/game/services/movement.rs`:

- `soft_units_still_split_push_evenly`: two moving riflemen separate roughly symmetrically.
- `tank_pushes_soft_infantry_more_than_it_moves`: tank/rifleman overlap mostly displaces the
  rifleman.
- `braced_machine_gunner_holds_ground_against_soft_unit`: deployed MG moves much less than a
  rifleman.
- `firing_rifleman_is_firmer_than_moving_rifleman`: firing rifleman moves less than a soft rifleman.
- Existing `harvesting_worker_is_fully_exempt_from_collision` still passes.
- Existing invariant overlap checks still pass.

Run:

```bash
cd server && cargo fmt && cargo test movement::tests
cd server && cargo test
```

## Acceptance Criteria

- Harvesters and active builders are still pass-through.
- Moving/idle soft units can be shoved aside by heavier or firmer units.
- Deployed or setting-up machine gunners behave like solid positional commitments, not ghosts.
- Tanks physically dominate soft infantry in overlap resolution.
- Equal-profile moving units still separate cleanly.
- No new panics, unwraps, or unchecked indexing are added to `Game::tick()` paths.
- No protocol or client files change in this phase.
