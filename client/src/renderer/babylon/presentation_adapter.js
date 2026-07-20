import {
  projectionSceneCamera,
  worldPointToScene,
  worldScaleToScene,
} from "./coordinates.js";
import { PRESENTATION_OUTCOME, immediatePresentationSubmission } from "../../presentation/submission.js";
import { BabylonFeedbackLayer } from "./feedback_layer.js";
import { BabylonFogLayer } from "./fog_layer.js";
import { BabylonGenericEntities } from "./generic_entities.js";

export class BabylonPresentationAdapter {
  constructor(canvasParent, { Babylon, sources = null }) {
    this.id = "babylon";
    this._Babylon = Babylon;
    this._parent = canvasParent;
    this._sources = sources || {};
    this._destroyed = false;
    this._renderFrameCount = 0;
    this._lastError = null;
    this._canvas = null;
    this._engine = null;
    this._scene = null;
    this._camera = null;
    this._ground = null;
    this._groundMaterial = null;
    this._fogLayer = null;
    this._genericEntities = null;
    this._feedbackLayer = null;
    this._errorNode = null;
    try {
      this._createScene();
    } catch (error) {
      this._recordError("babylonCreation", error);
      this._showBoundedError("Babylon could not create a WebGL scene on this device.");
    }
  }

  get app() {
    return Object.freeze({ renderer: this._engine, view: this._canvas });
  }

  render(frame) {
    if (!frame || frame.version !== 2) throw new TypeError("Babylon requires PresentationFrameV2.");
    const identity = { generation: frame.generation, frameId: frame.frameId };
    if (this._destroyed) {
      return immediatePresentationSubmission({ ...identity, status: PRESENTATION_OUTCOME.DESTROYED });
    }
    if (!this._scene) {
      return immediatePresentationSubmission({
        ...identity,
        status: PRESENTATION_OUTCOME.FAILED,
        error: this._lastError?.message || "Babylon scene is unavailable.",
      });
    }
    const profiler = this._sources?.profiler?.() || null;
    const time = (label, fn) => profiler ? profiler.time(label, fn) : fn();
    let retainedRevision = 0;
    try {
      time("renderer.update", () => {
        this._syncCamera(frame.projection);
        this._syncGround(frame.projection?.mapBounds);
        this._genericEntities.sync(frame);
        this._fogLayer.sync(frame);
        this._feedbackLayer.sync(frame);
        retainedRevision = frame.groundDecalRevision;
      });
      time("renderer.present", () => this._present());
      this._renderFrameCount += 1;
      this._lastError = null;
      return immediatePresentationSubmission({
        ...identity,
        retainedRevision,
        status: PRESENTATION_OUTCOME.PRESENTED,
      });
    } catch (error) {
      this._recordError("babylonPresentationFrame", error);
      return immediatePresentationSubmission({
        ...identity,
        retainedRevision,
        status: PRESENTATION_OUTCOME.FAILED,
        error,
      });
    }
  }

  resize(widthCssPx, heightCssPx) {
    if (this._destroyed || !this._canvas || !this._engine) return;
    if (Number.isFinite(widthCssPx) && widthCssPx >= 0) this._canvas.style.width = `${widthCssPx}px`;
    if (Number.isFinite(heightCssPx) && heightCssPx >= 0) this._canvas.style.height = `${heightCssPx}px`;
    this._engine.resize();
  }

  captureReadiness() {
    const renderErrors = this._lastError ? [this._lastError] : [];
    return {
      frame: this._renderFrameCount,
      assets: [],
      ready: !!this._scene && !this._destroyed && renderErrors.length === 0,
      failedAssets: [],
      pendingAssets: [],
      renderErrors,
      missingTextureSubjectIds: [],
    };
  }

  sceneDiagnostics() {
    return Object.freeze({
      renderer: "babylon",
      frame: this._renderFrameCount,
      fog: this._fogLayer?.diagnostics?.() || null,
      genericEntities: this._genericEntities?.diagnostics?.() || null,
      feedback: this._feedbackLayer?.diagnostics?.() || null,
      error: this._lastError ? Object.freeze({ ...this._lastError }) : null,
    });
  }

