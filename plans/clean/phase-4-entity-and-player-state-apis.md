# Phase 4 - Entity and Player State APIs

## Objective

Reduce reliance on broad raw mutable state. The target is not to hide every field immediately, but
to move important invariants behind small APIs that are hard to misuse.

## Work

- Introduce narrow APIs for player bookkeeping, starting with economy and scoring:
  - spend resources
  - refund resources
  - add gathered resources
  - reserve and release supply
  - record created/lost/killed entity score
- Migrate services away from direct `PlayerState` field mutation where a helper exists.
- Add entity transition helpers for high-risk state changes:
  - clearing/replacing active orders
  - entering and leaving construction
  - starting and consuming ability cooldowns/uses
  - applying damage and recording last-damage attribution
  - production queue push/pop/progress
- Update the checker so new direct writes to migrated fields fail outside approved modules.
- Keep read access pragmatic at first. Writes are the more dangerous coupling surface.

## Migration Strategy

- Start with fields that already have obvious invariants or duplicated logic.
- Move one field family at a time.
- After each migration, lower the baseline so future agents cannot add the old pattern back.
- Keep behavior-preserving tests close to the service being migrated.

## Verification

- Existing sim tests continue to pass after each migration.
- Checker fixtures prove a migrated field cannot be written directly from a random service.
- At least one real service migration demonstrates the intended pattern before broad enforcement.

## Outcome

The shared `Entity` and `PlayerState` records remain practical, but important lifecycle and economy
rules become API-enforced instead of convention-enforced.
