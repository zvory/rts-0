# Build Wait - Multi-Phase Plan

This plan changes worker construction so an arrived worker can hold a build order while waiting for
resources or short-lived unit blockers, instead of immediately going idle. The desired player
experience is that a valid build command remains intentional work until it can start, unless the
site becomes permanently invalid or a unit blocks the footprint for too long. The plan treats a
three-second unit-block grace as 90 simulation ticks at 30 Hz.

## Product Contract

- A worker with a build order should walk to the selected building footprint as it does today.
- When the worker is in build-arrival range and the footprint is otherwise legal but the player
  lacks resources, the worker should keep the active build order, stand near the site, and retry
  until resources become available.
- Build resources are still paid only when the scaffold is spawned. Waiting at the site must not
  reserve resources.
- Existing scaffold resume behavior must stay free. A worker can resume an owned matching scaffold
  at the same footprint without paying the original cost again, even if the player is currently
  broke.
- If a waiting build footprint becomes blocked by another building or scaffold, the worker should
  cancel the active build order and go idle.
- If a waiting build footprint is blocked only by a relevant unit body, the worker should keep the
  active build order for up to three seconds.
- If the relevant unit blocker clears before the three-second timeout, the worker should resume the
  normal placement/resource retry loop and eventually start construction when both site and
  resources are available.
- If the footprint is still unit-blocked at the three-second timeout, the worker should lose the
  active build order and go idle. Once the order has been dropped, later blocker movement should
  not implicitly restart it.
- Terrain, out-of-bounds footprints, resource nodes, unknown building kinds, missing tech
  requirements, or missing builder eligibility remain immediate rejection/cancel cases.
- Tank Trap placement keeps its current special blocking policy: infantry-like units do not block
  Tank Trap placement, while vehicle-body blockers count as unit blockers for this plan.
- Failure notices should be useful but not spam every tick while a worker waits. Prefer one notice
  when entering a waiting/failure state, then silence until the state changes or the order ends.

## Open Interpretation

The original request said the worker should go idle if the footprint is still unit-blocked after
three seconds, then said it should resume if the blockage clears after three seconds. This plan
resolves that as: clear before the three-second timeout resumes; still blocked at the timeout drops
the order. If product intent is instead an unlimited unit-block wait with a warning after three
seconds, Phase 1 should update the contract before implementation begins.

## Phase Summaries

### [Phase 1 - Placement Classification And Build-Wait State](phase-1.md)

Introduce the simulation vocabulary needed to distinguish why a build footprint is blocked. This
phase should add a classified placement probe, focused tests for building/unit/node/terrain
blockers, and build-order execution state capable of remembering waiting-at-site and
unit-blocked-tick progress. It should avoid changing player-visible build behavior except where
small helper refactors are necessary to preserve existing semantics.

### [Phase 2 - Waiting Construction Behavior](phase-2.md)

Wire the new state into build command application, queued build promotion, and the construction
system. Immediate and queued build orders should be able to move to the site without current
affordability, then wait at arrival until resources are available, while building blockers cancel
and unit blockers use the three-second grace timer. This phase should replace the current tests
that expect unaffordable queued builds or unit-blocked final placement to be skipped immediately.

### [Phase 3 - Integration Hardening And Documentation](phase-3.md)

Harden the changed behavior across tick ordering, notices, queued handoffs, AI/self-play, and
design documentation. This phase should add or tune integration-style simulation tests for
resources becoming available, blockers clearing, blockers timing out, and overlapping building
races. It should update `docs/design/server-sim.md` and the server-sim capsule if needed so the
new queued-order and build-arrival semantics are documented as the current contract.

## Phase Index

1. [Phase 1 - Placement Classification And Build-Wait State](phase-1.md)
2. [Phase 2 - Waiting Construction Behavior](phase-2.md)
3. [Phase 3 - Integration Hardening And Documentation](phase-3.md)

## Overall Constraints

- Keep this server-authoritative in `rts-sim`. The client may show existing order/build markers,
  but it must not decide whether a waiting build remains valid.
- Do not add protocol fields unless implementation evidence proves the client needs a new explicit
  state. Existing order markers and construction progress should be enough for the first pass.
- Preserve fog safety. Build failure notices and build events must remain owner/team/fog-gated as
  they are today.
- Preserve `Game::tick()` panic-freedom. Do not add `unwrap`, unchecked indexing, or arithmetic
  derived from client coordinates on the tick path.
- Keep build placement math checked. Debug overflow on bad build coordinates must remain impossible.
- Keep existing scaffold resume behavior and construction progress behavior intact.
- Keep worker queued orders intact where current active-order failure/completion semantics preserve
  them. If a timed-out build drops only the active order, later queued orders should still be
  available for promotion unless the phase explicitly updates that policy and tests it.
- Avoid broad AI rewrites. AI may benefit from wait-for-resource behavior, but this plan should not
  change AI strategy unless tests expose a direct regression.
- Do not broaden local verification by default. Use focused Rust tests for the touched sim
  services and let the PR `./tests/run-all.sh` gate provide full coverage.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite merge with the phase head reachable from `origin/main`
  before the next phase starts.
- After implementing each phase, the implementing agent must provide a handoff message describing
  what changed, what the next agent should do, and what should be manually tested. Manual testing
  notes should cover core behavior, not an exhaustive test matrix.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Suggested Execution

Implement one phase at a time from a clean worktree. Do not start a later phase from an assumed
merge; wait for the owned PR to merge and verify reachability from `origin/main`.

```bash
scripts/phase-runner.sh --plan buildwait 1 --pr --wait
scripts/phase-runner.sh --plan buildwait 2 --pr --wait
scripts/phase-runner.sh --plan buildwait 3 --pr --wait
```
