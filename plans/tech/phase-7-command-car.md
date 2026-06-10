# Phase 7 - Command Car

## Objective

Add the Mobile Warfare late-game capstone after Artillery gives Superior Firepower a long-range
position-grinding payoff.

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

- Add any new protocol messages, ability identifiers, fake-unit snapshot representation, and fog
  projection rules together.
- Ensure fake units cannot attack, gather, block ownership commands incorrectly, or leak hidden real
  unit information.
- Implement Command Car after Artillery so Mobile Warfare gains a late tool against entrenched
  Superior Firepower.

## Verification

- Command Car ability cooldowns, target validation, smoke synergy, fake lifetime, fake HP, and
  zero-damage behavior are tested.
- Fog tests cover fake army visibility.
- Regression tests cover fake units disappearing cleanly without stale ids.

## Player-Facing Outcome

Mobile Warfare gains tools to force breakthroughs, exploit smoke, and deceive defensive lines after
Superior Firepower's Artillery timing comes online.
