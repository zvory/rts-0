# Moving-Fire Combat Policy Plan

## Purpose

Make "fire while moving" a shared combat capability instead of a proxy for vehicle identity,
chase behavior, or legacy Rifleman Charge state. Moving-fire units executing player-issued `Move` or
`AttackMove` orders must never repath, chase, or replace their path goal because of an auto-acquired
enemy; they should keep their commanded destination and only fire at targets they can actually shoot
from the current movement path. Meth riflemen should use that same moving-fire capability without
inheriting vehicle turret, standoff, or repathing behavior.

## Overall Constraints

- Keep this plan high level. Each phase agent should inspect the current code and implement the
  cleanest local shape rather than following a prescriptive refactor recipe.
- Preserve direct `Attack` command behavior unless a phase explicitly proves it must change.
- Preserve idle-aggressive behavior separately from player-issued `Move` and `AttackMove` orders.
- For moving-fire units on player-issued `Move` and `AttackMove`, target acquisition may affect
  firing, facing, and `target_id`, but it must not issue chase paths, standoff paths, or
  enemy-directed repaths.
- "Currently fireable" means the target is inside the unit's current weapon range and passes the
  normal legal-shot checks from that position: hostile/targetable status, visibility and smoke
  gates, terrain line of sight, and friendly or hard-blocker interception.
- Do not change what happens after an `AttackMove` destination is reached. This plan only changes
  movement-order combat while the unit is still following or resuming the player-issued path; any
  current post-arrival idle/aggressive behavior should remain as it is.
- Do not globally remove normal non-moving-fire attack-move engagement behavior. Scope the no-repath
  invariant to units that can fire while moving.
- Do not let out-of-range auto-acquisition targets suppress valid in-range fire.
- Keep fog and projection rules authoritative; do not expose target ids or weapon facing through
  hidden targets.
- Preserve old wire/replay compatibility for legacy `charge` commands even if the gameplay state is
  removed.
- Update `docs/design/balance.md`, `docs/design/server-sim.md`, and any relevant hardening or
  protocol docs when behavior or compatibility contracts change.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite PR merge with the phase head reachable from
  `origin/main` before the next phase starts.
- Each phase handoff must name the core manual scenarios that should be checked in game.

## Phase Summaries

### [Phase 1 - Separate Moving-Fire Policy](phase-1.md)

Separate "can fire while moving" from vehicle-specific behavior and chase/repath permission. Remove
the behavior where a moving-fire unit on player-issued `Move` or `AttackMove` repaths toward an
auto-acquired enemy, including moving-fire vehicle standoff/chase paths. Vehicle turret and
presentation behavior should remain vehicle-specific policy, not a side effect of the shared
moving-fire capability. Leave `AttackMove` post-arrival behavior untouched.

### [Phase 2 - Movement-Order Target Filtering](phase-2.md)

Make target acquisition and retention respect current fireability while units are continuing a
movement path. Moving-fire `Move` and `AttackMove` orders should choose only fireable targets, and
the DV vs Soupman rifleman/tank case should become a focused regression for unreachable preferred
targets blocking valid in-range fallback fire. Fireability includes weapon range plus legal-shot,
line-of-sight, smoke, visibility, and blocker checks.

### [Phase 3 - Meth Riflemen Use Shared Moving Fire](phase-3.md)

Model Methamphetamines as a permanent rifleman combat capability that uses the shared moving-fire
policy. Upgraded riflemen should move and shoot on both `Move` and `AttackMove` without gaining
vehicle turret, standoff, or chase semantics. Unupgraded riflemen should keep their normal
stop-to-fire behavior.

### [Phase 4 - Remove Legacy Charge Runtime State](phase-4.md)

Purge legacy Charge gameplay state after meth riflemen no longer depend on `charge_ticks`.
Compatibility paths may still accept old `charge` wire commands and replay entries as no-ops, but
current gameplay should not refresh a fake long-duration charge state every tick. Clean up docs,
tests, constants, and client-visible metadata so Methamphetamines is described as the permanent
upgrade it is.

## Phase Index

1. [Phase 1 - Separate Moving-Fire Policy](phase-1.md)
2. [Phase 2 - Movement-Order Target Filtering](phase-2.md)
3. [Phase 3 - Meth Riflemen Use Shared Moving Fire](phase-3.md)
4. [Phase 4 - Remove Legacy Charge Runtime State](phase-4.md)

## Non-Goals

- Do not rebalance damage, range, cooldown, cost, or unit speed unless required to preserve the
  existing Methamphetamines upgrade effect.
- Do not redesign direct attack commands.
- Do not remove protocol parsing for old `charge` command logs.
- Do not introduce client-side combat authority or prediction-only fixes.

## Required Verification Themes

Each phase should run the smallest relevant subset of:

- focused Rust combat, movement, command, and replay tests near touched code
- `node scripts/check-faction-catalog-parity.mjs` for ability/catalog/client-visible metadata changes
- `node tests/protocol_parity.mjs` for wire/protocol compatibility changes
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` for
  cross-service helper or module-boundary changes
- `git diff --check`

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait gate
and confirm the phase head is reachable from `origin/main`.

For unattended executor passes after the plan is approved, use:

```bash
scripts/phase-runner.sh --plan movingfire phase-1 --pr --wait
scripts/phase-runner.sh --plan movingfire phase-2 --pr --wait
scripts/phase-runner.sh --plan movingfire phase-3 --pr --wait
scripts/phase-runner.sh --plan movingfire phase-4 --pr --wait
```
