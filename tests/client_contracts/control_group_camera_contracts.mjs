// Semantic camera contracts for control-group double-tap framing.

import { _jumpToControlGroupCluster } from "../../client/src/input/control_groups.js";
import { GameState } from "../../client/src/state.js";
import { KIND, STATE } from "../../client/src/protocol.js";
import { assert, assertApprox } from "./assertions.mjs";

function createHarness(entities, { focus = { x: 50, y: 50 }, clippedBounds = null } = {}) {
  let centered = null;
  const projection = {
    camera: { version: 1, focus, framingScale: 1, boundsPolicy: "mapOverscroll" },
    viewport: { widthCssPx: 100, heightCssPx: 100 },
    projectedExtent: () => ({ width: 1, height: 1, scaleX: 1, scaleY: 1, visible: true }),
  };
  const input = {
    camera: {
      projectionSnapshot: () => projection,
      viewportGroundBounds: () => clippedBounds,
      focusAt(point) { centered = point; },
    },
    selectionScene: { projection, proxies: entities.map((entity) => ({ id: entity.id })) },
    state: { controlGroups: [entities.map((entity) => entity.id)] },
    _visibleSelectionIds: (ids) => Array.from(ids),
    _selectionEntities: () => entities,
  };
  return { input, centered: () => centered };
}

{
  const state = new GameState({
    playerId: 1,
    players: [{ id: 1, teamId: 1, name: "A", color: "#f00", startTileX: 0, startTileY: 0 }],
    map: { width: 4, height: 4, tileSize: 32, tiles: Array(16).fill(0), resources: [] },
  });
  const workers = [
    { id: 199, owner: 1, kind: KIND.WORKER, x: 0, y: 0, state: STATE.IDLE },
    { id: 198, owner: 1, kind: KIND.WORKER, x: 1, y: 0, state: STATE.IDLE },
  ];
  const entityById = (id) => workers.find((entity) => entity.id === id) || null;
  state.setControlGroup(5, [workers[0].id], { entityById });
  state.addToControlGroup(5, [workers[1].id], { entityById });
  assert(
    state.controlGroups[5].join(",") === "199,198",
    "control-group save/add preserves last-presented entities absent from mutable snapshot state",
  );
}

{
  const harness = createHarness([
    { id: 1, x: 0, y: 0 },
    { id: 2, x: 20, y: 0 },
    { id: 3, x: 500, y: 500 },
  ]);
  assert(_jumpToControlGroupCluster.call(harness.input, 0), "control-group double-tap jumps to a cluster");
  assert(
    harness.centered().x < 100 && harness.centered().y < 100,
    "control-group jump chooses the dense cluster, not the all-entity centroid",
  );
}

{
  const harness = createHarness([
    { id: 1, x: 1, y: 1 },
    { id: 2, x: 99, y: 1 },
  ], {
    focus: { x: 25, y: 25 },
    clippedBounds: { minX: 0, minY: 0, maxX: 75, maxY: 75 },
  });
  assert(_jumpToControlGroupCluster.call(harness.input, 0), "control-group framing works during camera overscroll");
  assertApprox(
    harness.centered().x,
    49,
    0.001,
    "control-group framing uses the full projected viewport, not clipped map bounds",
  );
}

console.log("✅ control_group_camera_contracts.mjs: semantic control-group framing passed");
