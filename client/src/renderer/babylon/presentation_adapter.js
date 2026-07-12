import {
  projectionSceneCamera,
  worldFacingToSceneYaw,
  worldPointToScene,
  worldScaleToScene,
} from "./coordinates.js";

const MAX_KERNEL_ENTITIES = 96;

export class BabylonPresentationAdapter {
  constructor(canvasParent, { Babylon }) {
    this.id = "babylon";
    this._Babylon = Babylon;
    this._parent = canvasParent;
    this._destroyed = false;
    this._renderFrameCount = 0;
    this._lastError = null;
    this._entities = new Map();
    this._materials = new Map();
    this._canvas = null;
    this._engine = null;
    this._scene = null;
    this._camera = null;
    this._ground = null;
    this._groundMaterial = null;
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
    if (this._destroyed || !this._scene) return Object.freeze({ presented: false });
    try {
      if (!frame || frame.version !== 1) throw new TypeError("Babylon requires PresentationFrameV1.");
      this._syncCamera(frame.projection);
      this._syncGround(frame.projection?.mapBounds);
      this._syncEntities(frame.layers?.fogGatedWorld || []);
      this._scene.render();
      this._renderFrameCount += 1;
      this._lastError = null;
      return Object.freeze({ presented: true });
    } catch (error) {
      this._recordError("babylonPresentationFrame", error);
      return Object.freeze({ presented: false });
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

  enterFixedCapture() {}
  presentFixedCaptureFrame() { this._scene?.render(); }
  exitFixedCapture() {}

  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;
    for (const mesh of this._entities.values()) mesh.dispose();
    this._entities.clear();
    for (const material of this._materials.values()) material.dispose();
    this._materials.clear();
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

  _syncEntities(records) {
    const visible = records.filter((record) => record?.type === "entity").slice(0, MAX_KERNEL_ENTITIES);
    const retained = new Set();
    for (const record of visible) {
      const key = String(record.id);
      retained.add(key);
      let mesh = this._entities.get(key);
      if (!mesh) {
        mesh = this._createEntityMesh(record);
        this._entities.set(key, mesh);
      }
      const point = worldPointToScene({ x: record.x, y: record.y, heightPx: 0 });
      mesh.position.x = point.x;
      mesh.position.z = point.z;
      mesh.rotation.y = worldFacingToSceneYaw(Number.isFinite(record.facing) ? record.facing : 0);
      mesh.material = this._material(record.teamColor);
    }
    for (const [key, mesh] of this._entities) {
      if (retained.has(key)) continue;
      mesh.dispose();
      this._entities.delete(key);
    }
  }

  _createEntityMesh(record) {
    const B = this._Babylon;
    const height = worldScaleToScene(record.anchors?.hp?.heightPx || 18);
    const mesh = B.MeshBuilder.CreateBox(`visible-${record.id}`, {
      width: Math.max(0.35, height * 0.7),
      depth: Math.max(0.5, height),
      height: Math.max(0.35, height * 0.55),
    }, this._scene);
    mesh.position.y = Math.max(0.175, height * 0.275);
    mesh.isPickable = false;
    return mesh;
  }

  _material(color) {
    const key = /^#[0-9a-fA-F]{6}$/.test(color || "") ? color.toLowerCase() : "#9aa0a8";
    let material = this._materials.get(key);
    if (material) return material;
    const B = this._Babylon;
    material = new B.StandardMaterial(`team-${key.slice(1)}`, this._scene);
    material.diffuseColor = B.Color3.FromHexString(key);
    material.specularColor = new B.Color3(0.08, 0.08, 0.08);
    this._materials.set(key, material);
    return material;
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
