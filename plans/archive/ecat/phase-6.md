# Phase 6 - Moving Projectile Runtime

Status: Done.

## Goal

Add a generic moving projectile runtime for ability-owned hit volumes. This runtime should support
Ekat's out-and-back line projectile without being hard-coded to Ekat.

## Scope

- Add typed projectile instance data under the ability runtime or a closely related sim module:
  - owner
  - source caster
  - source object id when launched from an anchor
  - ability kind
  - origin
  - endpoint
  - return target, either a fixed world point or a live caster/entity id depending on the ability
  - current position or progress
  - leg, such as outbound or return
  - speed or duration
  - width/radius
  - created tick and age
  - hit set per leg or per projectile
- Add a tick system that advances projectiles at a deterministic point in `systems.rs`.
- Add swept segment hit detection so fast projectiles do not skip targets between ticks.
- Use `TeamRelations` for valid enemy filtering and avoid raw `owner != player` checks.
- Define friendly-fire behavior explicitly for ability projectiles. For this plan, Ekat's line
  projectile should damage enemies only unless Phase 0 revised the product contract.
- Emit fog-safe launch, travel, hit, or impact visuals through the projection/event strategy chosen
  earlier.
- Add tests for travel, endpoint turnaround, fixed-point return completion, live-caster return
  steering, hit dedupe, stale source caster, destroyed source object, and fog-safe visuals.

## Expected Deliverables

- A reusable projectile runtime that advances independently of command acceptance.
- Projectile damage is applied during tick progression, not immediately in `useAbility`.
- Outbound and return legs can carry different damage formulas later.
- The runtime can support projectiles whose return leg steers toward a live caster each tick without
  hard-coding that behavior to Ekat inside command acceptance.
- Existing mortar and artillery shell systems are not regressed or folded into the new runtime
  unless the phase explicitly justifies a tiny shared helper.

## Out of Scope

- Implementing Ekat's public line-shot ability button.
- Magic Anchor interactions.
- Projectile art polish.
- Reworking mortar or artillery into the new projectile runtime.

## Verification

- Run focused Rust tests for projectile advancement and collision.
- Run any fog/event projection tests added for projectile visuals.
- Run the sim architecture check if a new module or service edge is introduced.

## Manual Testing Focus

If a debug fixture can spawn a projectile, observe that it moves, turns around, expires, and does
not visibly leak through fog. No normal player-facing ability needs to be complete in this phase.

## Handoff Expectations

The handoff must describe projectile fields, tick order placement, return-target policy, stale/dead
caster behavior, damage/filtering policy, visual projection strategy, and the exact helper Phase 7
should call to spawn Ekat's line projectile.
