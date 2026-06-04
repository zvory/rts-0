# Phase 5 - Facing-Aware Tank Damage

Goal: make stable tank body facing matter for anti-armor damage. Front, side, and rear hits should
produce different damage only where that distinction is useful.

## Dependencies

- Phase 3 must be complete. Tank body facing must be stable before damage uses it.
- Phase 4 is recommended but not strictly required.

## Scope

In scope:

- Add pure facing-arc helpers in `server/src/rules/combat.rs`.
- Apply facing modifiers to tank victims hit by tank or AT-style weapons.
- Add combat tests for front, side, and rear hits.
- Update `DESIGN.md` balance/combat notes.

Out of scope:

- No UI display for armor arcs.
- No terrain-facing interaction.
- No infantry-facing damage.
- No building-facing damage.
- No protocol change.

## Files To Touch

- `server/src/rules/combat.rs`
- `server/src/game/services/combat.rs`
- `DESIGN.md`
- combat tests

## Damage Rules

Use body `facing` on the victim tank. Classify the attacker position relative to the victim:

```text
front: absolute angle from victim facing <= 45 degrees
side: 45 degrees < absolute angle <= 135 degrees
rear: absolute angle > 135 degrees
```

Balance multipliers:

- Front: `1.0`
- Side: `1.25`
- Rear: `1.75`

Apply only when:

- victim kind is `Tank`, and
- attacker kind is `Tank` or `AtTeam`.

Everything else keeps current `effective_damage` behavior.

## Implementation Steps

1. Add a pure enum and helpers in `rules::combat`:

   ```rust
   pub enum ArmorFacing {
       Front,
       Side,
       Rear,
   }

   pub fn classify_armor_facing(victim_facing: f32, victim_pos: (f32, f32), attacker_pos: (f32, f32)) -> ArmorFacing
   pub fn facing_damage_multiplier(attacker_kind: EntityKind, victim_kind: EntityKind, facing: ArmorFacing) -> f32
   ```

2. Keep the existing `effective_damage(...)` function for callers that do not have positions.

3. Add a new helper that combat can call with context, for example:

   ```rust
   pub fn effective_damage_with_facing(
       attacker_kind: EntityKind,
       victim_kind: EntityKind,
       base_dmg: u32,
       victim_terrain: Option<TerrainKind>,
       victim_facing: Option<f32>,
       victim_pos: (f32, f32),
       attacker_pos: (f32, f32),
   ) -> u32
   ```

4. Implement by first calling existing `effective_damage`, then applying facing multiplier when all
   gating conditions pass.

5. Round deterministically. Prefer:

   ```rust
   ((damage as f32) * multiplier).round().max(0.0) as u32
   ```

   Keep final damage saturating and nonnegative.

6. In `combat.rs`, pass attacker and victim positions plus victim facing into the new helper for
   primary target damage.

7. Decide whether overpenetration uses facing:

   - Preferred first implementation: use facing for overpenetration victims too, because attacker
     and victim positions are already known.
   - If this makes the patch too large, document that overpenetration keeps old damage for now and
     add a TODO in the phase change.

8. Update `DESIGN.md` combat/balance text with the first rules.

## Tests

Add Rust tests:

- `tank_front_hit_reduces_at_damage`.
- `tank_side_hit_uses_normal_at_damage`.
- `tank_rear_hit_boosts_at_damage`.
- `tank_shell_uses_same_facing_modifiers_against_tank`.
- `rifleman_vs_rifleman_ignores_facing`.
- `tank_vs_building_ignores_facing`.
- `facing_classification_wraps_around_pi`.
- Combat integration test proves victim HP differs for front and rear hits from the same attacker
  kind.

Run:

```bash
cd server && cargo fmt && cargo test rules::combat combat::tests
cd server && cargo test
```

## Acceptance Criteria

- Tank body facing affects tank/AT damage against tank victims.
- Non-tank combat damage remains unchanged unless explicitly covered by tests.
- Damage classification is deterministic and handles angle wraparound.
- No wire protocol changes are made in this phase.
