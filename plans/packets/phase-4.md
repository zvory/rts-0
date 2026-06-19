# Phase 4 - Resource And Fog Delta Prototype

## Phase Status

- [ ] Tentative. Needs Phase 3 design approval before implementation.

## Objective

Prototype deltas for recurring non-entity snapshot sections that may be simpler than full entity
deltas. This placeholder is not runner-ready.

## Tentative Scope

- Resource remaining values:
  - send resource updates only when a visible resource node first becomes visible, changes, or needs a
    keyframe refresh;
  - preserve correctness when latest-only pending snapshots are replaced;
  - keep depleted/removed resource behavior clear for the client.
- Visible tiles:
  - evaluate current run-length encoded full fog payloads against a delta or tile-set update format;
  - ensure the client can reconstruct the full `visibleTiles` array needed by rendering/state code;
  - handle full-vision, spectator, replay, and lab modes explicitly.
- Measurement:
  - compare p95 payload bytes and over-budget rate against Phase 1/2 baselines;
  - measure client reconstruction cost separately from existing apply cost if needed.

## Required Follow-up Before Execution

Do not implement this phase until Phase 3 defines the baseline/keyframe model and client recovery
rules. The rewritten phase should include exact wire shapes, default/fallback behavior, tests, and
manual test focus.
