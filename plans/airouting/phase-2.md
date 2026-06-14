# Phase 2 - AI Route Query Layer

Status: Planned.

## Objective

Expose atlas-backed route queries to AI code without changing live decisions. The AI should be able
to ask for route options and receive route facts, but it should not inspect raw terrain strings or
recompute map topology inside decision managers.

## Scope

- Add a public, fog-safe route query surface that AI can consume from the same public start/snapshot
  inputs it already receives.
- Keep `Game` AI-free. If the query needs to live in sim/map code, expose it as a public map or
  route service API rather than an AI-specific backdoor into private simulation state.
- Support route queries from point/anchor to point/anchor for a movement class.
- Return structured route facts:
  - route id or stable debug key
  - component match or no-route reason
  - region path
  - portal path
  - approximate distance or cost
  - minimum clearance along the route
  - first waypoint candidates in world coordinates
  - blocker/rejection reason when no acceptable route exists
- Derive route families from queries rather than authoring them in map JSON. Initial route families
  can include shortest/front route, wider alternate route, outside-biased route, and resource-line
  approach route when the atlas supports those distinctions.
- Add tests using bundled maps that prove route queries are deterministic and consistent with atlas
  connectivity and clearance.
- Keep live Scout Car harassment on the existing behavior until Phase 3.
- Update `docs/design/ai.md` to describe the AI route query surface and its fog-safety boundary.

## Expected Touch Points

- Atlas module from Phase 1
- Public route query API in sim/map or an AI-owned adapter over public atlas data
- `server/crates/ai/src/ai_core/observation.rs` only if AI observations need to carry an atlas
  handle, route summary, or map id
- New or updated AI routing tests under `server/crates/ai`
- `docs/design/ai.md`
- `docs/design/server-sim.md` if a public map API changes

## Verification

Run focused map and AI route tests:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-sim map
cargo test --manifest-path server/Cargo.toml -p rts-ai routing
```

If the new query API lives entirely in `rts-sim`, the `rts-ai routing` filter may not exist yet;
run the smallest relevant AI test filter that covers the adapter or observation changes.

## Manual Testing Focus

No gameplay manual test is required because decisions should not change. If development logging is
added while building the query layer, remove it or gate it before committing.

## Handoff Expectations

The handoff must name the query API, list the route facts it returns, and describe how Phase 3
should ask for a Scout Car resource-line approach route. It must also list fallback behavior for
missing atlas data, no-route results, and routes with insufficient clearance.

## Player-Facing Outcome

No intended player-facing change. This phase gives AI managers a tested route knowledge API without
using it for live commands yet.
