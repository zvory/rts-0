# Phase 4 - Mortar Team

## Objective

Implement the Superior Firepower path-entry unit. Mortar Teams should let SF players pressure
static defenses and punish clumped units before they reach AT Guns or Artillery.

## Unit Behavior

- Built at Steelworks.
- Shells take 2 seconds to land.
- Fires every 2 seconds.
- No impact marker is shown to enemies or the firing player.
- Deals small AOE damage.
- Semi armor piercing: armored targets only apply half of normal armored damage reduction.
- Requires 1 second of setup before firing, automatically like Machine Gunners.
- Cannot fire while moving.

## Autocast

- Mortar Teams fire automatically by default.
- Autocast predicts target position 2 seconds ahead.
- Prediction includes built-in error so shots are dangerous but not perfectly reliable.
- Multiple autocast Mortar Teams should stagger fire timing.
- Multiple autocast Mortar Teams should prefer different targets when possible.
- Player can disable autocast and manually target a point.

## Work

- Add protocol kind and mirrored client/server definitions.
- Add unit stats, training rules, costs, supply, sight, and render metadata.
- Add projectile or delayed-impact simulation behavior.
- Add AOE damage with fog-safe event projection.
- Add manual fire command or ability if the current command model does not support point-targeted
  unit fire.
- Add autocast state and UI affordance for toggling it.
- Add renderer treatment that reads clearly as a crew weapon distinct from MGs and AT Guns.
- Update `docs/design/protocol.md`, `docs/design/balance.md`, and relevant context capsules.

## Verification

- Mortar Teams train from Steelworks.
- Mortar Teams set up before firing.
- Delayed shells land after the intended delay.
- AOE damage is bounded and does not panic on stale or dead targets.
- Autocast does not synchronize all Mortar Teams into identical volleys.
- Manual fire works and respects command validation.
- Fog projection does not reveal hidden enemies or hidden impact positions incorrectly.

## Player-Facing Outcome

Superior Firepower gets an early active tool for positional grinding before AT Guns and Artillery.
Mobile Warfare can still punish poorly protected Mortars with Scout Cars.

