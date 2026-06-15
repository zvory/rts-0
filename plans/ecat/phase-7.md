# Phase 7 - Ekat Out-and-Back Line Projectile

Status: Not Started.

## Goal

Implement Ekat's line projectile using the moving projectile runtime. The ability should damage on
the outbound and return passes and preserve metadata for later damage scaling.

## Scope

- Add or update the Ekat line projectile ability catalog entry.
- Replace the current immediate Ekat line-damage path for this ability, or isolate it behind tests
  if temporary compatibility is needed during the phase.
- On activation:
  - validate caster, faction, range, cooldown, and target point
  - clamp endpoint according to ability range
  - spawn one outbound projectile from Ekat's current position to the endpoint
  - configure the projectile to return to the origin after reaching the endpoint
  - start cooldown at the contractually correct time
- On projectile tick:
  - damage valid enemies intersecting the swept line width
  - dedupe hits according to the phase's explicit rule
  - record leg, age, and travel distance for damage formula hooks
  - expire cleanly after the return leg completes
- Add client preview and placeholder visual feedback for outbound and return path shape.
- Update docs and factual patch notes.

## Expected Deliverables

- Ekat's line projectile visibly travels out and back.
- Damage happens on both legs and is server-authoritative.
- Damage scaling hooks are present even if the first fun-test value is simple.
- The old immediate line-shot behavior no longer defines the product path.
- Ekat can move or dash after firing th eline projective, which will meant he line projectile's return path can curve, but it always returns directly to Ekat.

## Out of Scope

- Anchor dual-origin launch.
- Anchor placement or destruction.
- Final damage tuning.
- Fancy projectile art or sound.

## Verification

- Add focused Rust tests for endpoint clamping, outbound hit, return hit, no duplicate same-leg hit
  if that is the chosen rule, enemy-only filtering, stale caster, and cooldown behavior.
- Add client preview tests for line path descriptors where practical.
- Run targeted protocol/client tests if new events or object fields are added.

## Manual Testing Focus

Start an Ekat match, fire line projectile at moving or stationary enemies, and confirm visible
outbound and return behavior. Confirm dash positioning can change where a later Q originates from,
even before anchor interaction exists.

## Handoff Expectations

The handoff must state damage and hit-dedupe rules, projectile speed/duration, cooldown timing,
tests added, and the spawn helper Phase 9 should reuse for anchor-origin projectiles.
