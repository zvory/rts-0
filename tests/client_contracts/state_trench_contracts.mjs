// tests/client_contracts/state_trench_contracts.mjs
// Focused GameState trench snapshot contracts imported by ../client_contracts.mjs.

import { GameState } from "../../client/src/state.js";
import { KIND, TERRAIN } from "../../client/src/protocol.js";

import { assert } from "./assertions.mjs";

function startPayload() {
  return {
    playerId: 1,
    tick: 0,
    map: {
      width: 4,
      height: 4,
      tileSize: 32,
      terrain: new Array(16).fill(TERRAIN.GRASS),
      resources: [{ id: 200, kind: KIND.STEEL, x: 64, y: 96 }],
    },
    players: [{ id: 1, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 }],
  };
}

{
  const state = new GameState(startPayload());

  state.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 0,
    entities: [],
    trenches: [{ id: 300, x: 96, y: 128, radiusTiles: 0.75 }],
    events: [],
  });
  assert(state.trenches[0]?.id === 300, "snapshot trench state updates from authoritative terrain");

  state.applySnapshot({
    tick: 2,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 0,
    entities: [],
    events: [],
  });
  assert(state.trenches.length === 0, "snapshot trench state replaces prior trench terrain");
}
