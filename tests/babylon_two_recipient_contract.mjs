import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

import { STATS } from "../client/src/config.js";
import { FixedPerspectiveCamera } from "../client/src/fixed_perspective_camera.js";
import { buildSelectionScene } from "../client/src/input/selection_projection.js";
import { PresentationFrameAssembler } from "../client/src/presentation/frame.js";
import { BabylonPresentationAdapter } from "../client/src/renderer/babylon/presentation_adapter.js";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const fixture = JSON.parse(execFileSync("cargo", [
  "run", "--quiet", "--manifest-path", "server/Cargo.toml", "-p", "rts-sim",
  "--example", "babylon_secrecy_fixture",
], { cwd: root, encoding: "utf8" }));
const sentinelId = fixture.sentinelId;
assert.ok(
  fixture.recipientTwo.entities.some((entity) => entity.id === sentinelId),
  "the owning recipient proves the authoritative sentinel exists",
);
assert.ok(
  fixture.recipientOne.entities.every((entity) => entity.id !== sentinelId),
  "the other real server projection never receives the sentinel",
);

const camera = new FixedPerspectiveCamera(1000, 700, { minZoom: 0.1, maxZoom: 4 });
camera.setMapBounds(fixture.map.width * fixture.map.tileSize, fixture.map.height * fixture.map.tileSize);
camera.focusAt({ x: 800, y: 800 });
const projection = camera.projectionSnapshot();
const players = fixture.players.map((player) => ({
  ...player,
  teamId: player.team_id,
}));
const assembler = new PresentationFrameAssembler({ map: fixture.map, entityStats: STATS });
const frame = assembler.assemble({
  map: fixture.map,
  projection,
  frameContext: { alpha: 1, interpolatedEntities: fixture.recipientOne.entities },
  rememberedBuildings: fixture.recipientOne.rememberedBuildings,
  fog: {
    visibleGrid: fixture.recipientOne.visibleTiles,
    exploredGrid: fixture.recipientOne.visibleTiles,
    visibleRevision: fixture.recipientOne.tick + 1,
    exploredRevision: fixture.recipientOne.tick + 1,
  },
  players,
  playerId: 1,
  sourceTick: fixture.recipientOne.tick,
});
const presentedRecords = Object.values(frame.layers).flat();
assert.ok(
  presentedRecords.every((record) => record?.id !== sentinelId),
  "PresentationFrameV1 cannot reconstruct the never-received sentinel",
);
const selection = buildSelectionScene({
  entities: fixture.recipientOne.entities,
  projection,
  tileSize: fixture.map.tileSize,
  generation: frame.generation,
  frameId: frame.frameId,
});
assert.ok(
  selection.proxies.every((proxy) => proxy.id !== sentinelId),
  "SelectionSceneV1 has no picking candidate for the never-received sentinel",
);

const priorDocument = globalThis.document;
const dom = fakeDom();
globalThis.document = dom.document;
try {
  const renderer = new BabylonPresentationAdapter(dom.parent, { Babylon: fakeBabylon() });
  assert.equal((await renderer.render(frame).settled).status, "presented", "the recipient-one frame reaches the Babylon scene");
  assert.ok(
    renderer._scene.meshes.every((mesh) => !mesh.name?.includes(`:${sentinelId}`)),
    "the Babylon scene creates no sentinel instance",
  );
  const diagnostics = renderer.sceneDiagnostics();
  assert.deepEqual(
    Object.keys(diagnostics.genericEntities).sort(),
    ["categories", "hpBars", "placeholderKinds", "receivedEntities", "selected", "sharedGeometrySources", "sharedMaterials"].sort(),
    "Babylon diagnostics remain aggregate-only and expose no entity ids or positions",
  );
  renderer.destroy();
} finally {
  if (priorDocument === undefined) delete globalThis.document;
  else globalThis.document = priorDocument;
}

function fakeDom() {
  const parent = {
    children: [],
    appendChild(node) { node.parentNode = this; this.children.push(node); },
  };
  return {
    parent,
    document: {
      createElement(tag) {
        return {
          tag, className: "", style: {}, hidden: false,
          setAttribute() {},
          remove() {
            if (!this.parentNode) return;
            this.parentNode.children = this.parentNode.children.filter((child) => child !== this);
            this.parentNode = null;
          },
        };
      },
    },
  };
}

function fakeBabylon() {
  class Vector3 {
    constructor(x, y, z) { Object.assign(this, { x, y, z }); }
    copyFrom(value) { Object.assign(this, { x: value.x, y: value.y, z: value.z }); }
  }
  class Engine {
    static isSupported() { return true; }
    resize() {}
    dispose() {}
  }
  class Scene {
    constructor() { this.meshes = []; }
    render() {}
    dispose() {}
  }
  class FreeCamera {
    constructor(_name, position) { this.position = position; this.inputs = { clear() {} }; }
    setTarget(target) { this.target = target; }
  }
  class Color3 {
    constructor(r, g, b) { Object.assign(this, { r, g, b }); }
    static FromHexString(value) { return new Color3(value, value, value); }
  }
  class Color4 { constructor(r, g, b, a) { Object.assign(this, { r, g, b, a }); } }
  class StandardMaterial { dispose() {} }
  class DynamicTexture {
    constructor() {
      this.context = {
        createImageData: (width, height) => ({ data: new Uint8ClampedArray(width * height * 4) }),
        putImageData() {},
      };
    }
    getContext() { return this.context; }
    update() {}
    dispose() {}
  }
  class HemisphericLight {}
  const mesh = (name = "mesh", _options = {}, scene = null) => {
    const value = {
      name,
      position: new Vector3(0, 0, 0),
      scaling: new Vector3(1, 1, 1),
      rotation: { y: 0 },
      dispose() {},
      enableEdgesRendering() {},
      createInstance(instanceName) { return mesh(instanceName, {}, scene); },
    };
    scene?.meshes?.push(value);
    return value;
  };
  return {
    Engine, Scene, FreeCamera, Vector3, Color3, Color4, StandardMaterial, DynamicTexture, HemisphericLight,
    Texture: { NEAREST_SAMPLINGMODE: 1, CLAMP_ADDRESSMODE: 0 },
    MeshBuilder: { CreateGround: mesh, CreateBox: mesh, CreateTorus: mesh, CreateLines: mesh },
  };
}

console.log("✅ babylon_two_recipient_contract.mjs: server projection, frame, scene, picking, and diagnostics passed");
