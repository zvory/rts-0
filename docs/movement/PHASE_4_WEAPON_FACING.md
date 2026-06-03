# Phase 4 - Weapon Facing and Aim Gates

Goal: split weapon/barrel facing from body facing. Tanks should rotate hull and turret separately,
and firing should wait until the weapon is aimed.

This is the first movement phase that changes the wire protocol.

## Dependencies

- Phase 2 for client angle interpolation.
- Phase 3 for stable tank body facing.

## Scope

In scope:

- Add `weaponFacing?: f32` to entity snapshots.
- Mirror compact snapshot encoding and decoding.
- Add weapon-facing state to combat-capable entities.
- Aim-gate tank firing based on weapon angle.
- Render tank barrels from `weaponFacing` while hulls still use `facing`.
- Preserve fog safety for hidden target-derived angles.

Out of scope:

- No side/rear damage yet.
- No full AT-gun arc system.
- No new command fields.
- No projectile simulation.

## Files To Touch

- `DESIGN.md`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- `server/src/game/entity.rs`
- `server/src/rules/projection.rs`
- `server/src/game/services/combat.rs`
- `client/src/state.js`
- `client/src/renderer.js`
- protocol/client tests

## Protocol Requirements

Add `weaponFacing?: f32` after `facing?: f32` in the semantic `Entity` shape in `DESIGN.md`.

In compact snapshots:

- Add a new optional slot directly after `facing`.
- Bump the positional indexes for later optional fields.
- Update max compact entity field count in `client/src/protocol.js`.
- Add protocol serialization/deserialization tests.

Do not remove or rename `facing`. It remains body/hull facing.

## Fog Safety

`weaponFacing` can reveal hidden target direction. Projection must obey this rule:

- Own entities: include `weaponFacing`.
- Full-world dev snapshots: include `weaponFacing`.
- Enemy/allied visible entities with no active combat target: include `weaponFacing` if useful.
- Enemy visible entities aiming at a target the viewer cannot see: omit `weaponFacing` or send body
  `facing` instead. Prefer omitting so the client naturally falls back to body facing.
- If `targetId` would be visible under the existing `target_id` gate, `weaponFacing` may be visible.

Keep this logic in `rules::projection`, not scattered in combat or serialization code.

## Implementation Steps

1. Extend `CombatState` in `server/src/game/entity.rs`:

   ```rust
   pub weapon_facing: f32,
   pub desired_weapon_facing: f32,
   ```

   Initialize both to `0.0`.

2. Add entity accessors:

   ```rust
   pub fn weapon_facing(&self) -> Option<f32>
   pub fn set_weapon_facing(&mut self, facing: f32)
   pub fn set_desired_weapon_facing(&mut self, facing: f32)
   ```

   Return `None` for entities without combat state.

3. Decide first-unit behavior:

   - Tanks: independent `weapon_facing`, limited traverse, aim gate.
   - Machine gunners and infantry: weapon can mirror body `facing` in this phase unless a test needs
     otherwise.

4. Add combat constants:

   ```rust
   const TANK_TURRET_TURN_RATE_RAD_PER_TICK: f32 = 0.070;
   const TANK_TURRET_FIRE_TOLERANCE_RAD: f32 = 0.18;
   ```

5. In `combat_system`, when a target is selected:

   - Compute target angle from attacker to target.
   - For tanks, rotate `weapon_facing` toward target angle by turret turn rate.
   - Fire only if range, cooldown, setup requirements, and weapon-angle tolerance all pass.
   - Do not require hull/body alignment once the turret exists.

6. When a tank has no target, let weapon facing relax toward body facing at turret turn rate. This
   avoids stale barrels pointing at dead targets forever.

7. Add `weapon_facing` to `EntityView` and compact snapshot serialization.

8. Update `client/src/protocol.js` compact decoder and object-shaped snapshot handling if needed.

9. Update `client/src/state.js` to angle-interpolate `weaponFacing` with the same helper used for
   `facing`.

10. Update `client/src/renderer.js`:

    - Tank body polygon uses `facing`.
    - Tank barrel uses `weaponFacing ?? facing`.
    - Muzzle flash origin uses `weaponFacing ?? facing` for tanks.

11. Update `DESIGN.md` protocol and rendering sections.

## Tests

Add or update tests:

- Rust protocol compact snapshot includes `weaponFacing` at the documented index.
- JS compact decoder reads `weaponFacing`.
- `GameState.entitiesInterpolated` shortest-arc interpolates `weaponFacing`.
- Tank turret facing changes gradually toward a target.
- Tank firing is delayed until turret is within tolerance.
- Tank can fire at a target outside hull facing once turret is aligned.
- Projection omits enemy `weaponFacing` when it is aimed at a hidden target.
- Renderer smoke still passes.

Run:

```bash
cd server && cargo fmt && cargo test
node tests/client_contracts.mjs
cd tests && npm install && node client_smoke.mjs
```

## Acceptance Criteria

- `facing` and `weaponFacing` are both documented and mirrored.
- Tanks render hull and barrel independently.
- Tank shots respect turret traverse.
- No snapshot leaks hidden target direction through `targetId`, events, or `weaponFacing`.
- Existing clients/tests using compact snapshots are updated, not left silently mis-decoding fields.