  enterFixedCapture() {}
  exitFixedCapture() {}

  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;
    this._feedbackLayer?.destroy();
    this._feedbackLayer = null;
    this._fogLayer?.destroy();
    this._fogLayer = null;
    this._genericEntities?.destroy();
    this._genericEntities = null;
    this._ground?.dispose();
    this._ground = null;
    this._groundMaterial?.dispose();
    this._groundMaterial = null;
    this._scene?.dispose();
    this._scene = null;
    this._engine?.dispose();
    this._engine = null;
    this._canvas?.remove();
    this._canvas = null;
    this._errorNode?.remove();
    this._errorNode = null;
    this._sources = null;
  }

  _present() {
    this._scene.render();
  }

  _createScene() {
    const B = this._Babylon;
    if (!B?.Engine?.isSupported?.()) throw new Error("WebGL is unavailable.");
    const canvas = document.createElement("canvas");
    canvas.className = "rts-babylon-canvas";
    canvas.setAttribute("aria-label", "Babylon game world");
    this._parent.appendChild(canvas);
    this._canvas = canvas;
    this._engine = new B.Engine(canvas, true, { preserveDrawingBuffer: true, stencil: true });
    this._scene = new B.Scene(this._engine);
    this._scene.clearColor = new B.Color4(0.055, 0.075, 0.09, 1);
    this._camera = new B.FreeCamera("rts-camera", new B.Vector3(0, 10, -10), this._scene);
    this._camera.inputs.clear();
    const light = new B.HemisphericLight("kernel-light", new B.Vector3(-0.3, 1, -0.2), this._scene);
    light.intensity = 0.9;
    this._genericEntities = new BabylonGenericEntities(this._scene, { Babylon: B });
    this._fogLayer = new BabylonFogLayer(this._scene, { Babylon: B });
    this._feedbackLayer = new BabylonFeedbackLayer(this._scene, this._parent, { Babylon: B });
  }

  _syncCamera(projection) {
    const B = this._Babylon;
    const camera = projectionSceneCamera(projection);
    this._camera.position.copyFrom(new B.Vector3(camera.position.x, camera.position.y, camera.position.z));
    this._camera.setTarget(new B.Vector3(camera.target.x, camera.target.y, camera.target.z));
    this._camera.fov = camera.fovYRad;
    this._camera.minZ = Math.max(0.001, camera.nearScene);
    this._camera.maxZ = Math.max(this._camera.minZ + 1, camera.farScene);
  }

  _syncGround(mapBounds) {
    if (!mapBounds) return;
    const widthPx = mapBounds.maxX - mapBounds.minX;
    const heightPx = mapBounds.maxY - mapBounds.minY;
    const signature = `${mapBounds.minX}:${mapBounds.minY}:${widthPx}:${heightPx}`;
    if (this._ground?.metadata?.signature === signature) return;
    this._ground?.dispose();
    this._groundMaterial?.dispose();
    const B = this._Babylon;
    const ground = B.MeshBuilder.CreateGround("authoritative-map-bounds", {
      width: worldScaleToScene(widthPx),
      height: worldScaleToScene(heightPx),
      subdivisions: 1,
    }, this._scene);
    const center = worldPointToScene({
      x: mapBounds.minX + widthPx / 2,
      y: mapBounds.minY + heightPx / 2,
      heightPx: 0,
    });
    ground.position.copyFrom(new B.Vector3(center.x, center.y, center.z));
    const material = new B.StandardMaterial("kernel-ground-material", this._scene);
    material.diffuseColor = new B.Color3(0.18, 0.23, 0.18);
    material.specularColor = new B.Color3(0.02, 0.02, 0.02);
    ground.material = material;
    this._groundMaterial = material;
    ground.enableEdgesRendering();
    ground.edgesWidth = 2;
    ground.edgesColor = new B.Color4(0.68, 0.73, 0.62, 1);
    ground.metadata = { signature };
    this._ground = ground;
  }

  _recordError(label, error) {
    this._lastError = {
      label,
      count: 1,
      message: error?.message || String(error),
    };
  }

  _showBoundedError(message) {
    const node = document.createElement("div");
    node.className = "renderer-capability-error";
    node.setAttribute("role", "alert");
    node.textContent = message;
    this._parent.appendChild(node);
    this._errorNode = node;
  }
}
