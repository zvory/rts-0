# AI Terrain Routing Plan

## Purpose

Give AI harassment and later movement managers enough terrain awareness to choose usable flanks
instead of geometrically plausible but tactically bad choke routes. The motivating bug is the
Default map right-side spawn pairing: top-right and bottom-right AIs both route Scout Car harassment
through the same one-tile right-edge choke, then repeatedly evade visible enemies and reissue the
same failed route. This plan builds a small AI-only terrain routing layer from public start-payload
map data, keeps final command emission through the existing action layer, and uses focused tests and
diagnostics before widening the behavior surface.

## Phase Summaries

Phase 1 carries static terrain into AI observation and adds small passability helpers without
changing decisions. It keeps the data fog-safe because terrain is already public in the start
payload, and it documents the new AI map contract in the AI design doc. The outcome is that AI code
can ask basic questions about map tiles without reaching into simulation internals.

Phase 2 builds an AI-only route candidate evaluator and test harness. It generates candidate
harassment corridors, scores them against static terrain, lane overlap, path length, and choke
narrowness, and proves the right-side Default map pairing no longer prefers the one-tile edge choke
in isolation. The outcome is route intelligence that can be tested without changing live AI command
behavior yet.

Phase 3 integrates the terrain route evaluator into Scout Car harassment. It replaces the single
geometric flank waypoint with a selected corridor and a short queued waypoint route while preserving
`AiActionContext` and ordinary `SimCommand::Move` emission. The outcome is that Scout Cars choose
actually usable flank corridors for the known bad right-side spawn case and still fall back safely
when no route candidate is available.

Phase 4 adds harassment route memory and visible-threat influence. It detects repeated evasion or
low-progress loops, temporarily marks the current corridor hot, and folds visible enemy units into
route scoring so Scout Cars switch routes instead of reissuing a failing choke approach. The outcome
is a harassment manager that can recover from both predictable static chokepoints and newly occupied
flanks.

Phase 5 hardens diagnostics, self-play validation, and documentation for future AI movement uses. It
adds compact trace output and focused self-play or scenario checks for routing regressions, then
updates design docs to define what this routing layer is and is not responsible for. The outcome is
a stable base that later army movement, scouting, and expansion logic can reuse without turning this
work into a full tactical planner immediately.

## Phase Index

1. [Phase 1 - Terrain Observation](phase-1.md)
2. [Phase 2 - Static Route Evaluator](phase-2.md)
3. [Phase 3 - Scout Car Harassment Integration](phase-3.md)
4. [Phase 4 - Route Memory and Threat Influence](phase-4.md)
5. [Phase 5 - Diagnostics and Validation](phase-5.md)

## Overall Constraints

- Keep the AI fog-safe. Static terrain, public starts, and public resource positions may be used;
  hidden enemy unit or building positions must not be inferred from private simulation state.
- Do not change the wire protocol unless implementation proves a new field is required. The current
  start payload already exposes terrain to every client, so the first implementation should route
  that existing data into `AiObservation`.
- Keep final command emission centralized in `AiActionContext` and `ai_core::actions`. Routing code
  should choose goals, waypoints, blockers, scores, and intents; it should not create a parallel
  command pipeline.
- Keep `Game` AI-free. `rts-ai` may consume public `StartPayload` and snapshot surfaces, but it must
  not import private simulation internals or bypass ordinary command validation.
- Prefer focused route tests over long self-play loops during development. Add deterministic unit
  tests around route scoring and the known right-side spawn failure before relying on end-to-end
  match results.
- Preserve existing behavior as a fallback. If terrain data is missing, no route is found, or a
  route candidate is invalid, Scout Car harassment should either use the previous safe fallback or
  skip issuing commands for that think instead of panicking or spamming bad orders.
- Keep the first routing layer deliberately small. This plan is not a full HTN/GOAP rewrite, not a
  global omniscient influence map, and not a new movement engine; it is a static terrain route
  scorer plus live route selection memory for AI intent.
- Update `docs/design/ai.md` whenever AI observation, route planning responsibility, trace output,
  or harassment behavior semantics change. Refresh `docs/context/server-sim.md` only if section
  pointers or code maps shift.
- Coordinate write ownership. Phases 1 through 4 all touch `server/crates/ai/src/ai_core`; implement
  them sequentially from this plan instead of in parallel worktrees.
- Balance/gameplay patch notes should describe player-facing harassment behavior changes: Scout
  Cars should avoid obvious contested chokepoints, use alternate flank routes, and stop looping on a
  repeatedly occupied corridor.

## Implementation and Handoff Rules

Implement one phase at a time. Each phase should be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit.

After each phase, the implementing agent must provide a handoff message describing what the next
agent should do and what should be manually tested. Manual testing notes should cover the core
features for that phase, not an exhaustive test matrix.
