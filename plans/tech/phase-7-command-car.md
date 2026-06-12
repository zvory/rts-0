# Phase 7 - Command Car

## Objective

Add the Mobile Warfare late-game capstone after Artillery gives Superior Firepower a long-range
position-grinding payoff.

## Command Car

Working checklist: [command-car-checklist.md](command-car-checklist.md).

- Built at Vehicle Works.
- Requires Tank Production and a late Vehicle Works upgrade named Command Car.
- Costs 150 steel / 75 oil.
- Has no weapon.
- Ability: **Breakthrough!**
- Breakthrough is a centered AOE speed boost.
- Breakthrough bonus is doubled for units in smoke or that recently left smoke.
- Fake Army is deferred out of this implementation pass.

## Work

- Add any new protocol messages, ability identifiers, Breakthrough status representation, and fog
  projection rules together.
- Do not implement Fake Army, fake-unit snapshot representation, fake-unit fog projection, fake
  attacks, fake HP, fake lifetime, or fake cleanup in this pass.
- Implement Command Car after Artillery so Mobile Warfare gains a late tool against entrenched
  Superior Firepower.

## Verification

- Command Car unlock, production, ability cooldown, owned-unit targeting, queued casting, moving
  casts, smoke synergy, duration, and non-stacking behavior are tested.
- Fog tests cover Breakthrough visibility without leaking effects on hidden units.
- Regression tests cover stale ids, duplicate commands, and command cleanup without implementing
  Fake Army.

## Player-Facing Outcome

Mobile Warfare gains a tool to force breakthroughs and exploit smoke after Superior Firepower's
Artillery timing comes online. Fake Army deception is intentionally deferred.
