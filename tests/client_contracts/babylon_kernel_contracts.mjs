import assert from "node:assert/strict";

import { FixedPerspectiveCamera } from "../../client/src/fixed_perspective_camera.js";
import {
  BABYLON_SCRIPT_URL,
  BABYLON_VERSION,
  RendererSelectionError,
  createSelectedBackendBundle,
  loadBabylonDependency,
  parseRendererSelection,
  showRendererBootstrapError,
} from "../../client/src/renderer/backend_selection.js";
import { createBabylonBackendBundle } from "../../client/src/renderer/babylon/backend_bundle.js";
import {
  projectScenePoint,
  sceneGroundHit,
  sceneGroundToWorld,
  sceneYawToWorldFacing,
  worldFacingToSceneYaw,
  worldPointToScene,
} from "../../client/src/renderer/babylon/coordinates.js";
import { BabylonPresentationAdapter } from "../../client/src/renderer/babylon/presentation_adapter.js";
import { PresentationFrameAssembler } from "../../client/src/presentation/frame.js";

const approx = (actual, expected, epsilon = 1e-6) => {
  assert.ok(Math.abs(actual - expected) <= epsilon, `${actual} should approximate ${expected}`);
};

assert.deepEqual(parseRendererSelection({ pathname: "/", search: "" }), { id: "pixi" });
assert.deepEqual(parseRendererSelection({ pathname: "/", search: "?rtsRenderer=pixi" }), { id: "pixi" });
assert.deepEqual(parseRendererSelection({ pathname: "/lab", search: "?rtsRenderer=babylon" }), { id: "babylon" });
assert.throws(
  () => parseRendererSelection({ pathname: "/", search: "?rtsRenderer=babylon" }),
  (error) => error instanceof RendererSelectionError && error.code === "unsupportedRendererRoute",
  "Babylon remains Lab-only in Phase 4",
);
assert.equal(
  (await createSelectedBackendBundle({
    locationLike: { pathname: "/", search: "" },
    documentLike: { head: { appendChild() { throw new Error("default path loaded Babylon"); } } },
  })).id,
  "pixi",
  "the default selector constructs Pixi without loading Babylon",
);
assert.throws(
  () => parseRendererSelection({ pathname: "/lab", search: "?rtsRenderer=webgpu" }),
  (error) => error instanceof RendererSelectionError && error.code === "invalidRenderer",
  "invalid selectors fail with one bounded error",
);

{
  const Babylon = fakeBabylon();
  let appended = null;
  const fakeDocument = {
    querySelector: () => null,
    createElement() {
      const listeners = {};
      return {
        dataset: {},
        addEventListener(type, listener) { listeners[type] = listener; },
        trigger(type) { listeners[type]?.(); },
      };
    },
    head: {
      appendChild(script) {
        appended = script;
        fakeGlobal.BABYLON = Babylon;
        script.trigger("load");
      },
    },
  };
  const fakeGlobal = {};
  assert.equal(await loadBabylonDependency({ documentLike: fakeDocument, globalLike: fakeGlobal }), Babylon);
  assert.equal(appended.src, BABYLON_SCRIPT_URL, "only the pinned Babylon URL is requested");
  assert.equal(appended.dataset.rtsBabylon, BABYLON_VERSION, "the lazy script records its exact version");
}

{
  const target = { hidden: true, setAttribute(name, value) { this[name] = value; } };
  const message = showRendererBootstrapError(
    new RendererSelectionError("invalidRenderer", "bounded selector failure"),
    { getElementById: () => target },
  );
  assert.equal(message, "bounded selector failure");
  assert.equal(target.hidden, false);
  assert.equal(target.role, "alert");
}

const camera = new FixedPerspectiveCamera(1000, 700, { minZoom: 0.2, maxZoom: 4 });
camera.setMapBounds(3200, 2400);
camera.focusAt({ x: 1600, y: 1200 });
const projection = camera.projectionSnapshot();
assert.equal(projection.project({ x: 1600, y: 1200, heightPx: 0 }).x, 500);
assert.equal(projection.project({ x: 1600, y: 1200, heightPx: 0 }).y, 350);
assert.ok(
  projection.project({ x: 1600, y: 1200, heightPx: 40 }).y < 350,
  "positive presentation height rises in the fixed perspective",
);
for (const point of [{ x: 1300, y: 1000 }, { x: 1600, y: 1200 }, { x: 1900, y: 1450 }]) {
  const projected = projection.project({ ...point, heightPx: 0 });
  const hit = projection.groundAtScreen(projected);
  approx(hit.x, point.x);
  approx(hit.y, point.y);
}
assert.ok(projection.viewportGroundPolygon().length >= 3, "perspective viewport exposes a bounded ground polygon");
assert.ok(Object.isFrozen(projection.perspective), "engine-independent perspective coefficients are detached");

{
  const fitCamera = new FixedPerspectiveCamera(1920, 1080, { minZoom: 0.1, maxZoom: 4 });
  fitCamera.setMapBounds(4096, 4096);
  const fitPoints = [{ x: 1000, y: 1000 }, { x: 3000, y: 3000 }];
  assert.equal(fitCamera.fitWorldPoints(fitPoints, { paddingCssPx: 64 }), true);
  for (const point of fitPoints) {
    const fitted = fitCamera.project({ ...point, heightPx: 0 });
    assert.ok(
      fitted.x >= 64 && fitted.x <= 1856 && fitted.y >= 64 && fitted.y <= 1016,
      "perspective fitting honors CSS viewport padding through the actual projection",
    );
  }
}

