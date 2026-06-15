# AI Resource Availability Plan

## Purpose

Make AI resource use depend on an explicit resource-availability model instead of raw resource
node lists. The model should distinguish known resources from resources that are currently
mineable by a completed City Centre, free for assignment, occupied, saturated, or reserved for
future expansion. This should fix the AI 1.0 idle-worker class where workers can be commanded to
non-mineable oil while free mineable steel exists, and it should give later AI versions a single
place to reason about main-base, expansion, and panic-economy resource choices.

## Phase Summaries

Phase 1 introduces an AI-owned resource availability model without changing command emission. It
derives per-node availability from the fog-respecting `AiObservation`, completed own City Centres,
resource remaining values, current worker latches, and existing reservation data. The outcome is a
tested facts layer that can answer which steel or oil nodes are mineable now, which are merely known
for expansion planning, and which workers are already occupying each node.

Phase 2 moves economy planning onto the new availability model while preserving the current AI 1.0
profile targets where resources are actually available. Oil demand should be suppressed when there
is no free mineable oil, steel saturation should count free mineable steel rather than all nearby
known steel, and expansion planning should continue to use known non-main resources as future
candidate sites. The outcome is that economy intent no longer asks idle workers to satisfy an
impossible oil target before available steel work.

Phase 3 routes worker assignment through availability-aware selection and adds defensive command
backstops. `assign_workers_to_resource` should receive candidate nodes from the availability model
or enforce the same availability predicate before emitting `Gather`, and failed oil assignment
should not reserve workers away from later steel assignment. The outcome is a command-emission layer
that cannot knowingly issue gather commands to non-mineable resource nodes even if an upstream
policy miscalculates.

Phase 4 adds focused regression scenarios, trace/readout coverage, and documentation for the new
resource contract. It should include a deterministic pre-expansion case with free steel and
non-mineable oil, a post-expansion case where oil becomes mineable after the City Centre completes,
and a short profile-backed replay/checkup that proves the AI still reaches its tech economy. The
outcome is documented, replay-inspectable evidence that the architecture fixes the idle-worker
class without regressing normal AI 1.0/AI 1.1 economy progression.

## Phase Index

1. [Phase 1 - Resource Availability Facts](phase-1.md)
2. [Phase 2 - Availability-Driven Economy Intent](phase-2.md)
3. [Phase 3 - Availability-Safe Assignment](phase-3.md)
4. [Phase 4 - Regression Scenarios and Documentation](phase-4.md)

## Overall Constraints

- Do not give AI private simulation access. Resource availability must be derived from the same
  fog-filtered snapshot/start-payload inputs already used by `AiObservation`, plus public rules and
  AI-owned pending-build/reservation state.
- Keep final command emission centralized through `AiActionContext` and `ai_core::actions`.
  Behavior changes should flow through shared AI facts/plans/actions, not profile-specific
  special cases.
- Preserve the sim's authoritative gather validation in `services/commands.rs`. AI availability is
  an intent and command-quality model; it is not a replacement for `gather_node_valid`.
- Keep expansion planning able to see known resource clusters that are not currently mineable.
  "Known for expansion" and "mineable now" must be separate concepts.
- Treat starting-base steel and oil exactly like any other node: they are mineable only if a
  completed own City Centre is in mining range and the node has remaining resources.
- Do not silently assume start-payload resources with no visible delta are depleted. Preserve the
  current known-position behavior unless a phase explicitly adds a better observed-remaining model.
- Avoid broad gameplay tuning in this rollout. Worker targets, oil thresholds, expansion timing,
  panic behavior, and AI 1.1 profile differences should change only where availability makes an
  existing target impossible.
- Add deterministic focused tests before or with behavior changes. Use long `RTS_FULL_AI_TESTS=1`
  or release `ai-matchup` runs only in the validation phase or when a behavior phase directly
  touches profile-backed long-match strategy.
- Update `docs/design/ai.md` in the same phase that establishes the new resource contract, because
  AI economy behavior and trace/readout semantics are design-visible.
- Collect factual gameplay patch notes during implementation. The player-facing impact should be
  described as AI workers preferring currently mineable resources and no longer idling on impossible
  oil assignments, not as a broad economy buff unless replay evidence shows one.

## Known Evidence To Preserve

- The reproduced bug class is pre-expansion, around the low-to-mid 20s supply range, with idle
  workers near the City Centre, free mineable steel nodes, and no free mineable oil nodes.
- In that reproduced class, the last worker gather command targeted oil that had resources
  remaining but was outside completed-City-Centre mining range.
- `assign_workers_to_resource` currently chooses by kind, remaining, reservations, and distance,
  while the sim's gather validation requires a completed mining City Centre in range.
- AI economy assignment currently attempts oil before steel, so an impossible oil target can consume
  the worker opportunity that should have gone to available steel.

## Implementation and Handoff Rules

Implement one phase at a time. Each phase should be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit.

After each phase, the implementing agent must provide a handoff message describing what the next
agent should do and what should be manually tested. Manual testing notes should cover the core
features for that phase, not an exhaustive test matrix.
