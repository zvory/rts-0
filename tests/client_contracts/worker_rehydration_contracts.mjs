import { assert } from "./assertions.mjs";
import { createWorkerPresentationState } from "../../client/src/renderer/worker_rehydration.js";

const state = createWorkerPresentationState();
state.reset(1);
state.map(request("mapGeneration", {
  map: { version: 1, revision: 1, width: 1, height: 1, tileSize: 32, terrain: grid(1, [0]) },
}));
state.revisions(request("revisionedGrids", {
  revisions: { visible: grid(1, [1]), explored: grid(1, [1]) },
}));
state.retainDecals(request("durableDecals", { revision: 1, decals: [decal(1)] }));
const first = state.frame(request("frame", { frame: frame(1, 1) }));
state.retainDecals(request("durableDecals", { revision: 2, decals: [decal(2)] }));
state.decalsPresented(first.groundDecalRevision);
const second = state.frame(request("frame", { frame: frame(2, 2) }));
assert(second.layers.persistentGroundMark.some((record) => record.id === 2),
  "acknowledging an older frame retains a newer independently delivered decal revision");
assert(!second.layers.persistentGroundMark.some((record) => record.id === 1),
  "acknowledging an older frame retires only the durable decals that frame included");

function request(type, payload) {
  return { generation: 1, type, payload };
}

function grid(revision, values) {
  return { version: 1, revision, width: 1, height: 1, values: Uint8Array.from(values).buffer };
}

function decal(id) {
  return { type: "groundDecal", id };
}

function frame(frameId, revision) {
  return {
    frameId,
    staticMapRevision: 1,
    visible: { revision: 1 },
    explored: { revision: 1 },
    groundDecalRevision: revision,
    layers: { persistentGroundMark: [] },
  };
}
