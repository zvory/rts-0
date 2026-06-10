# Phase 4 - Mortar Team

## Objective

Implement the Superior Firepower path-entry unit. Mortar Teams should let SF players pressure
static defenses, deter attacks and harassment, and punish clumped units before they reach AT Guns or Artillery.

## Unit Behavior

- Built at Gun Works.
- Shells take 1.5 seconds to land.
- Fires every 2 seconds.
- No impact marker is shown to enemies, but they are shown to the firing player.
- Deals 30 AOE damage in a 1.5 tile radius. Within a .5 tile radius, does 60 damage, and deals semi-armor piercing damage (applies only half of normal damage reduction)
- Requires 1 second of setup before firing, automatically like Machine Gunners.
- Cannot fire while moving.
- It should look like a heavy mortar and two guys beside it.

## Autocast

- Mortar Teams fire automatically by default.
- Fire ability is on X and has a swirl overlay when set to autocast. Right clicking wil disable autocast and allow players to fire manually with a range preview.
- Autocast predicts target position 1.5 seconds ahead.
  - If the taret is predicted to exit the mortar's range in 1.5, uhh, idk what to do. Should the mortar hold fire, or predict the unit will curve inwards into the radius?
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

- Mortar Teams train from Gun Works.
- Mortar Teams set up before firing.
- Delayed shells land after the intended delay.
- AOE damage is bounded and does not panic on stale or dead targets.
- Autocast does not synchronize all Mortar Teams into identical volleys.
- Manual fire works and respects command validation.
- Fog projection does not reveal hidden enemies or hidden impact positions incorrectly.

## Player-Facing Outcome

Superior Firepower gets an early active tool for positional grinding before AT Guns and Artillery.
Mobile Warfare can still punish poorly protected Mortars with Scout Cars.

