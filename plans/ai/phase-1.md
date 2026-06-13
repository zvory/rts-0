# Phase 1 - Scenario Harness and Product Contract

Status: not started

## Goal

Create the fast test foundation and product contract for the 1.0 AI without changing live gameplay
AI behavior. This phase should make later AI work testable from authored opening, mid-game, and
late-game positions instead of requiring long self-play setup runs.

## Scope

- Document the launch AI requirements in `docs/design/ai.md` or a linked AI design subsection.
- Add compact AI scenario fixtures/builders inside `server/crates/ai` test support.
- Support authored states for:
  - opening economy with City Centre, workers, nearby steel, and one enemy start
  - first Barracks and Rifleman wave readiness
  - early expansion decision with reachable oil/steel expansion resources
  - Scout Car tech/harassment setup
  - tank-tech setup with Factory/Steelworks/unlock prerequisites
  - blocked cases such as no idle worker, no supply, unaffordable tech, and occupied placement
- Add scorecard helpers for bounded AI comparisons:
  - army value
  - worker count
  - production building count
  - expansion count
  - tech milestones
  - attacks launched
  - damage/buildings killed when available

## Expected Touch Points

- `docs/design/ai.md`
- `server/crates/ai/src/selfplay/`
- `server/crates/ai/src/ai_core/decision/tests.rs` or a new scenario-test module
- Existing self-play artifact or milestone helpers if they can be reused cleanly

## Verification

- Focused Rust tests for each authored scenario builder.
- Focused tests that compute scorecards from scenario states.
- A short bounded self-play smoke that proves the new harness can simulate a fixed number of ticks
  and report scorecard metrics without requiring match resolution.

## Manual Testing Focus

- Run a short self-play/watch flow and confirm artifacts still open.
- Confirm the added design contract matches the intended 1.0 product behavior.

## Handoff

The handoff should name the scenario builders that now exist, the fastest command for running them,
and any scenario coverage gaps the Phase 2 agent must know about.
