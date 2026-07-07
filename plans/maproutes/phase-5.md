# Phase 5: Trace-only AI route reasoning

Status: Pending

## Goal

Let AI decision code evaluate map routes in shadow mode and explain its choices, while preserving
legacy command behavior.

## Scope

- Add route-aware plan fields to frontal attack, defensive staging, proxy placement, and tank-trap
  planning traces where those planners already exist.
- Score route choices with static metrics first: distance, choke width, shared chokes, approach
  angle, and staging quality.
- Add optional dynamic inputs only when they are already available from fog-safe observations, such
  as visible enemy threat near a route.
- Emit diagnostics that explain selected route id, rejected route ids, blockers, and key scores.
- Ensure emitted `SimCommand`s remain identical to pre-phase behavior unless an existing trace-only
  command ordering artifact makes that impossible; if so, treat it as a blocker.

## Non-goals

- Do not change actual attack, movement, defense, proxy, or tank-trap commands.
- Do not add hidden-information influence maps.
- Do not add new unit micro or split-force behavior yet.

## Expected touch points

- `server/crates/ai/src/ai_core/decision/frontal.rs`
- `server/crates/ai/src/ai_core/decision/defense.rs`
- `server/crates/ai/src/ai_core/decision/proxy.rs`
- `server/crates/ai/src/ai_core/decision/trace.rs`
- AI diagnostics payload/panel if additional trace rows need display
- AI decision tests

## Verification

- Run focused AI decision tests proving command output remains unchanged while trace output includes
  route decisions.
- Run a targeted self-play or AI matchup smoke if trace wiring touches live controller behavior.
- Add regression tests for route scoring tie-breaks so traces are deterministic.

## Manual testing focus

Watch AI-vs-AI spectator games with route overlays and AI diagnostics open. Confirm trace-selected
routes correspond to visible overlay routes and that legacy AI behavior has not visibly changed.

## Handoff

The handoff must include examples of trace lines from live or test output and recommend the first
decision family that is safe to enable in Phase 6.
