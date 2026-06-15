# Phase 5 - Dash Return Ability

Status: Not Started.

## Goal

Implement Ekat's dash and delayed return marker using the new ability runtime, projection, and
recast contract. This is the first complete product ability in the 0.1 plan.

## Scope

- Add or revise Ekat ability catalog entries for dash and return behavior while preserving faction
  gating. It is acceptable to repurpose the existing `ekatTeleport` id if that is the lowest-friction
  migration path, but the old instant teleport behavior must not remain reachable.
- On dash activation:
  - validate target range and static standability
  - record the original position as an ability object
  - set the earliest return tick so return cannot happen instantly
  - move Ekat to the destination
  - clear or preserve orders according to the phase's explicit command contract
  - start only the cooldowns intended by the product requirement
- On return activation:
  - validate the active return marker
  - validate return timing
  - validate the return destination is still standable
  - move Ekat back to the marker
  - consume or expire the marker
- Project the return marker through fog according to Phase 2 policy.
- Render and preview the return marker using Phase 3 client surfaces.
- Add notices or lightweight feedback for invalid return attempts where useful.
- Update design docs and factual patch-note bullets for changed Ekat behavior.

## Expected Deliverables

- Ekat can dash to a valid world point and leave a projected return marker.
- Ekat cannot return instantly.
- Ekat can return while the marker is active and the destination remains valid.
- Hidden markers do not leak to enemies without vision.
- Existing Ekat faction start and non-Ekat factions remain intact.
- Existing Ekat instant teleport behavior is scrubbed from player-facing runtime, docs, and tests
  except for any explicitly justified replay/test compatibility shim.

## Out of Scope

- Out-and-back line projectile behavior.
- Magic Anchor behavior.
- Damage or invulnerability during dash.
- Fancy animation or sound.
- Balance-quality cooldown tuning beyond the product-required fun-test values.

## Verification

- Add focused Rust tests for dash success, invalid landing point, no-instant-return, valid return,
  expired marker, blocked return destination, stale caster, and fog projection.
- Add or update client command-card/preview tests for dash and return affordances.
- Run targeted protocol parity tests if ability ids or snapshot fields change.

## Manual Testing Focus

Start an Ekat match, dash to valid and invalid points, confirm the marker appears where expected,
confirm immediate return is blocked, and confirm return works after the delay. Use enemy vision or a
second client if practical to confirm the marker is visible only when the spot is visible.

## Handoff Expectations

The handoff must state the final dash and return ability ids, cooldown and return-window behavior,
what old teleport code/docs/tests were removed or quarantined, manual fog result, tests added, and
any remaining UX roughness for later cleanup.
