# Phase 2 - Make Live Profiles Converge

## Goal

Make every live AI profile use the same tank late-game production and attack policy once it reaches
its late-game trigger, while preserving each profile's early opening.

This phase is where behavior should start to change intentionally.

## Plain-Language Intent

The AI should stop having three different versions of tank late game. The opening can decide how it
gets there. Once it is there, the late-game plan should be the same.

For now, "same late game" means the same tank+rifle production priorities and the same tank-required
outbound wave policy. It does not mean the AI has formation micro or a sophisticated army
composition system.

## Profile-Specific Expectations

### `tech_to_tanks`

This profile is already closest to the desired late game. It should use the shared tank late-game
policy directly for its main production and attack plan, or through a zero/early transition if that
keeps the code shape cleaner.

At the end:

- it still techs to tanks quickly;
- it still requires tank tech;
- it uses the same late-game tank+rifle production and wave settings as other live profiles.

### `rifle_flood_full_saturation`

This profile should keep its macro rifle opening and its 50-supply pivot idea unless there is a
separate balance reason to change the trigger.

At the end:

- before the pivot, it still saturates steel and produces rifle waves;
- after the pivot, it uses the shared tank late-game production and attack policy;
- the transition remains data-driven through profile policy rather than controller-side profile
  switching.

### `rifle_flood_fast`

This profile should keep its proxy pressure opening. It may still have a hard time reaching late
game because the opening is economically thin. That is acceptable for this phase.

At the end:

- before the pivot, it still proxy rushes;
- after the pivot, if it reaches the pivot, it uses the shared tank late-game policy;
- economic recovery is not solved here.

## Trigger Policy

Do not make every profile use the same trigger in this phase unless the current triggers are already
wrong for gameplay.

Reasonable first pass:

- keep `rifle_flood_full_saturation` at its existing 50 supply transition;
- keep `rifle_flood_fast` at its existing 70 supply transition until its recovery plan is designed;
- let `tech_to_tanks` use the shared policy as its baseline tank plan.

This separates two concerns:

- convergence destination: shared now;
- timing and recovery: profile-specific until tuned.

## Implementation Notes

- Reuse the shared constants or helpers from Phase 1.
- Keep `active_tech_transition`, `active_production_policy`, and `active_attack_policy` simple.
- Avoid adding a generic phase engine unless duplication remains painful after this phase.
- Be careful with `AiDecisionMemory`: it tracks attack size, last attack tick, and attack policy.
  Existing `ensure_attack_policy` behavior should be audited so the pivot does not create strange
  repeated resets every think tick.
- Do not rename profiles. The opening identity is still the profile identity.

## Expected Behavior At End

At the end:

- all live profiles converge on the same late-game production policy;
- all live profiles converge on the same late-game attack policy;
- early profile differences still exist;
- `tech_to_tanks` does not become the name for "late game";
- `rifle_flood_fast` may still fail to reach late game in many matches.

## Tests

Focused tests should prove:

- `tech_to_tanks` active production/attack policy matches the shared tank late-game policy;
- `rifle_flood_full_saturation` uses rifle-only attack before the transition and shared tank late
  game after the transition;
- `rifle_flood_fast` uses rifle-only/proxy pressure behavior before the transition and shared tank
  late game after the transition;
- profile ids and live profile pool remain stable;
- the shared attack policy requires a tank before outbound waves launch.

Tests can be unit-level around profile data and decision output. Full self-play is useful later,
but the core convergence should not depend on long matches to detect.

## Done When

- One code path or one shared data object defines late-game tank production and attack settings.
- Live profiles use that shared policy after their appropriate trigger.
- The AI still emits ordinary commands through existing command validation.
- Patch notes can say: "AI late-game tank production and outbound wave behavior now uses a shared
  policy across live profiles."
