# AI map route analysis plan

## Summary

This plan adds human-inspectable static map understanding for AI before changing AI behavior. The
goal is a deterministic terrain analysis model that identifies regions, chokes, route alternatives,
and candidate defensive points from public map data, then renders the exact same data in spectator
diagnostics. AI attack, defense, and tank-trap decisions are only allowed to consume the analysis
after the overlay is readable enough for humans to audit.

## Phase summaries

### Phase 1: Static analysis scaffold

Create the AI-side map-analysis module, data types, cache boundary, and deterministic fixture tests
without changing decisions or protocol. The first analyzer pass should build passability,
clearance, component, and base/resource mapping data from `StartPayload.map` so later phases have a
stable foundation. It must expose a compact debug snapshot for tests, but no client-visible overlay
or gameplay behavior changes yet.

### Phase 2: Spectator diagnostics transport and overlay shell

Expose a spectator-only map-analysis diagnostics payload and render toggles in the existing AI
diagnostics experience. The client should draw server-provided analysis primitives as map overlays
with stable colors, labels, and layer toggles, even if the first primitives are basic components and
base markers. This phase proves the human review loop: an AI-vs-AI spectator can turn the overlay on
and inspect the same analysis data that future AI decisions will use.

### Phase 3: Region and choke extraction

Replace the scaffold output with a BWEM-style tile analysis that derives open regions and choke
portals from passability and clearance. The algorithm should favor deterministic, explainable tile
operations over opaque geometry, and its output should be legible in the overlay on Default,
Low Econ, and No Terrain maps. This phase still has no AI behavior changes; success means humans can
judge whether the identified regions and choke lines match the intended map shape.

### Phase 4: Route graph and tactical candidates

Build the region-choke route graph, base-to-base route queries, route labels, and candidate defense
or tank-trap sites. The diagnostics overlay should show main and alternate routes, shared chokes,
route lengths, and candidate placement markers with short reasons. This phase provides the tactical
vocabulary the AI needs, but decisions should continue to run as before except for optional trace-only
shadow evaluations.

### Phase 5: Trace-only AI route reasoning

Wire the AI decision core to compute route-aware plans in shadow mode and emit trace diagnostics
without changing emitted commands. Frontal attack, defensive staging, proxy placement, and future
tank-trap logic should report which route or choke they would prefer and why, while still issuing
legacy orders. This phase validates the policy layer against live AI-vs-AI games before it can
affect gameplay.

### Phase 6: Gated gameplay adoption

Enable route-aware behavior one decision family at a time after the diagnostics have proven readable
and stable. Start with low-risk staging and attack-move waypoints, then defense placement, then
tank-trap candidate use only if placement legality and pathing behavior are covered by tests. This
phase must collect player-facing patch notes because it changes how AI attacks, stages, defends, and
possibly blocks routes.

## Cross-phase constraints

- Keep `Game` AI-free. AI map analysis should be owned by `rts-ai` or exposed as a generic
  fog-safe sim helper, never by making `rts-sim` depend on `rts-ai`.
- The overlay must render the exact server/AI analysis data structure, not a separate client-side
  approximation.
- Use only public static map/start data and fog-filtered snapshots. Do not let AI infer hidden
  enemy entities or bypass command validation.
- No gameplay behavior changes before Phase 6. Phase 5 may compute shadow plans and traces only.
- Keep expensive analysis out of the tick path. Compute once per map/start payload or behind a
  bounded cache keyed by map binding and player/start layout.
- The route model is strategic vocabulary, not a replacement for movement. Existing sim pathing and
  command validation remain the source of truth for whether units/buildings can actually move or
  be placed.
- If a phase changes protocol/start payload shapes, update the Rust contract, server protocol,
  client protocol mirror, and `docs/design/protocol.md` together.
- If a phase changes `GameState` or `DerivedState`, update `docs/design/server-sim.md` state
  registry and checkpoint expectations in the same PR.
- Collect patch-note bullets for any phase that changes AI-visible gameplay, especially route
  selection, defensive staging, tank-trap placement, or attack timing.

## Execution requirements

Each phase must be implemented and committed on its own `zvorygin/` branch from a fresh worktree
based on `origin/main`. After implementation, push the branch, open an owned PR with auto-merge
armed, then run `scripts/wait-pr.sh <pr>` and verify the phase head is reachable from `origin/main`
before reporting the phase complete or starting the next phase. When a phase is complete, mark that
phase document as done in the implementation commit for that phase.

After implementing each phase, the implementing agent must provide a handoff message describing what
the next agent should do and what should be manually tested. Manual testing notes should name the
core feature surfaces, such as AI-vs-AI spectator overlays, route labels, trace rows, or route-aware
AI behavior, rather than an exhaustive test matrix.
