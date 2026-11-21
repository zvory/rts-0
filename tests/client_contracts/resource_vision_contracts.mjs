import { strict as assert } from "node:assert";
import { GameState } from "../../client/src/state.js";
import { KIND } from "../../client/src/protocol.js";

for (const Grid of [Array, Uint8Array]) {
  const state = new GameState({ playerId: 1, map: { width: 2, height: 1, tileSize: 32,
    resources: [{ id: 200, kind: KIND.STEEL, x: 16, y: 16 }, { id: 201, kind: KIND.OIL, x: 48, y: 16 }],
  }});
  const apply = (tick, resourceDeltas, resourceReset = false, visible = [0, 0], events = []) => state.applySnapshot({
    tick, entities: [], events, resourceReset, resourceDeltas, visibleTiles: Grid.from(visible), exploredTiles: Grid.from([1,1]),
  });
  assert.equal(state.resourceById.get(200).remaining, 0, "start map is positions, not historical knowledge");
  apply(1, [{ id: 200, remaining: 17 }, { id: 201, remaining: 23 }], true);
  apply(2, [], false, [1, 1]);
  assert.equal(state.entityById(200).remaining, 17, "omitted delta never implies depletion, even on visible tiles");
  apply(3, [{ id: 200, remaining: 0 }]);
  assert.equal(state.entityById(200), undefined);
  assert.equal(state.entityById(201).remaining, 23);
  state.resetForReplaySeek();
  apply(1, [{ id: 200, remaining: 17 }, { id: 201, remaining: 23 }], true);
  assert.equal(state.entityById(200).remaining, 17, "backward seek restores earlier authoritative knowledge");
  apply(4, [{ id: 200, remaining: 0 }, { id: 201, remaining: 0 }], true);
  state.resetForReplaySeek();
  apply(4, [{ id: 200, remaining: 0 }, { id: 201, remaining: 0 }], true);
  assert.equal(state.entityById(200), undefined, "forward seek cannot resurrect depleted patches");
  state.setObserverView({mode: "omniscient"});
  apply(4, [{ id: 201, remaining: 0 }], true);
  state.setObserverView({mode: "player", playerId: 1});
  apply(4, [{ id: 200, remaining: 17 }, { id: 201, remaining: 23 }], true);
  assert.equal(state.entityById(201).remaining, 23, "Alex view replaces omniscient knowledge");
  apply(4, [], true);
  assert.equal(state.entityById(201), undefined, "empty complete baseline clears previous perspective");
}
console.log("Resource vision contracts passed");
