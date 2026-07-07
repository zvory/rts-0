# Phase 4: Route graph and tactical candidates

Status: Pending

## Goal

Build useful strategic route queries and tactical candidate points on top of the region/choke graph,
still without changing AI behavior.

## Scope

- Build a route graph alternating regions and chokes.
- Add base-to-base route queries, including shortest route and alternate routes penalized for shared
  chokes or regions.
- Label route candidates in human terms such as `main`, `left_flank`, `right_flank`, or stable
  deterministic equivalents when orientation is ambiguous.
- Compute route metrics: length, choke widths, shared choke count, approach angle, and staging
  region near the target.
- Compute candidate defense and tank-trap points near chokes/routes, with reason strings and
  placement prefilter metadata.
- Render route lines, route labels, shared chokes, and candidate markers in the diagnostics overlay.

## Non-goals

- Do not emit route-aware AI commands.
- Do not rely on candidate points as final placement legality; sim validation remains authoritative.
- Do not include dynamic threat/influence scoring beyond placeholders or trace-only fields.

## Expected touch points

- AI map-analysis route graph module
- Diagnostics payload and client overlay modules
- `server/crates/ai/src/ai_core/decision/trace.rs` only if adding trace-only shadow fields
- `docs/design/ai.md`

## Verification

- Run focused map-analysis route tests on synthetic multi-route maps.
- Assert Default exposes multiple distinct base-to-middle or base-to-base routes where the terrain
  supports them.
- Verify candidate points are in-bounds, on passable/buildable terrain where applicable, and tied to
  a route/choke id.

## Manual testing focus

Inspect AI-vs-AI spectator overlays for Default. Confirm route labels are understandable, alternate
routes are visually distinct, and defense/tank-trap markers appear near sensible route-control
positions rather than scattered arbitrary points.

## Handoff

The handoff must name which route and candidate queries are stable enough for Phase 5 shadow
planning, and which should remain diagnostic-only.
