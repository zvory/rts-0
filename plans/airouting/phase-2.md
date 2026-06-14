# Phase 2 - Static Route Evaluator

Status: Planned.

## Objective

Build a deterministic AI-only route candidate evaluator that can score harassment corridors before
any live command behavior changes. It should prove, with focused tests, that the known Default map
top-right versus bottom-right spawn case does not prefer the one-tile right-edge choke when better
static terrain options exist.

## Scope

- Add a small routing module under `server/crates/ai/src/ai_core/decision/` or
  `server/crates/ai/src/ai_core/` for route candidates, scores, waypoints, and route reasons.
- Generate a bounded set of harassment corridor candidates from public map data. Initial candidates
  can be simple and deterministic: direct lane, left/right or top/bottom outer lanes, natural-side
  lanes, and map-edge approaches derived from own start, enemy start, and enemy steel-line target.
- Implement static route scoring with penalties for unreachable paths, excessive path length,
  narrow choke width or low local passability clearance, overlap with the direct army lane, and
  early proximity to enemy start or natural areas.
- Use the terrain helpers from Phase 1 and avoid importing private simulation pathing internals.
- Include compact score details for tests and later traces, such as selected corridor id, total
  score, path length, worst choke width, and main-lane overlap.
- Add a deterministic test fixture using the Default map right-side spawn pairing. The test should
  assert that the route evaluator recognizes the far-right midpoint/edge passage as highly choked
  and selects an alternate candidate or reports that no acceptable flank exists.
- Keep live AI behavior unchanged in this phase. Existing harassment commands should still come from
  the old geometric planner until Phase 3.

## Expected Touch Points

- New `server/crates/ai/src/ai_core/decision/routing.rs` or similar
- `server/crates/ai/src/ai_core/decision/mod.rs`
- `server/crates/ai/src/ai_core/decision/harassment.rs` only for shared target helpers if needed,
  not behavior integration
- `server/crates/ai/src/ai_core/decision/tests.rs`
- `server/assets/maps/default-handcrafted.json` as test input only; avoid changing the map asset
- `docs/design/ai.md`

## Verification

Run focused route and decision tests:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-ai routing
cargo test --manifest-path server/Cargo.toml -p rts-ai scout_car_harassment
```

The new tests should be fast, deterministic, and not require a running server.

## Manual Testing Focus

No gameplay manual test is required because live behavior should not change. If route score output
is temporarily logged for development, remove or gate it before committing.

## Handoff Expectations

The handoff must describe the route candidate model, the scoring dimensions, and the exact assertion
used for the right-side Default map failure case. It should also tell Phase 3 how to obtain the
selected corridor waypoints and what fallback to use when no acceptable route is returned.

## Player-Facing Outcome

No intended player-facing change. This phase creates tested route intelligence but does not yet use
it to command Scout Cars.
