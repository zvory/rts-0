import {
  worldFacingToSceneYaw,
  worldPointToScene,
  worldScaleToScene,
} from "./coordinates.js";

const CATEGORY_SPECS = Object.freeze({
  current: Object.freeze({ layer: "fogGatedWorld", type: "entity", alpha: 1, emissive: false }),
  remembered: Object.freeze({ layer: "rememberedWorld", type: "rememberedBuilding", alpha: 0.42, emissive: false }),
  intel: Object.freeze({ layer: "belowFogIntel", type: "intelEntity", alpha: 0.56, emissive: false }),
  reveal: Object.freeze({ layer: "aboveFogReveal", type: "shotRevealEntity", alpha: 0.9, emissive: true }),
});

export class BabylonGenericEntities {
  constructor(scene, { Babylon }) {
    this._Babylon = Babylon;
    this._scene = scene;
    this._instances = new Map();
    this._sources = new Map();
    this._materials = new Map();
    this._diagnostics = emptyDiagnostics();
  }

  sync(frame) {
    const retained = new Set();
    const categories = {};
    const placeholderKinds = new Set();
    let selected = 0;
    let hpBars = 0;
    for (const [category, spec] of Object.entries(CATEGORY_SPECS)) {
      const records = (frame?.layers?.[spec.layer] || []).filter((record) => record?.type === spec.type);
      categories[category] = records.length;
      for (const record of records) {
        const key = `${category}:${record.id}`;
        retained.add(key);
        placeholderKinds.add(record.kind);
        let visual = this._instances.get(key);
        if (!visual) {
          visual = this._createVisual(key, category, record);
          this._instances.set(key, visual);
        }
        this._updateVisual(visual, category, record);
        if (record.selected) selected += 1;
        if (finiteRatio(record.hp, record.maxHp) != null) hpBars += 1;
      }
    }
    for (const [key, visual] of this._instances) {
      if (retained.has(key)) continue;
      disposeVisual(visual);
      this._instances.delete(key);
    }
    this._diagnostics = {
      receivedEntities: retained.size,
      categories,
      selected,
      hpBars,
      placeholderKinds: Array.from(placeholderKinds).filter(Boolean).sort().slice(0, 64),
      sharedGeometrySources: this._sources.size,
      sharedMaterials: this._materials.size,
    };
  }

  diagnostics() {
    return Object.freeze({
      ...this._diagnostics,
      categories: Object.freeze({ ...this._diagnostics.categories }),
      placeholderKinds: Object.freeze([...this._diagnostics.placeholderKinds]),
    });
  }

  destroy() {
    for (const visual of this._instances.values()) disposeVisual(visual);
    this._instances.clear();
    for (const mesh of this._sources.values()) mesh.dispose();
    this._sources.clear();
    for (const material of this._materials.values()) material.dispose();
    this._materials.clear();
  }

  _createVisual(key, category, record) {
    const bounds = normalizedBounds(record.visualBounds);
    const bodySource = this._source(category, record.teamColor, bounds.class);
    const body = bodySource.createInstance(`placeholder-${key}`);
    body.isPickable = false;
    const selection = this._sourceMesh("selection-ring", () => {
      const mesh = this._Babylon.MeshBuilder.CreateTorus("shared-selection-ring", {
        diameter: 1, thickness: 0.055, tessellation: 24,
      }, this._scene);
      mesh.material = this._material("selection", "#83e6ff", 0.95, true);
      return mesh;
    }).createInstance(`selection-${key}`);
    selection.isPickable = false;
    const hpBack = this._sourceMesh("hp-back", () => this._barSource("hp-back", "#191d20")).createInstance(`hp-back-${key}`);
    const hpFill = this._sourceMesh("hp-fill", () => this._barSource("hp-fill", "#70df78")).createInstance(`hp-fill-${key}`);
    const progress = this._sourceMesh("progress", () => this._barSource("progress", "#e5bc5d")).createInstance(`progress-${key}`);
    return { body, selection, hpBack, hpFill, progress };
  }

