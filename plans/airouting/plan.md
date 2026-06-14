# AI Routing Atlas Plan

## Purpose

Build a deterministic map-atlas foundation for AI routing so harassment and later movement
managers reason from machine-generated map topology instead of geometric guesses. The existing
Scout Car harassment behavior has already shipped, but it can choose poor routes because the AI
does not have a strategic representation of terrain, clearance, connected areas, or base-resource
approaches. This plan replaces the stale AI routing plan; it preserves the product requirement that
Scout Cars pressure resource lines through credible flanking routes without cheating, but it does
not preserve the old plan's unverified narrow-choke claim about the Default map.

The atlas should be generated for every authored map from public map data: terrain, map sites,
starts, and resource anchors. It should be legible to AI code through typed route queries, not by
asking decision managers to inspect raw terrain strings. The generated atlas should also be
legible to humans through a diagnostic, non-editable map editor tab before AI route queries start
depending on it.

## Phase Summaries

Phase 1 defines and generates the static map atlas. It computes the smallest defensible foundation:
movement-class passability, connected components, clearance, regions, portals, and semantic anchors
for authored starts, naturals, and resource clusters. The outcome is a deterministic atlas that can
be validated without changing live AI decisions.

Phase 1.5 adds a dev/editor-only static atlas inspection tab to the map editor. The server computes
the atlas through the authoritative map-loading path and exposes diagnostic atlas data for the
editor to render as legible overlays. The outcome is a designer-facing way to verify that Phase 1
generated the right world model before Phase 2 builds AI route queries on top of it.

Phase 2 exposes atlas-backed routing queries to AI code. It adds route options between semantic
anchors and arbitrary points, returns route facts such as distance, component match, portal path,
and minimum clearance, and keeps the AI on public fog-safe map information. The outcome is an AI
route API that managers can consume without importing private simulation internals or rebuilding
terrain analysis locally.

Phase 3 rewrites Scout Car harassment route selection to use the atlas query layer. It replaces the
current geometric flank waypoint with a chosen route toward the enemy resource-line approach while
preserving `AiActionContext`, ordinary `SimCommand::Move` emission, existing reservation behavior,
and safe fallback when no credible route exists. The outcome is harassment that selects a route
because the atlas proves it is reachable and vehicle-appropriate, not because a perpendicular
offset looked plausible.

Phase 4 adds route memory, visible-threat influence, and focused validation. It detects repeated
evasion or low-progress loops, temporarily cools down failing route choices, folds visible enemy
combat units into route scoring, and adds deterministic tests for route switching or no-command
fallback. The outcome is Scout Car harassment that can recover when a route becomes occupied
without treating unseen areas as known safe or known dangerous.

Phase 5 is a design-blocked agent-legibility planning gate, not an implementation phase. It records
that, beyond the approved Phase 1.5 static atlas editor tab, we do not yet know the right format,
workflow, or UX for broader agent-facing reports, exports, route-debug artifacts, or committed
visual outputs. The outcome should be a follow-up design brief only if the user chooses to pursue
additional tooling after the atlas-backed AI behavior is working.

## Phase Index

1. [Phase 1 - Map Atlas Foundation](phase-1.md)
2. [Phase 1.5 - Static Atlas Editor View](phase-1.5.md)
3. [Phase 2 - AI Route Query Layer](phase-2.md)
4. [Phase 3 - Scout Car Harassment Rewrite](phase-3.md)
5. [Phase 4 - Route Memory and Validation](phase-4.md)
6. [Phase 5 - Broader Agent Legibility Design Gate](phase-5.md)

## Overall Constraints

- Keep the AI fog-safe. Static terrain, public starts, public map sites, and public resource
  positions may be used; hidden enemy unit or building positions must not be inferred from private
  simulation state.
- Keep `Game` AI-free. `rts-ai` may consume public `StartPayload`, snapshot surfaces, and public
  route query APIs, but it must not import private simulation internals or bypass ordinary command
  validation.
- Keep final command emission centralized in `AiActionContext` and `ai_core::actions`. Atlas and
  routing code may choose route options, waypoints, blockers, scores, and intents; it must not
  create a parallel command pipeline.
- Do not change the wire protocol unless implementation proves a new field is required. The first
  atlas should be server-side and derived from map assets already loaded by the simulation.
- Preserve deterministic behavior with sorted inputs, stable ids, stable tie-breaks, and
  snapshot-testable generated data.
- Preserve existing behavior as a fallback. If atlas generation fails, a route query returns no
  acceptable route, or a route is already effectively complete, harassment should skip or use the
  existing safe fallback rather than panic or spam bad orders.
- Treat movement-class differences as a first-class requirement. Scout Cars, Tanks, infantry, and
  future units may need different passability or clearance facts even when they share the same
  terrain grid.
- Keep atlas scope narrow. The foundation is passability, connected components, clearance, regions,
  portals, and semantic anchors. Lanes and route summaries should be derived query outputs, not
  authored schema in the first pass. Dynamic influence maps should be introduced only after static
  atlas queries are stable.
- Avoid baking unverified map claims into tests or docs. If a test asserts a choke, route width, or
  alternate path, it must be derived from the atlas and checked against the authored map.
- Update `docs/design/ai.md` whenever AI observation, route planning responsibility, trace output,
  or harassment behavior semantics change. Update `docs/design/server-sim.md` only if atlas
  ownership or the `Game`/map API seam changes.
- Coordinate write ownership. These phases touch shared map, sim, and AI contracts; implement them
  sequentially from this plan instead of in parallel worktrees.
- Balance/gameplay patch notes should describe player-facing harassment behavior changes: Scout
  Cars should pressure resource lines through credible reachable routes and stop looping on a
  repeatedly occupied route.
- Phase 1.5 is the approved human-legibility tooling scope: a static, non-editable map editor tab
  that renders server-computed atlas facts for diagnosis. Generated images, committed visual
  artifacts, dynamic influence maps, route-debug exports, and broader agent reports remain out of
  scope unless a later user-approved plan adds them.

## Implementation and Handoff Rules

Implement one phase at a time. Each phase should be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit.

After each phase, the implementing agent must provide a handoff message describing what the next
agent should do and what should be manually tested. Manual testing notes should cover the core
features for that phase, not an exhaustive test matrix.
