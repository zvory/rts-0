# Phase 02: Entity Interpolation Cleanup

Purpose: remove one obviously wasteful client-side allocation path without treating client
rendering as the root cause of the stutter.

This phase is optional. It is kept because repeated `entitiesInterpolated()` calls allocate and
clone more than necessary, and that is easy to clean up. Do not broaden this phase into HUD,
minimap, fog, or renderer throttling work.

## Current Waste

`GameState.entitiesInterpolated(alpha)` returns a new array and shallow-clones each entity. During a
single frame, current code can call it multiple times:

- `Match.ownEntities()` calls `state.entitiesInterpolated(1)`;
- `renderer.render()` calls `state.entitiesInterpolated(alpha)`;
- `minimap.render()` calls `state.entitiesInterpolated(1)`;
- HUD tech checks call `state.entitiesInterpolated(1)` when checking owned buildings.

This is not the suspected root cause. It is just pointless repeated work.

## Target Behavior

- Compute the interpolated render list once per frame when needed.
- Reuse that list for consumers that need interpolated positions.
- Use current snapshot entities directly for checks that only need owner/kind/build state.
- Avoid shallow-cloning entities when the caller does not mutate them.

## Suggested Implementation

1. Add `GameState.currentEntities()`:

   ```js
   currentEntities() {
     return (this._cur && this._cur.entities) || [];
   }
   ```

2. In `Match.frame`, compute the render list once:

   ```js
   const renderEntities = this.state.entitiesInterpolated(alpha);
   ```

3. Extend `Renderer.render(...)` to accept `renderEntities` instead of asking `GameState` to build
   them internally.

4. Extend `Minimap.render(...)` to accept current or interpolated entities if it still needs them.
   If this requires unrelated rendering changes, skip minimap in the first pass.

5. Change HUD tech checks to use `currentEntities()` because they only need current owner/kind and
   `buildProgress`.

6. Consider changing `entitiesInterpolated(alpha)` so entities without a prior sample return the
   original object when safe, instead of always shallow-cloning. Only do this if callers do not
   mutate returned entities.

## Non-Goals

- Do not add unrelated UI/rendering throttles or caches.
- Do not rewrite renderer structure.
- Do not claim this fixes the observed stutter.

## Tests

Run the normal suite:

```bash
tests/run-all.sh
```

Client smoke is relevant if `Renderer.render`, `Minimap.render`, or HUD helpers change. Verify:

- entity positions still interpolate;
- selection and command targeting still line up with rendered positions;
- HUD tech/build/train checks still work;
- minimap still shows entities if its render signature changed.

## Done Criteria

- `entitiesInterpolated()` is not called multiple times for the same frame.
- HUD tech checks no longer call `entitiesInterpolated(1)`.
- No unrelated UI/rendering throttling is included.
- Client smoke still passes.