  _updateVisual(visual, category, record) {
    const B = this._Babylon;
    const bounds = normalizedBounds(record.visualBounds);
    const construction = constructionFraction(record);
    const width = worldScaleToScene(bounds.widthPx);
    const depth = worldScaleToScene(bounds.depthPx);
    const fullHeight = worldScaleToScene(bounds.heightPx);
    const height = Math.max(worldScaleToScene(2), fullHeight * construction);
    const point = worldPointToScene({ x: record.x, y: record.y, heightPx: 0 });
    visual.body.position.copyFrom(new B.Vector3(point.x, height / 2, point.z));
    visual.body.rotation.y = worldFacingToSceneYaw(Number.isFinite(record.facing) ? record.facing : 0);
    visual.body.scaling.copyFrom(new B.Vector3(width, height, depth));
    visual.body.isVisible = true;

    visual.selection.position.copyFrom(new B.Vector3(point.x, worldScaleToScene(1), point.z));
    visual.selection.scaling.copyFrom(new B.Vector3(width * 1.18, 1, depth * 1.18));
    visual.selection.isVisible = category === "current" && !!record.selected;

    const hpRatio = finiteRatio(record.hp, record.maxHp);
    const showHp = category === "current" && hpRatio != null && (record.selected || hpRatio < 0.999);
    const hpHeight = worldScaleToScene(record.anchors?.hp?.heightPx || bounds.heightPx) + worldScaleToScene(4);
    syncBar(visual.hpBack, point, hpHeight, width, 1, showHp, B);
    syncBar(visual.hpFill, point, hpHeight + worldScaleToScene(0.5), width, hpRatio ?? 0, showHp, B);

    const progressRatio = progressFraction(record);
    syncBar(
      visual.progress,
      point,
      hpHeight + worldScaleToScene(3),
      width,
      progressRatio ?? 0,
      category === "current" && progressRatio != null,
      B,
    );
  }

  _source(category, color, shapeClass) {
    const key = `body:${category}:${normalizedColor(color)}:${shapeClass}`;
    return this._sourceMesh(key, () => {
      const mesh = this._Babylon.MeshBuilder.CreateBox(`shared-${key}`, { size: 1 }, this._scene);
      const spec = CATEGORY_SPECS[category];
      mesh.material = this._material(key, color, spec.alpha, spec.emissive);
      return mesh;
    });
  }

  _barSource(key, color) {
    const mesh = this._Babylon.MeshBuilder.CreateBox(`shared-${key}`, { size: 1 }, this._scene);
    mesh.material = this._material(key, color, 0.96, true);
    return mesh;
  }

  _sourceMesh(key, create) {
    let mesh = this._sources.get(key);
    if (mesh) return mesh;
    mesh = create();
    mesh.isVisible = false;
    mesh.isPickable = false;
    this._sources.set(key, mesh);
    return mesh;
  }

  _material(key, color, alpha, emissive) {
    const materialKey = `${key}:${normalizedColor(color)}:${alpha}:${emissive}`;
    let material = this._materials.get(materialKey);
    if (material) return material;
    const B = this._Babylon;
    material = new B.StandardMaterial(`shared-material-${this._materials.size}`, this._scene);
    material.diffuseColor = B.Color3.FromHexString(normalizedColor(color));
    material.specularColor = new B.Color3(0.06, 0.06, 0.06);
    material.alpha = alpha;
    if (emissive) material.emissiveColor = material.diffuseColor;
    this._materials.set(materialKey, material);
    return material;
  }
}

function syncBar(mesh, point, height, width, ratio, visible, B) {
  const normalized = Math.max(0, Math.min(1, ratio));
  mesh.position.copyFrom(new B.Vector3(point.x - width * (1 - normalized) / 2, height, point.z));
  mesh.scaling.copyFrom(new B.Vector3(Math.max(0.001, width * normalized), worldScaleToScene(1.5), worldScaleToScene(1.5)));
  mesh.isVisible = visible;
  mesh.isPickable = false;
}

function normalizedBounds(bounds) {
  return {
    class: bounds?.class === "building" ? "building" : "unit",
    widthPx: positive(bounds?.widthPx, 16),
    depthPx: positive(bounds?.depthPx, 16),
    heightPx: positive(bounds?.heightPx, 16),
  };
}

function constructionFraction(record) {
  const build = finiteRatio(record?.buildProgress, 1);
  const deconstruct = finiteRatio(record?.deconstructProgress, 1);
  if (build != null) return Math.max(0.12, build);
  if (deconstruct != null) return Math.max(0.12, 1 - deconstruct);
  return 1;
}

function progressFraction(record) {
  return finiteRatio(record?.buildProgress, 1)
    ?? finiteRatio(record?.deconstructProgress, 1)
    ?? finiteRatio(record?.prodProgress, 1);
}

function finiteRatio(value, maximum) {
  const number = Number(value);
  const max = Number(maximum);
  if (!Number.isFinite(number) || !Number.isFinite(max) || max <= 0) return null;
  return Math.max(0, Math.min(1, number / max));
}

function positive(value, fallback) {
  return Number.isFinite(Number(value)) && Number(value) > 0 ? Number(value) : fallback;
}

function normalizedColor(color) {
  return /^#[0-9a-fA-F]{6}$/.test(color || "") ? color.toLowerCase() : "#9aa0a8";
}

function disposeVisual(visual) {
  for (const mesh of Object.values(visual)) mesh?.dispose();
}

function emptyDiagnostics() {
  return { receivedEntities: 0, categories: {}, selected: 0, hpBars: 0, placeholderKinds: [], sharedGeometrySources: 0, sharedMaterials: 0 };
}
