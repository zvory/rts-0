# Tank Coaxial Machine Gun Requirements

Status: Draft product requirements. This document describes the desired player-facing tank coaxial
machine gun behavior and the important contract constraints discovered during scoping. It is not an
implementation plan and does not authorize code, protocol, balance, art, or test changes by itself.

## Purpose

Tanks should gain a coaxial machine gun tied to the turret direction. The coax should make tanks
more credible against enemies directly in front of the turret, especially infantry, without making
the tank hull or turret automatically swing toward every nearby soft target.

The feature should preserve current tank identity: the main cannon remains the tank's primary
anti-armor weapon, while the coax is a secondary opportunistic small-arms weapon.

## Combat Behavior

- Every Tank has a coaxial machine gun in addition to its existing main cannon.
- The coax has 6-tile range.
- The coax deals 4 damage per shot, matching the Machine Gunner attack damage.
- The coax uses the Machine Gunner attack cadence unless a later balance pass explicitly changes
  it. The current expected cooldown is 6 ticks.
- Coax damage is small-arms damage. It must not use Tank anti-tank/AP damage classification just
  because the firing entity is a Tank.
- Coax damage attribution still belongs to the firing Tank's owner for damage alerts, scoring,
  reveal, and AI damage-memory purposes.
- The coax has its own cooldown independent of the main cannon cooldown.
- Coax shots should overpenetrate using the same direct-fire overpenetration system as current
  direct shots, but with coax small-arms damage and coax weapon identity.
- Main cannon firing, cooldown, target selection, turret rotation, stationary range ramp, and
  overpenetration behavior should continue to work as they do today unless an implementation issue
  requires an explicitly reviewed change.

## Arc And Firing Conditions

- The coax fires when a legal enemy target is within 10 degrees on either side of the current tank
  turret direction.
- The 10-degree check is against the Tank's current authoritative turret/weapon facing, not the
  hull facing.
- The coax must not rotate the turret by itself. It only uses the current turret direction created
  by normal tank behavior, direct attack orders, cannon aiming, movement, or idle turret relaxation.
- The coax may fire while the cannon is rotating, ready, reloading, or otherwise unavailable, as
  long as a legal target is inside the current coax arc.
- The coax may fire while the Tank is moving if the current tank state otherwise permits the Tank
  to expose and attack targets.
- The coax should use the same hostile, visibility, smoke, line-of-sight, targetability, and friendly
  hard-blocker safety checks as direct-fire combat.
- Resource nodes are not valid coax targets.

## Target Priority

- The coax prioritizes infantry targets inside its current arc.
- For this implementation, infantry-priority targets are Workers, Riflemen, Machine Gunners, and
  future Panzerfaust-style infantry when that unit exists.
- Mortar Teams, Artillery, Anti-Tank Guns, Ekat, Golems, vehicles, buildings, resources, and field
  obstacles are not infantry-priority targets for the coax.
- If at least one infantry-priority target is legal and inside the current arc, the coax should pick
  from that group before considering fallback targets.
- If no infantry-priority target is legal and inside the current arc, the coax should fire at legal
  fallback enemy targets inside the arc.
- Fallback targets can include vehicles and buildings when they are otherwise legal direct-fire
  targets.
- Tank Trap behavior should follow existing combat legality. Do not add special coax-only Tank Trap
  targeting rules unless a later requirement calls for them.
- Ties within the same material priority should be deterministic, using existing style: distance
  first, then id if needed.

## Audio And Visual Feedback

- Coax shots use the machine gun combat sound, not the tank cannon sound.
- A Tank should render a tiny gray rectangular coax barrel beside the main gun barrel.
- Coax muzzle flash and tracer feedback should originate from the smaller coax barrel, not from the
  main cannon muzzle.
- Coax shots should use machine-gun-scale visual feedback. They must not trigger the large tank
  cannon muzzle flash or tank cannon recoil.
- The main cannon should continue to use the existing tank cannon sound, muzzle flash, tracer, and
  recoil behavior.

## Protocol And Projection Requirements

- If attack events remain the source of combat sound and visual feedback, the wire event must carry
  enough weapon identity for clients to distinguish a Tank cannon shot from a Tank coax shot.
- Any event weapon hint must be fog-safe. It may identify the shot class needed for rendering/audio,
  but must not reveal hidden target data beyond the existing attack-event visibility rules.
- Compact snapshot/event decoding, JSON protocol docs, Rust contract structs, and JavaScript protocol
  mirrors must stay in sync if the event shape changes.
- Coax attack events should obey current attack-event projection rules: visible to the attacker's
  team and to enemy players who can see the target/impact side according to existing combat event
  visibility.

## UI And Data Surface

- The Tank command card, cost, supply, sight, and trainability do not change.
- The Tank's displayed primary weapon range should remain the current main-cannon range behavior
  unless a later UI requirement adds a separate coax range display.
- The generated stats/wiki surface should mention the coax as a secondary weapon if implementation
  exposes secondary weapons in text or generated stats.
- No new player command is required. The coax is passive/opportunistic.

## Testing Expectations

Focused verification for an implementation should cover:

- A Tank coax shot damages an in-arc infantry target for 4 small-arms damage.
- The coax does not use Tank AP damage against armored fallback targets.
- The coax prioritizes in-arc infantry over in-arc fallback targets.
- Ekat is not treated as infantry priority.
- The coax fires at fallback targets when no infantry-priority target is in arc.
- The coax does not fire at targets outside the 10-degree arc or outside 6-tile range.
- The coax cooldown is independent from the main cannon cooldown.
- Coax overpenetration applies with small-arms damage and does not trigger cannon-scale feedback.
- Coax attack events produce machine-gun audio and small-barrel visual feedback.
- Existing tank cannon targeting, cooldown, turret alignment, stationary range ramp, and cannon
  audio/visual behavior continue to pass focused regression tests.

## Non-Goals

- Do not add a player command, toggle, upgrade, research, or ability for the coax in this pass.
- Do not change Tank cost, supply, HP, movement speed, sight, train requirements, or main-cannon
  stats.
- Do not make the coax rotate the turret, chase targets, or replace explicit player attack intent.
- Do not make the coax a suppression, morale, or area-denial system.
- Do not treat this requirements document as implementation approval for code, protocol, balance,
  art, or test changes.
