# Bow Novice

## Phase 0: approved brief

Add an immediately available Cultivator ranged infantry unit, Bow Novice, reusing the
preserved Wind archer artwork and idle/run/draw/release/recovery animation. Rename the
existing base Novice to Sword Disciple (keep its `warrior` identity and rules).
UI description: “Ranged cultivator who earns insight and evolves into a Dao Adept.”
Its purpose is early ranged support with eventual Dao progression; Riflemen retain better
immediate combat stats per steel. Focus fire and cheaper ranged infantry punish its cost.
The user reviewed the brief, numerical comparison and defaults, and authorized implementation.
Ship through normal Portal production, not as a hidden or debug-only unit.

## Phase 1: approved rules

- Identity: `bow_novice`, distinct authoritative kind; display name Bow Novice.
- Cost: 50 steel, zero oil; two supply; Portal production; 300 ticks (10 seconds).
- No research or additional building prerequisite. Portal card slot 1 (W); Sword Disciple
  retains slot 0 (Q), researched Adepts retain slots 3–5.
- 48 HP (Sword Disciple 58); unarmored ordinary ground infantry; no special immunity.
- Speed 2 world pixels/tick, sight 11 tiles, collision radius 17 world pixels.
  Selection uses the same base cultivator affordance; artwork retains its original scale.
- Primary: five-tile range, five damage, 32-tick cooldown, zero armor penetration,
  ordinary Rifleman target legality and stationary firing, with the existing five-tick
  Rifleman post-shot movement pause. Immediate server
  damage with the preserved arrow presentation/release timing. No area damage.
- No Charge, Blink, secondary melee attack, or passive insight. Normal commands and queues.
- Same insight rate as Sword Disciple: one per hostile unit HP removed, quarter-rate building
  HP with fractional carry. At 60 insight, evolve to chosen Wind/War/Magic Adept; absent
  intent, use the existing random choice. Existing healing, body-growth, cooldown/order
  preservation, Master progression, Cosmic Energy and shop rules apply.
- Both base kinds may enter existing tournament/training conversion paths; their output
  remains the existing Adept. Existing resource, repair, harvesting and economy roles unchanged.
- Existing AI may encounter, target and assess it as infantry; no new AI build strategy.
- Reuse checked-in art and arrow texture; do not generate new art or use rifle muzzle flashes.
  Dedicated bow audio is deferred; do not play a sword or gun sound for arrows.

## Delivery checklist

- [x] Phase 0 brief written and reviewed in conversation.
- [x] Phase 1 rules written; user authorized implementation.
- [x] Rules, protocol mirrors, production and progression integrated.
- [x] Client card, artwork, projectile and selection integrated.
- [x] Focused production/combat/evolution and parity validation.
- [x] Interact combat WebM captured and visually inspected.
- [ ] Patch-note copy staged; owned PR merged.

Patch-note draft: Portals can immediately train Bow Novices for 50 steel. Bow Novices
stop to fire arrows, earn insight and evolve into any of the three existing Dao Adepts.
The former Novice is now named Sword Disciple.

Validation: rules/protocol/simulation nextest coverage, client contracts, protocol and faction
parity, wiki generation, client/simulation architecture checks and `tests/run-all.sh --no-rust`.
Interact verified an unresearched Portal produces a 48-HP Bow Novice in 300 ticks.
Combat demo: three Bow Novices versus three Riflemen; 360 fixed-step frames, 30 FPS,
12-second WebM. Dedicated bow audio remains deferred.
