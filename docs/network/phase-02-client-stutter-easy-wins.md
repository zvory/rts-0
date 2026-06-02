# Phase 02: Client Stutter Easy Wins

Purpose: reduce repeated small client-side freezes before doing a transport rewrite.

The symptom this phase targets is: the authoritative game keeps progressing, commands are not the
main complaint, but the browser freezes briefly and then resumes on a newer game state. That points
at browser main-thread stalls, stale snapshot delivery, or both. Phase 01 handles stale WebSocket
snapshot delivery; this phase handles easy client frame-budget wins.

## Why This Is Early

These changes are smaller than WebTransport and are useful even if WebTransport is later added.
They also match the current code shape:

- `Match.frame` calls `fog.update`, `renderer.render`, `hud.update`, and `minimap.render` every
  animation frame.
- `Match.ownEntities()` calls `state.entitiesInterpolated(1)` every frame.
- `renderer.render()` calls `state.entitiesInterpolated(alpha)` every frame.
- `minimap.render()` calls `state.entitiesInterpolated(1)` every frame.
- HUD helper methods call `state.entitiesInterpolated(1)` when checking owned tech.
- `GameState.entitiesInterpolated()` allocates a new array and shallow-clones every entity it
  returns.
- `Minimap.render()` redraws terrain, entities, fog, resources, and viewport every frame.
- The Pixi renderer redraws the fog overlay from the whole fog grid every frame.

That is a lot of allocation and tile work at 60 fps, independent of network transport.

## Easy Win 1: Stop Recomputing Interpolated Entities Many Times Per Frame

Current behavior can allocate several full entity arrays per frame:

- one for fog ownership via `ownEntities()`;
- one for Pixi rendering;
- one for minimap rendering;
- more from HUD helper methods when selected command cards need tech checks.

Target behavior:

- compute the interpolated render list once in `Match.frame`;
- pass that list to renderer/minimap/HUD helpers, or cache it inside `GameState` for the current
  frame/alpha;
- use current snapshot entities directly for checks that only need owner/kind/state;
- avoid shallow-cloning every entity unless the caller mutates it.

Pragmatic first implementation:

1. Add `GameState.currentEntities()` that returns the current snapshot entity array without cloning.
2. Add `GameState.ownCurrentEntities(playerId)` or compute own entities from current entities for
   fog. Fog does not need interpolated positions every rAF; current snapshot positions are enough.
3. In `Match.frame`, compute `const renderEntities = this.state.entitiesInterpolated(alpha)` once.
4. Extend `Renderer.render(...)` and `Minimap.render(...)` to accept that list.
5. Update HUD tech checks to use `currentEntities()` instead of `entitiesInterpolated(1)`.

This should reduce per-frame allocations and GC pressure.

## Easy Win 2: Throttle HUD Updates

HUD updates every animation frame, but most HUD state changes only when one of these changes:

- resources/supply from a new snapshot;
- current selection;
- command target mode;
- placement mode;
- selected entity hp/production state.

Target behavior:

- render resources on snapshot only;
- render selection panel only when selection or selected entity data changes;
- render command card only when its signature changes;
- do not call the full HUD update unconditionally at 60 fps.

The HUD already has `_cardSig` to avoid command-card rebuilds. Extend that idea:

- keep a resource signature such as `steel|oil|supplyUsed|supplyCap`;
- keep a selected-panel signature with selected ids and relevant hp/progress values;
- make `HUD.update()` return quickly when signatures are unchanged;
- or call `HUD.update()` only from `onSnapshot`, selection mutations, command target changes, and
  placement changes.

First pass: signature guard the selected panel and resource bar. That is safer than moving all HUD
call sites.

## Easy Win 3: Throttle And Cache The Minimap

`Minimap.render()` currently redraws everything every frame. On the 96x96 map, terrain alone loops
over 9216 tiles per rAF, and fog loops over the same grid. That is unnecessary work.

Target behavior:

- cache minimap terrain to an offscreen canvas once per map;
- redraw dynamic minimap layers at 10-15 Hz instead of every rAF;
- draw the viewport rectangle every rAF only if camera movement needs immediate feedback.

Pragmatic first implementation:

1. In `Minimap`, build an offscreen terrain canvas when the map transform changes.
2. In `render()`, blit the cached terrain instead of looping all tiles.
3. Add a `lastDynamicRenderMs` and skip entity/fog/resource redraws until the interval elapses,
   unless a snapshot just arrived.
4. Optionally split `renderViewport()` so camera motion stays smooth.

This is likely one of the best easy wins because minimap work is pure UI and not gameplay-critical.

## Easy Win 4: Update Fog Less Often

The client fog overlay is cosmetic. The authoritative fog filtering already happens on the server.
Currently `fog.update(this.ownEntities(), tileSize)` runs every animation frame.

Target behavior:

- recompute fog when a snapshot arrives, not every rAF;
- optionally recompute during camera-independent local interpolation only if visual smoothness
  noticeably suffers;
- keep `revealAll` behavior for dev self-play.

First pass:

- set a dirty flag in `onSnapshot`;
- update fog once in the next frame using current own entities;
- skip fog recomputation on frames without a new snapshot.

This changes fog visual freshness from 60 Hz to snapshot rate, currently 30 Hz. It should be hard to
notice and saves repeated grid clearing/stamping.

## Easy Win 5: Redraw The Pixi Fog Overlay Only When Fog Changes

`Renderer._drawFog(fog)` clears and redraws the whole map fog overlay every frame. The fog grid
usually changes at snapshot cadence, not rAF cadence.

Target behavior:

- track a `fog.version` incremented by `Fog.update`;
- have `Renderer` remember the last rendered fog version;
- only rebuild `_fogGfx` when the version changes;
- camera movement should not require fog geometry rebuild because the world container transform
  moves the already-built overlay.

This is separate from Easy Win 4. If both are done, fog geometry rebuilds drop from 60/s to at most
snapshot rate, and often less when no own vision changed.

## Easy Win 6: Watch For DOM Churn In Selection UI

HUD selection rendering still uses `innerHTML = ""` and rebuilds nodes. That is fine when selection
changes, but costly if done every frame with stable selection.

Target behavior:

- selected panel rebuilds only when selected ids or displayed selected data changes;
- command card rebuilds only when `_cardSig` changes;
- resource text only writes when values change.

Do not over-engineer this. Simple signatures are enough.

## What Not To Do In This Phase

- Do not change server simulation.
- Do not change the wire protocol.
- Do not add WebTransport.
- Do not introduce a framework.
- Do not rewrite the renderer wholesale.
- Do not remove interpolation unless measurement proves it is the problem.

## Tests

Run the normal test suite:

```bash
tests/run-all.sh
```

If the client smoke test is available, it is important here because these changes touch rendering
and UI refresh cadence.

Add focused tests only where feasible:

- unit-level test for stale snapshot rejection if added;
- browser smoke/assertion that HUD resources update after snapshots;
- browser smoke/assertion that minimap still draws terrain/entities/fog;
- manual or automated trace showing fewer long tasks and lower per-frame work.

## Done Criteria

- Repeated per-frame entity clone/allocation paths are reduced.
- HUD avoids rebuilding unchanged DOM at 60 fps.
- Minimap no longer redraws static terrain every frame.
- Fog recomputation and Pixi fog redraws happen only when needed.
- Client smoke still passes.
- A Phase 00 trace shows improved frame-time p90/p99 or fewer long tasks.