{
  const world = { x: 1234, y: 876, heightPx: 37 };
  const scene = worldPointToScene(world);
  const roundTrip = sceneGroundToWorld(scene);
  approx(roundTrip.x, world.x);
  approx(roundTrip.y, world.y);
  assert.deepEqual(projectScenePoint(scene, projection), projection.project(world));
  const screen = projection.project({ x: world.x, y: world.y, heightPx: 0 });
  const sceneHit = sceneGroundHit(screen, projection);
  approx(sceneHit.x, scene.x);
  approx(sceneHit.z, scene.z);
  approx(sceneYawToWorldFacing(worldFacingToSceneYaw(0.73)), 0.73);
}

{
  const sequence = [];
  const bundle = createBabylonBackendBundle({ Babylon: fakeBabylon() });
  const semanticCamera = bundle.createCamera();
  sequence.push(semanticCamera instanceof FixedPerspectiveCamera ? "camera" : "bad-camera");
  const priorDocument = globalThis.document;
  const dom = fakeDom();
  globalThis.document = dom.document;
  try {
    const renderer = bundle.createRenderer(dom.parent);
    sequence.push(renderer instanceof BabylonPresentationAdapter ? "renderer" : "bad-renderer");
    semanticCamera.resize(900, 600);
    semanticCamera.setMapBounds(2000, 1400);
    semanticCamera.focusAt({ x: 1000, y: 700 });
    const map = { width: 2, height: 2, tileSize: 1000, terrain: [0, 0, 0, 0], resources: [] };
    const assembler = new PresentationFrameAssembler({ map, entityStats: { tank: { size: 24 } } });
    const frame = assembler.assemble({
      map,
      projection: semanticCamera.projectionSnapshot(),
      frameContext: { alpha: 1, interpolatedEntities: [{
        id: 7, kind: "tank", owner: 1, x: 1000, y: 700, facing: 0.5, hp: 80, maxHp: 100,
      }] },
      fog: { visibleGrid: [1, 1, 1, 1], exploredGrid: [1, 1, 1, 1] },
      players: [{ id: 1, teamId: 1, color: "#336699" }],
      playerId: 1,
    });
    assert.ok(Object.isFrozen(frame.projection.perspective), "presentation frame retains detached scene coefficients");
    assert.equal(renderer.render(frame).presented, true);
    assert.equal(renderer._scene.renderCount, 1, "Match-driven render calls scene.render exactly once");
    assert.equal(renderer._engine.runRenderLoopCalls, 0, "Babylon never owns an engine render loop");
    renderer.resize(640, 480);
    assert.equal(renderer._canvas.style.width, "640px");
    assert.equal(renderer.captureReadiness().ready, true);
    renderer.destroy();
    renderer.destroy();
    assert.equal(dom.parent.children.length, 0, "idempotent teardown removes the one owned canvas");
    assert.equal(renderer._scene, null, "teardown releases the one owned scene");
    const reentered = bundle.createRenderer(dom.parent);
    assert.equal(reentered.render(frame).presented, true, "one normal leave/re-enter creates a fresh scene");
    assert.equal(dom.parent.children.length, 1, "re-entry still owns exactly one canvas");
    reentered.destroy();
    assert.equal(dom.parent.children.length, 0, "re-enter cleanup leaves no canvas behind");
  } finally {
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
  }
  assert.deepEqual(sequence, ["camera", "renderer"], "selected bundle creates camera before renderer");
}

function fakeDom() {
  const parent = {
    children: [],
    appendChild(node) {
      node.parentNode = this;
      this.children.push(node);
    },
  };
  const document = {
    createElement(tag) {
      return {
        tag,
        className: "",
        style: {},
        setAttribute() {},
        remove() {
          if (!this.parentNode) return;
          this.parentNode.children = this.parentNode.children.filter((child) => child !== this);
          this.parentNode = null;
        },
      };
    },
  };
  return { document, parent };
}

function fakeBabylon() {
  class Vector3 {
    constructor(x, y, z) { this.x = x; this.y = y; this.z = z; }
    copyFrom(value) { this.x = value.x; this.y = value.y; this.z = value.z; }
  }
  class Engine {
    static Version = BABYLON_VERSION;
    static isSupported() { return true; }
    constructor() { this.runRenderLoopCalls = 0; this.resizeCount = 0; this.disposed = false; }
    runRenderLoop() { this.runRenderLoopCalls += 1; }
    resize() { this.resizeCount += 1; }
    dispose() { this.disposed = true; }
  }
  class Scene {
    constructor(engine) { this.engine = engine; this.renderCount = 0; this.disposed = false; }
    render() { this.renderCount += 1; }
    dispose() { this.disposed = true; }
  }
  class FreeCamera {
    constructor(_name, position) {
      this.position = position;
      this.inputs = { clear() {} };
    }
    setTarget(target) { this.target = target; }
  }
  class Color3 {
    constructor(r, g, b) { this.r = r; this.g = g; this.b = b; }
    static FromHexString(value) { return new Color3(value, value, value); }
  }
  class Color4 { constructor(r, g, b, a) { Object.assign(this, { r, g, b, a }); } }
  class StandardMaterial {
    constructor() { this.disposed = false; }
    dispose() { this.disposed = true; }
  }
  class HemisphericLight { constructor() { this.intensity = 0; } }
  const mesh = () => ({
    position: new Vector3(0, 0, 0),
    rotation: { y: 0 },
    metadata: null,
    dispose() { this.disposed = true; },
    enableEdgesRendering() {},
  });
  return {
    Engine, Scene, FreeCamera, Vector3, Color3, Color4, StandardMaterial, HemisphericLight,
    MeshBuilder: { CreateGround: mesh, CreateBox: mesh },
  };
}

console.log("✅ babylon_kernel_contracts.mjs: selector, projection, coordinates, and lifecycle passed");
