# AI Late Game Convergence - Multi-Phase Plan

This plan makes live AI profiles keep distinct openings while converging on one shared tank-based
late game. The goal is not to erase profile identity. The goal is to stop tuning three different
late games while tanks are the only credible endgame unit.

Today the live lobby pool contains:

- `tech_to_tanks`
- `rifle_flood_fast`
- `rifle_flood_full_saturation`

`steel_expansion_tanks` exists in the shared profile list and test/self-play tooling, but it is not
currently selected by ordinary live lobby AI opponents.

## Current Problem

The current profile data already has several tank transitions, but they are not one shared late
game:

- `tech_to_tanks` attacks with tank+rifle groups from its main plan.
- `rifle_flood_full_saturation` pivots at 50 supply to tank+rifle production and tank-required
  waves.
- `rifle_flood_fast` pivots later, at 70 supply, to tank+rifle production and tank-required waves.
- `steel_expansion_tanks` pivots at 50 supply into tank-only production and tank-only attacks.

That means late game tuning is duplicated and inconsistent. Changes to tank wave size, production
mix, cadence, or expansion expectations have to be remembered in several places.

## Desired Shape

Each profile should have:

1. A distinct opening.
2. Optional profile-specific recovery or transition behavior.
3. A shared late-game destination.

For now, the shared destination is:

- tank tech is required;
- tank+rifle production is the default late-game army;
- outbound attacks require at least one tank;
- wave size, wave growth, staging distance, regroup behavior, and reissue cadence are common;
- every live profile has an eventual expansion path.

## Explicit Non-Goals

- Do not implement tank-front/rifle-back formation behavior in this plan.
- Do not switch a whole AI profile to `tech_to_tanks` mid-game.
- Do not solve `rifle_flood_fast` economic recovery here. The proxy opening can remain fragile
  until a separate recovery plan gives it a better bridge into the shared late game.
- Do not add profile selection UI.
- Do not change wire protocol or client rendering.

## Phases

- [Phase 1 - Name the shared late game](PHASE_1.md)
- [Phase 2 - Make live profiles converge](PHASE_2.md)
- [Phase 3 - Unify expansion expectations](PHASE_3.md)
- [Phase 4 - Regression coverage and documentation audit](PHASE_4.md)

## Guiding Invariants

1. **Openings stay distinct.** A proxy rush should still proxy rush. A macro rifle profile should
   still saturate first. A tank rush should still tech faster.
2. **Late-game tuning lives in one place.** If tank-wave size changes, it should not require editing
   every live profile by hand.
3. **Profile switching is avoided.** Shared policies are composed into profiles; the controller
   does not replace one profile id with another during a match.
4. **AI remains fair.** The AI still emits ordinary `SimCommand`s and uses public enemy start tiles
   for outbound attacks unless defending against visible local threats.
5. **Panic defense remains shared.** Local-defense panic mode should continue to override normal
   production and attack choices when visible threats are close to the AI base or workers.
6. **Design docs are updated only for real behavior changes.** This implementation plan is not a
   contract change by itself. When code behavior changes, update `DESIGN.md` in the same change.

## Suggested Implementation Order

Implement phases in order. Phase 1 should be mostly refactor and naming. Phase 2 should change
late-game attack and production convergence. Phase 3 should make expansion expectations consistent.
Phase 4 should lock the behavior down with tests and documentation updates.

If `rifle_flood_fast` still fails to reach late game, treat that as expected until its separate
recovery plan exists. The shared late-game policy should be a destination, not a rescue mechanism
for an all-in opening.
