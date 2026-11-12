import { strict as assert } from "node:assert";
import { GameState } from "../../client/src/state.js";
import { KIND } from "../../client/src/protocol.js";

// Both decoded visibility representations must repair missed resource deaths.
for (const Grid of [Array, Uint8Array]) {
  const state = new GameState({
    playerId: 1,
    map: {
      width: 4, height: 4, tileSize: 32,
      resources: [
        { id: 200, kind: KIND.STEEL, x: 65, y: 97 },
        { id: 201, kind: KIND.OIL, x: 112, y: 112 },
        { id: 202, kind: KIND.STEEL, x: 16, y: 16 },
      ],
    },
  });
  const grid = (visible) => Grid.from(Array.from({ length: 16 }, (_, i) => visible.includes(i) ? 1 : 0));
  const snapshot = (tick, visible, resourceDeltas = []) => state.applySnapshot({
    tick, entities: [], events: [], resourceDeltas,
    visibleTiles: grid(visible), exploredTiles: grid([0, 14, 15]),
  });
  snapshot(0, [14, 15], [{ id: 200, remaining: 17 }, { id: 201, remaining: 23 }]);
  assert.equal(state.entityById(200).remaining, 17);
  assert.equal(state.entityById(201).remaining, 23);

  // Enemy mining happens out of sight. Exploration and missing deltas reveal nothing.
  snapshot(1, []);
  assert.equal(state.entityById(200).remaining, 17);
  assert.equal(state.entityById(201).remaining, 23);
  snapshot(2, [14]);
  assert.equal(state.resourceById.get(200).remaining, 0);
  assert.equal(state.entityById(200), undefined);
  assert.equal(state.entityById(201).remaining, 23);
  snapshot(3, [15]);
  assert.equal(state.resourceById.get(201).remaining, 0);
  assert.equal(state.entityById(201), undefined);
  assert(!state.entitiesInterpolated(1).some((e) => e.id === 200 || e.id === 201));
  assert(state.entityById(202), "unseen resources remain known");
  snapshot(4, []);
  assert.equal(state.entityById(200), undefined, "depletion stays remembered after vision leaves");

  // Missing/no-fog grids do not establish visible absence.
  state.applySnapshot({ tick: 5, entities: [], events: [], visibleTiles: [] });
  state.applySnapshot({ tick: 6, entities: [], events: [] });
  assert(state.entityById(202));
  state.resetForReplaySeek();
  snapshot(0, [14, 15], [{ id: 200, remaining: 17 }, { id: 201, remaining: 23 }]);
  assert.equal(state.entityById(200).remaining, 17, "timeline reset restores resource baseline");
}

console.log("Resource vision contracts passed");
