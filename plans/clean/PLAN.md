# Sim Internal Architecture Cleanup Plan

## Purpose

Make `rts-sim::game` trend toward lower coupling during ordinary feature work. The goal is not to
pause every few days for a large cleanup pass. The goal is amortized architecture: every agent task
must either keep the internal boundaries steady or make them slightly better.

The first enforcement target is structural, not stylistic. Agents should be nudged away from adding
more broad mutable access to `Entity`, raw `PlayerState`, and all-purpose services such as
`commands.rs`. Comments and docs are useful context, but Rust-owned checks should carry the
enforcement.

## Direction

- Build a Rust architecture checker instead of a Node script for sim internals.
- Start with current-state baselines so the first checks prevent regressions rather than requiring
  an immediate rewrite.
- Prefer ratchets over hard ideals. A large file budget is useful as a token and review-cost signal,
  but it should fail on growth past the baseline, not demand arbitrary tiny files.
- Bless the existing good pattern: pure planning modules like `order_planner` take facts and return
  decisions without importing mutable world state.
- Gradually move invariants behind narrower APIs on `EntityStore`, entity transition helpers, and a
  player economy/bookkeeping facade.

## Phase Index

1. [Phase 1 - Rust Architecture Checker](phase-1-rust-architecture-checker.md)
2. [Phase 2 - Baseline and Ratchet Budgets](phase-2-baseline-and-ratchets.md)
3. [Phase 3 - Service Boundary Rules](phase-3-service-boundary-rules.md)
4. [Phase 4 - Entity and Player State APIs](phase-4-entity-and-player-state-apis.md)
5. [Phase 5 - CI, Hooks, and Agent Workflow](phase-5-ci-hooks-agent-workflow.md)

## Non-Goals

- Do not rewrite the simulation into ECS as part of this effort.
- Do not block all work on existing large files.
- Do not use line count as a proxy for quality by itself. Use it as one budget among import fan-in,
  mutable world access, and number of public/internal exports.
- Do not weaken the public `Game` API seam while cleaning internals.
- Do not make gameplay/balance changes unless a phase explicitly needs a tiny behavior-preserving
  refactor.
