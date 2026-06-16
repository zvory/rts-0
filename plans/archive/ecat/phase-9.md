# Phase 9 - Anchor and Projectile Composition

Status: Done.

## Implementation Notes

- Ekat Line Shot now queries the active Magic Anchor at launch time and builds projectile specs from
  both Ekat and the anchor when the anchor is active.
- Anchor-origin shots use the same cursor point but clamp their endpoint from the anchor origin with
  the normal Line Shot range. Both hero-origin and anchor-origin projectiles return toward Ekat's
  current entity position.
- The command spends cost and starts cooldown once. An active anchor does not double-spend cost or
  double-start cooldown.
- Hit dedupe remains per projectile and per leg: a target can be hit by both the hero-origin and
  anchor-origin projectiles, and each projectile keeps separate outbound and return hit sets.
- Projectile visuals remain active ability objects and use the existing fog-filtered projection:
  enemies only receive a projectile object when its current position is visible.
- The existing client preview path already draws owned Magic Anchors as extra Line Shot origins.

## Patch Notes

- Ekat's Line Shot now fires a second projectile from her active Magic Anchor.
- Anchor-enhanced Line Shots use the same target cursor, but each origin has its own range clamp.
- Destroyed or expired anchors fall back to the normal single-projectile Line Shot.

## Goal

Compose Magic Anchor with Ekat's line projectile so one command can launch projectiles from both
Ekat and her active anchor. This phase proves independent ability objects and projectiles can
interact through typed server queries rather than bespoke command branches.

## Scope

- Add a server helper to query Ekat's active anchor at line-projectile launch time.
- On line projectile activation:
  - always launch from Ekat if the caster activation is valid
  - also launch from the active anchor when one exists and is still valid
  - use the same cursor target point for both origins
  - apply range, endpoint, or clamping rules explicitly for the anchor-origin projectile
  - return both hero-origin and anchor-origin projectiles toward Ekat's current position using the
    Phase 7 return-target behavior
  - avoid double-spending cost or double-starting cooldowns unless the product contract says so
- Define hit dedupe across multiple simultaneous origins:
  - whether the same target can be hit by both hero-origin and anchor-origin projectiles
  - whether outbound and return legs are deduped separately
  - how damage scaling uses each projectile's own travel metadata
- Update client targeting preview to draw both hero-origin and anchor-origin paths when the anchor
  is projected to the owner.
- Add fog-safe visual behavior for an anchor-origin projectile when enemies can see the anchor,
  the projectile, or the hit position.
- Update docs and factual patch notes.

## Expected Deliverables

- With an active anchor, Ekat's line projectile launches from both Ekat and the anchor.
- Without an active anchor, the line projectile behaves exactly as Phase 7 defined.
- Both projectiles return toward Ekat's current position, not their launch origins.
- Client preview accurately shows the extra origin for the owning player.
- Hidden anchors or hidden projectile origins do not leak through enemy snapshots or events.

## Out of Scope

- New anchor abilities beyond projectile composition.
- Balance tuning for double-origin damage.
- AI use of anchor combos.
- Final visual/audio polish.

## Verification

- Add focused Rust tests for no-anchor single launch, active-anchor dual launch, expired-anchor
  single launch, destroyed-anchor single launch plus lockout, cross-origin hit dedupe, cooldown
  behavior, and fog projection.
- Add client preview tests for dual-origin path descriptors.
- Run targeted protocol/client tests if new visual events are added.

## Manual Testing Focus

Start an Ekat match, place an anchor, fire the line projectile, and confirm two projectiles launch
toward the cursor. Repeat after the anchor expires and after it is destroyed to confirm the ability
falls back to the single-origin behavior.

## Handoff Expectations

The handoff must state the cross-origin dedupe rule, range/clamping rule for anchor-origin shots,
confirmation that both return legs target current Ekat, visual/fog behavior, tests added, and any
damage-tuning concerns left for a future balance pass.
