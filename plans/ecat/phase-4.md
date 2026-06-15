# Phase 4 - Recast and Per-Caster State Contract

Status: Not Started.

## Goal

Define and implement reusable recast semantics and per-caster owner-only ability state. This phase
creates the command and projection contract that dash-return will use in Phase 5.

## Scope

- Decide the recast command shape using Phase 0's decision. Acceptable options include a distinct
  ability id for the return, a structured `useAbility` mode, or another explicit protocol shape; do
  not infer recast from missing `x`/`y` fields.
- Add server validation for recast commands:
  - caster exists, is owned, alive, and eligible
  - the matching active ability state exists
  - the minimum no-instant-return delay has elapsed
  - the target destination is still valid for the effect
  - cooldown, cost, and lockout rules are not bypassed
- Add per-caster ability state helpers for:
  - active return marker id
  - return availability tick
  - active anchor id
  - anchor lockout expiry tick
  - owner-only remaining lifetime or availability projection
- Project owner-only affordance state through existing `EntityView.abilities` if that is sufficient,
  or through the Phase 2 ability-object owner-only fields if the phase decides that is cleaner.
- Mirror any protocol changes in the client and update docs.
- Add client command-card behavior for recast affordances without requiring final UX polish.

## Expected Deliverables

- Recast commands are explicit and mirrored.
- Server recast validation is deterministic and panic-free.
- Owner-only ability affordance state is visible enough for the client command card to show whether
  return or anchor placement is available.
- Existing one-shot abilities continue to work.

## Out of Scope

- Implementing dash movement or return movement.
- Implementing anchors or projectiles.
- Full hotkey profile redesign.
- Client prediction.

## Verification

- Run focused Rust command/protocol tests for valid recast, missing state, stale caster, too-early
  recast, and hidden enemy non-projection.
- Run JS protocol/client command-card descriptor tests if client command-card data changes.
- Run protocol parity checks if command or snapshot shapes change.

## Manual Testing Focus

Use a temporary fixture or debug command if needed to confirm recast affordances appear only for the
owning player and that invalid recast attempts do not crash or desync the match.

## Handoff Expectations

The handoff must document the recast wire shape, owner-only state projection, validation rules, and
the exact affordance Phase 5 should use for dash return.
