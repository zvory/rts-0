# Phase 7 - Ekat Out-and-Back Line Projectile

Status: Not Started.

## Goal

Implement Ekat's line projectile using the moving projectile runtime. The ability should damage on
the outbound and return passes and preserve metadata for later damage scaling.

## Scope

- Add or update the Ekat line projectile ability catalog entry. It is acceptable to repurpose the
  existing `ekatLineShot` id if convenient, but the old immediate line-damage behavior must not
  remain reachable as product behavior.
- Scrub the current immediate Ekat line-damage path from runtime, docs, and tests except for any
  explicitly justified replay/test compatibility shim.
- On activation:
  - validate caster, faction, range, cooldown, and target point
  - clamp endpoint according to ability range
  - spawn one outbound projectile from Ekat's current position to the endpoint
  - configure the projectile to switch to a return leg after reaching the endpoint
  - steer the return leg toward Ekat's current position each tick, not the projectile's launch origin
  - start cooldown at the contractually correct time
- On projectile tick:
  - damage valid enemies intersecting the swept line width
  - dedupe hits according to the phase's explicit rule
  - record leg, age, and travel distance for damage formula hooks
  - expire cleanly when the return leg reaches Ekat or when the documented stale/dead caster rule
    applies
- Add client preview and placeholder visual feedback for outbound and return path shape.
- Update docs and factual patch notes.

## Expected Deliverables

- Ekat's line projectile visibly travels out and back.
- Damage happens on both legs and is server-authoritative.
- Damage scaling hooks are present even if the first fun-test value is simple.
- The old immediate line-shot behavior no longer exists as reachable product behavior.
- Ekat can move or dash after firing the line projectile; the return path may curve, but it always
  travels toward Ekat's current position.

## Out of Scope

- Anchor dual-origin launch.
- Anchor placement or destruction.
- Final damage tuning.
- Fancy projectile art or sound.

## Verification

- Add focused Rust tests for endpoint clamping, outbound hit, return hit, curved return after Ekat
  moves or dashes, no duplicate same-leg hit if that is the chosen rule, enemy-only filtering, stale
  caster, and cooldown behavior.
- Add client preview tests for line path descriptors where practical.
- Run targeted protocol/client tests if new events or object fields are added.

## Manual Testing Focus

Start an Ekat match, fire line projectile at moving or stationary enemies, and confirm visible
outbound and return behavior. Move or dash Ekat after firing and confirm the return path bends back
toward her current position.

## Handoff Expectations

The handoff must state damage and hit-dedupe rules, projectile speed/duration, return-to-Ekat and
stale/dead caster behavior, cooldown timing, tests added, what old line-damage code/docs/tests were
removed or quarantined, and the spawn helper Phase 9 should reuse for anchor-origin projectiles.
