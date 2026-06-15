# Phase 8 - Magic Anchor Lifecycle

Status: Not Started.

## Goal

Implement Magic Anchor placement as a persistent, destructible ability world object. This phase
does not yet make the anchor launch extra projectiles.

## Scope

- Add the Magic Anchor ability catalog entry for Ekat.
- On placement:
  - validate caster, faction, range, cooldown, lockout state, and map point
  - create one active anchor object linked to Ekat
  - replace or reject an existing active anchor according to the phase's explicit rule
  - set a 10-second natural expiry
- Define and implement anchor targetability:
  - how enemies can attack or damage the anchor
  - whether it has hp, radius, armor, or a simple damage threshold
  - whether it blocks pathing or collision; default should be no
  - whether it can be selected or inspected; default should be non-selectable or read-only
- On natural expiry:
  - remove the anchor without applying the destroyed lockout
- On destruction:
  - remove the anchor
  - apply a 60-second anchor placement lockout to Ekat
  - emit fog-safe destruction feedback
- Project anchor lifetime and owner-only lockout state.
- Render simple anchor visuals and command-card availability.
- Update docs and factual patch notes.

## Expected Deliverables

- Ekat can place a visible 10-second Magic Anchor.
- Enemies can destroy the anchor through the contract chosen in this phase.
- Destroyed anchors lock placement for 60 seconds; naturally expired anchors do not.
- Anchor state is server-authoritative and fog-filtered.

## Out of Scope

- Launching line projectiles from the anchor.
- Anchor movement, teleporting to anchor, or anchor-triggered buffs.
- Pathing blockers or collision unless the phase explicitly decides targetability requires them.
- Balance polish.

## Verification

- Add focused Rust tests for placement validation, single-active-anchor rule, natural expiry,
  destruction lockout, no lockout on natural expiry, fog projection, and stale caster cleanup.
- Add client command-card/render tests for anchor availability and lockout where practical.
- Run protocol parity tests if owner-only lockout fields are added or changed.

## Manual Testing Focus

Start an Ekat match, place an anchor, watch it expire after 10 seconds, and confirm the ability is
available again. Destroy the anchor with an enemy and confirm placement is locked out for 60 seconds
while hidden anchors do not reveal their position to players without vision.

## Handoff Expectations

The handoff must document anchor hp/targetability, replacement rules, lockout behavior, projection
fields, tests added, and the active-anchor query Phase 9 should use.
