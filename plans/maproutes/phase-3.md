# Phase 3: Region and choke extraction

Status: Pending

## Goal

Implement deterministic tile-based region and choke extraction that is visible and reviewable in
the Phase 2 overlay.

## Scope

- Compute clearance/altitude from passable terrain for relevant ground movement classes.
- Derive open regions using deterministic tile operations, with explicit thresholds and comments for
  why those thresholds are chosen.
- Detect choke/portal segments between regions, including center, endpoints, width, adjacent
  regions, and approach tiles.
- Merge or simplify noisy regions so Default and Low Econ produce human-legible regions rather than
  tiny fragments.
- Render region fills and choke segments through the existing diagnostics overlay.
- Add focused tests for synthetic maps with open fields, one choke, two alternate chokes, and no
  terrain blockers.

## Non-goals

- Do not wire AI decisions to region/choke output.
- Do not attempt full polygonal BWTA or Voronoi geometry unless tile analysis demonstrably fails.
- Do not make dynamic buildings/tank traps mutate the static analysis in this phase.

## Expected touch points

- `server/crates/ai/src/ai_core/map_analysis*.rs`
- AI map-analysis tests
- Diagnostics payload projection added in Phase 2
- `docs/design/ai.md`

## Verification

- Run focused `rts-ai` tests for map analysis.
- Add golden-ish structural assertions that are stable but not brittle: number of meaningful
  regions/chokes within a narrow range, starts assigned to regions, chokes connecting distinct
  regions, and no panics on No Terrain.
- Run any targeted diagnostics payload tests affected by new fields.

## Manual testing focus

Inspect Default and Low Econ in AI-vs-AI spectator mode with region and choke overlays on. Confirm
the overlay finds the intended middle area, base approaches, and narrow passages without excessive
noise or misleading labels.

## Handoff

The handoff must describe the chosen thresholds, where they are likely fragile, and which observed
routes/chokes on Default should be treated as baseline expectations in Phase 4.
