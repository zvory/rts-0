# Artillery Queuing Plan

## Purpose

Rework queued artillery ordering so players can plan `move -> setup -> pointFire` sequences with
clear client feedback and server-authoritative execution. The feature should let a Shift-queued
setup freeze the planned field-of-fire cones at the accepted setup location, then allow queued
Point Fire to target against that planned emplacement instead of the artillery's current packed
position. The server remains the source of truth for order acceptance, queue promotion, deployment,
range, arc, ammo cost, and final firing behavior.

## Phase Summaries

### [Phase 1 - Authoritative Queue Semantics](phase-1.md)

Teach the simulation to accept a queued artillery Point Fire only when it is already legal today or
when the same artillery has a queued setup stage immediately before that future shot. Preserve the
existing rule that Point Fire is terminal in the unit queue, and make promotion wait through the
setup/deployment transition instead of popping the Point Fire as stale while the gun is still
setting up. This phase should land focused server tests for packed artillery no-ops, queued
`setup -> pointFire`, `move -> setup -> pointFire`, terminal behavior, stop/clear behavior, and
final range/ammo validation.

### [Phase 2 - Frozen Client Planning UX](phase-2.md)

Add a client-owned frozen setup planning preview created by Shift-clicking Set Up. After the queued
setup command is issued, clear the armed setup target even while Shift remains held, keep the
accepted setup cones frozen at their projected origins, and let Point Fire targeting use those
frozen origins for range and field-of-fire feedback. This phase should cover world and minimap
targeting paths, selection/command reset rules, and focused client contract tests for the planned
cone lifetime.

### [Phase 3 - Integration, Documentation, And Playtest Hardening](phase-3.md)

Align the end-to-end workflow so the server queue, owner-only `orderPlan`, command-card affordance,
frozen cone rendering, and Point Fire preview all agree. Update the server-sim, client UI, and
protocol design docs to describe the new conditional queued Point Fire contract without changing
the wire shape. This phase should add regression coverage for mixed selections and stale queued
states, run the smallest relevant cross-area checks, and produce patch-note bullets that describe
the gameplay impact.

## Overall Constraints

- Keep artillery firing and queue execution server-authoritative. The client may preview planned
  setup cones and target validity, but the server must recheck final position, deployment state,
  range, field-of-fire, ownership, tech/faction eligibility, ammo affordability, and liveness.
- Do not add a new wire message or snapshot field unless implementation evidence proves the
  existing `setupAntiTankGuns`, `useAbility(pointFire)`, and owner-only `orderPlan` data are
  insufficient.
- Preserve current immediate behavior: unqueued Point Fire from packed artillery remains a no-op and
  must not auto-setup or fire.
- Preserve current terminal behavior: once a queued Point Fire is accepted for a unit, later queued
  unit orders for that unit must not append behind it.
- Keep queued setup semantics general for Anti-Tank Guns and Artillery, but only Artillery gains the
  special queued Point Fire follow-up.
- Treat client frozen cones as local planning feedback. Clear them on stale selection, stop/hold or
  unqueued replacement commands for the affected artillery, explicit cancel, match teardown, and
  any other state change where keeping the cone would mislead the player.
- Support mixed selections. Non-artillery units should ignore setup and Point Fire stages as they do
  today, while later compatible orders still apply to compatible selected units unless Point Fire is
  the accepted terminal stage for an artillery unit.
- Keep `Game::tick()` panic-free. Stale ids, dead units, cleared queues, invalid setup points,
  out-of-range final targets, and unaffordable ammo must be safe no-ops or existing notices.
- Update the relevant design docs whenever a phase changes current command, queue, preview, or
  protocol contract wording.
- Collect factual gameplay patch-note bullets as phases land: queued setup can now stage artillery
  Point Fire, setup targeting de-arms after Shift-click, frozen cones show the planned
  field-of-fire, and final firing still depends on deployment/range/ammo at execution time.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite merge with the phase head reachable from `origin/main`
  before the next phase starts.
- After implementing each phase, the implementing agent must provide a handoff message describing
  what changed, what the next agent should do, and what should be manually tested. Manual testing
  notes should name core gameplay scenarios, not an exhaustive test matrix.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Required Verification Themes

Each phase should run the smallest relevant subset of:

- focused Rust tests for command admission, order queue promotion, artillery setup, and Point Fire
  execution
- focused client contract tests for command-card affordances, input targeting, minimap targeting,
  and renderer feedback view-model behavior
- `node scripts/check-client-architecture.mjs` for client module or wiring changes
- `node tests/protocol_parity.mjs` if protocol vocabulary, order-plan projection, compact
  metadata, or protocol docs change
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` for
  cross-service or sim architecture changes
- `node scripts/check-docs-health.mjs` for docs-only or plan/doc phases
- `git diff --check`

## Suggested Execution

Implement one phase at a time from a clean worktree. Do not start a later phase from an assumed
merge; wait for the owned PR to merge and verify reachability from `origin/main`.

```bash
scripts/phase-runner.sh --plan artilleryqueuing 1 --pr --wait
scripts/phase-runner.sh --plan artilleryqueuing 2 --pr --wait
scripts/phase-runner.sh --plan artilleryqueuing 3 --pr --wait
```
