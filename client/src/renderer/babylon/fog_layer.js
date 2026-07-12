import { worldPointToScene, worldScaleToScene } from "./coordinates.js";

const FOG_ALPHA = Object.freeze({ visible: 0, explored: 0.48, unknown: 0.92 });

export class BabylonFogLayer {
  constructor(scene, { Babylon }) {
    this._Babylon = Babylon;
    this._scene = scene;
    this._mesh = null;
    this._material = null;
    this._texture = null;
    this._shapeSignature = "";
    this._revisionSignature = "";
    this._diagnostics = { visibleRevision: null, exploredRevision: null, visibleTiles: 0, exploredTiles: 0, unknownTiles: 0 };
  }

  sync(frame) {
    const visible = frame?.visible;
    const explored = frame?.explored;
    const bounds = frame?.projection?.mapBounds;
    if (!validGrid(visible) || !validGrid(explored) || !bounds) {
      throw new TypeError("Babylon fog requires revisioned visible/explored grid snapshots and map bounds.");
    }
    if (visible.width !== explored.width || visible.height !== explored.height) {
      throw new TypeError("Babylon fog grids must have matching dimensions.");
    }
    const widthPx = bounds.maxX - bounds.minX;
    const heightPx = bounds.maxY - bounds.minY;
    const shapeSignature = `${visible.width}:${visible.height}:${bounds.minX}:${bounds.minY}:${widthPx}:${heightPx}`;
    if (shapeSignature !== this._shapeSignature) this._rebuild(visible, bounds, widthPx, heightPx);

    const revisionSignature = `${frame.generation}:${visible.revision}:${explored.revision}`;
    if (revisionSignature !== this._revisionSignature) {
      this._paint(visible, explored);
      this._revisionSignature = revisionSignature;
    }
  }

  diagnostics() {
    return Object.freeze({ ...this._diagnostics });
  }

  destroy() {
    this._mesh?.dispose();
    this._material?.dispose();
    this._texture?.dispose();
    this._mesh = null;
    this._material = null;
    this._texture = null;
  }

  _rebuild(grid, bounds, widthPx, heightPx) {
    this.destroy();
    const B = this._Babylon;
    const texture = new B.DynamicTexture(
      "authoritative-fog-texture",
      { width: grid.width, height: grid.height },
      this._scene,
      false,
      B.Texture?.NEAREST_SAMPLINGMODE,
    );
    texture.hasAlpha = true;
    texture.wrapU = B.Texture?.CLAMP_ADDRESSMODE;
    texture.wrapV = B.Texture?.CLAMP_ADDRESSMODE;
    const material = new B.StandardMaterial("authoritative-fog-material", this._scene);
    material.diffuseTexture = texture;
    material.opacityTexture = texture;
    material.useAlphaFromDiffuseTexture = true;
    material.disableLighting = true;
    material.backFaceCulling = false;
    material.alpha = 1;
    const mesh = B.MeshBuilder.CreateGround("authoritative-fog", {
      width: worldScaleToScene(widthPx),
      height: worldScaleToScene(heightPx),
      subdivisions: 1,
    }, this._scene);
    const center = worldPointToScene({
      x: bounds.minX + widthPx / 2,
      y: bounds.minY + heightPx / 2,
      heightPx: 0.4,
    });
    mesh.position.copyFrom(new B.Vector3(center.x, center.y, center.z));
    mesh.material = material;
    mesh.isPickable = false;
    mesh.renderingGroupId = 2;
    this._texture = texture;
    this._material = material;
    this._mesh = mesh;
    this._shapeSignature = `${grid.width}:${grid.height}:${bounds.minX}:${bounds.minY}:${widthPx}:${heightPx}`;
  }

  _paint(visible, explored) {
    const context = this._texture.getContext();
    const image = context.createImageData(visible.width, visible.height);
    let visibleTiles = 0;
    let exploredTiles = 0;
    let unknownTiles = 0;
    for (let index = 0; index < visible.width * visible.height; index += 1) {
      let alpha;
      if (visible.get(index)) {
        alpha = FOG_ALPHA.visible;
        visibleTiles += 1;
      } else if (explored.get(index)) {
        alpha = FOG_ALPHA.explored;
        exploredTiles += 1;
      } else {
        alpha = FOG_ALPHA.unknown;
        unknownTiles += 1;
      }
      const offset = index * 4;
      image.data[offset] = 8;
      image.data[offset + 1] = 13;
      image.data[offset + 2] = 18;
      image.data[offset + 3] = Math.round(alpha * 255);
    }
    context.putImageData(image, 0, 0);
    this._texture.update(false);
    this._diagnostics = {
      visibleRevision: visible.revision,
      exploredRevision: explored.revision,
      visibleTiles,
      exploredTiles,
      unknownTiles,
    };
  }
}

function validGrid(grid) {
  return grid?.version === 1 && Number.isInteger(grid.width) && Number.isInteger(grid.height)
    && typeof grid.get === "function";
}
