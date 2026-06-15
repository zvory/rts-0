# Phase 9 - Anchor and Projectile Composition

Status: Not Started.

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
visual/fog behavior, tests added, and any damage-tuning concerns left for a future balance pass.
