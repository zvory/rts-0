# Phase 6 - Capstone Units: Artillery and Command Car

## Objective

Add late-path overpowered tools after the Scout Car, Mortar, AT Gun, and Tank timing foundation is
playable.

## Artillery

- Built at Gun Works.
- Requires a late Gun Works upgrade.
- Very slow.
- Very long range.
- Shoots at a point, not a unit.
- Must set up before firing and tear down before moving.
- First shot has low accuracy.
- Accuracy improves over time while maintaining fire.
- Intended to let Superior Firepower grind down positions and destroy resource bases if it survives
  the Mobile Warfare stage-two surge.

## Command Car

- Built at Vehicle Works.
- Requires a late Vehicle Works upgrade.
- Ability: **Breakthrough!**
- Breakthrough is an AOE speed boost.
- Breakthrough bonus is doubled for units in smoke or that recently left smoke.
- Ability: **Fake Army**
- Fake Army copies army units in AOE and places the fake force at a target location within range.
- Fake copies deal no damage.
- Fake copies have 10% of the real units' HP.
- Fake copies disappear after 20 seconds.

## Work

- Implement Artillery first if the immediate need is to test the SF payoff.
- Implement Command Car after Artillery if the immediate need is an MW tool against entrenched SF.
- Add any new protocol messages, ability identifiers, fake-unit snapshot representation, and fog
  projection rules together.
- Ensure fake units cannot attack, gather, block ownership commands incorrectly, or leak hidden real
  unit information.
- Ensure Artillery point targeting is validated and cannot panic on invalid coordinates.

## Verification

- Artillery setup, teardown, point fire, accuracy ramp, and range limits are tested.
- Command Car ability cooldowns, target validation, smoke synergy, fake lifetime, fake HP, and
  zero-damage behavior are tested.
- Fog tests cover Artillery events and fake army visibility.
- Regression tests cover fake units disappearing cleanly without stale ids.

## Player-Facing Outcome

Late game becomes deliberately explosive: Superior Firepower can start deleting economic positions
from range, while Mobile Warfare gains tools to force breakthroughs, exploit smoke, and deceive
defensive lines.

