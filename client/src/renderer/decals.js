import { STATS } from "../config.js";
import { KIND } from "../protocol.js";
import {
  canLoadGroundDecalAtlas,
  GROUND_DECAL_ATLAS_STATUS,
  loadGroundDecalAtlas,
} from "./decals/asset_loader.js";
import {
  createGroundDecalStampPlan,
  mulberry32,
  normalizeColorNumber,
  rgba,
} from "./decals/selection.js";
import { createWorkerSafeCanvas } from "./raster_primitives.js";

export const GROUND_DECAL_TEXTURE_WORLD_SCALE = 4;

const DECAL_CLASS_INFANTRY = "infantry";
const DECAL_CLASS_SCORCH = "scorch";
const DECAL_CLASS_BUILDING_SCORCH = "buildingScorch";
const DECAL_CLASS_MORTAR_BLAST = "mortarBlast";
const DECAL_CLASS_ARTILLERY_BLAST = "artilleryBlast";
const SCORCH_DARK = 0x070706;
const SCORCH_ASH = 0x181816;
const BLAST_SOIL = 0x5a4024;
const BLAST_CHAR = 0x1d170e;
const MORTAR_BLAST_RADIUS_WORLD = 48;
const ARTILLERY_BLAST_RADIUS_WORLD = 64;
const VEHICLE_SCORCH_MASK_LENGTH = 62;
const VEHICLE_SCORCH_MASK_WIDTH = 38;
const TANK_SCORCH_SCALE_X = 1.18;
const TANK_SCORCH_SCALE_Y = 1.03;
const TANK_SCORCH_OPACITY_SCALE = 1.28;
const TANK_ASH_OPACITY_SCALE = 1.45;
const TANK_PAINT_OPACITY_SCALE = 1.35;

export class GroundDecalLayer {
  constructor({
    layer,
    pixi = globalThis.PIXI,
    createCanvas = createWorkerSafeCanvas,
    downsample = GROUND_DECAL_TEXTURE_WORLD_SCALE,
    recordDiagnostic = null,
    loadAtlas = loadGroundDecalAtlas,
  } = {}) {
    this.layer = layer;
    this.pixi = pixi;
    this.createCanvas = createCanvas;
    this.downsample = downsample;
    this.recordDiagnostic = recordDiagnostic;
    this.loadAtlas = loadAtlas;
    this.canvas = null;
    this.ctx = null;
    this.texture = null;
    this.sprite = null;
    this.atlas = null;
    this.assetStatus = GROUND_DECAL_ATLAS_STATUS.IDLE;
    this.assetLoadPromise = null;
    this.assetLoadError = null;
    this._assetLoadGeneration = 0;
    this._queuedUntilAssets = [];
    this._tintScratch = null;
    this.totalStamped = 0;
    this.textureUpdateCount = 0;
  }

  resetForMap(map) {
    this.destroy();
    this.totalStamped = 0;
    this.textureUpdateCount = 0;
    if (!this.pixi?.Texture || !this.pixi?.Sprite || !this.layer) return false;
    const tileSize = Number.isFinite(map?.tileSize) ? map.tileSize : 32;
    const worldWidth = Math.max(1, (map?.width || 1) * tileSize);
    const worldHeight = Math.max(1, (map?.height || 1) * tileSize);
    this.canvas = this.createCanvas();
    this.canvas.width = Math.max(1, Math.ceil(worldWidth / this.downsample));
    this.canvas.height = Math.max(1, Math.ceil(worldHeight / this.downsample));
    this.ctx = this.canvas.getContext("2d", { alpha: true });
    if (!this.ctx) {
      this.canvas = null;
      return false;
    }
    this.ctx.imageSmoothingEnabled = false;
    this.texture = this.pixi.Texture.from(this.canvas);
    this.sprite = new this.pixi.Sprite(this.texture);
    this.sprite.scale.set(this.downsample);
    this.layer.addChild(this.sprite);
    this.recordDiagnostic?.("renderer.groundDecals.displayObjects", this.displayObjectCount());
    this._beginAssetLoad();
    return true;
  }

  stampBatch(decals, { onError = null } = {}) {
    if (!this.ctx || !Array.isArray(decals) || decals.length === 0) return 0;
    if (this.assetStatus === GROUND_DECAL_ATLAS_STATUS.FAILED) {
      throw this.assetLoadError || new Error("ground decal PNG atlas is unavailable");
    }
    if (this.assetStatus === GROUND_DECAL_ATLAS_STATUS.PENDING) {
      this.recordDiagnostic?.("renderer.groundDecals.awaitingAtlas", decals.length);
      return 0;
    }
    const batch = this._queuedUntilAssets.length > 0
      ? this._queuedUntilAssets.splice(0).concat(decals)
      : decals;
    return this._stampDecodedBatch(batch, { onError });
  }

  _beginAssetLoad() {
    this._assetLoadGeneration += 1;
    const generation = this._assetLoadGeneration;
    this.atlas?.destroy?.();
    this.atlas = null;
    this.assetLoadError = null;
    this.assetLoadPromise = null;

    if (!canLoadGroundDecalAtlas()) {
      this.assetStatus = GROUND_DECAL_ATLAS_STATUS.FAILED;
      this.recordDiagnostic?.("renderer.groundDecals.assetFallback", 1);
      return;
    }

    this.assetStatus = GROUND_DECAL_ATLAS_STATUS.PENDING;
    this.recordDiagnostic?.("renderer.groundDecals.assetLoadPending", 1);
    this.assetLoadPromise = this.loadAtlas().then((atlas) => {
      if (generation !== this._assetLoadGeneration || !this.ctx) {
        atlas?.destroy?.();
        return null;
      }
      this.atlas = atlas;
      this.assetStatus = GROUND_DECAL_ATLAS_STATUS.READY;
      this.recordDiagnostic?.("renderer.groundDecals.assetLoadReady", 1);
      return atlas;
    }).catch((err) => {
      if (generation !== this._assetLoadGeneration) return null;
      this.assetLoadError = err;
      this.assetStatus = GROUND_DECAL_ATLAS_STATUS.FAILED;
      this.recordDiagnostic?.("renderer.groundDecals.assetLoadFailed", 1);
      return null;
    });
  }

  _stampQueuedAfterAssetSettled() {
    if (!this.ctx || this.assetStatus === GROUND_DECAL_ATLAS_STATUS.PENDING) return 0;
    if (this._queuedUntilAssets.length === 0) return 0;
    const queued = this._queuedUntilAssets.splice(0);
    return this._stampDecodedBatch(queued);
  }

  _stampDecodedBatch(decals, { onError = null } = {}) {
    if (!this.ctx || !Array.isArray(decals) || decals.length === 0) return 0;
    this.recordDiagnostic?.("renderer.groundDecals.pending", decals.length);
    let stamped = 0;
    const tintScratch = this.assetStatus === GROUND_DECAL_ATLAS_STATUS.READY
      ? this._ensureTintScratch()
      : null;
    const atlas = this.assetStatus === GROUND_DECAL_ATLAS_STATUS.READY ? this.atlas : null;
    for (const decal of decals) {
      try {
        if (stampGroundDecal(this.ctx, decal, this.downsample, { atlas, tintScratch })) stamped += 1;
        else this.recordDiagnostic?.("renderer.groundDecals.skipped", 1);
      } catch (err) {
        this.recordDiagnostic?.("renderer.groundDecals.skipped", 1);
        onError?.("groundDecal", err);
      }
    }
    if (stamped > 0) {
      updateTexture(this.texture);
      this.totalStamped += stamped;
      this.textureUpdateCount += 1;
      this.recordDiagnostic?.("renderer.groundDecals.stamped", stamped);
      this.recordDiagnostic?.("renderer.groundDecals.textureUpdates", 1);
    }
    return stamped;
  }

  _ensureTintScratch() {
    if (this._tintScratch?.ctx) return this._tintScratch;
    const canvas = this.createCanvas();
    const ctx = canvas.getContext("2d", { alpha: true });
    if (!ctx) return null;
    ctx.imageSmoothingEnabled = false;
    this._tintScratch = { canvas, ctx };
    return this._tintScratch;
  }

  displayObjectCount() {
    return Array.isArray(this.layer?.children) ? this.layer.children.length : 0;
  }

  diagnostics() {
    return {
      totalStamped: this.totalStamped,
      pendingDecals: this._queuedUntilAssets.length,
      textureUpdateCount: this.textureUpdateCount,
      textureWidth: this.canvas?.width || 0,
      textureHeight: this.canvas?.height || 0,
      downsample: this.downsample,
      layerChildCount: this.displayObjectCount(),
      assetStatus: this.assetStatus,
    };
  }

  destroy() {
    this._assetLoadGeneration += 1;
    this.assetLoadPromise = null;
    this.assetLoadError = null;
    this.assetStatus = GROUND_DECAL_ATLAS_STATUS.IDLE;
    this._queuedUntilAssets = [];
    this.atlas?.destroy?.();
    this.atlas = null;
    if (this._tintScratch?.canvas) {
      this._tintScratch.canvas.width = 0;
      this._tintScratch.canvas.height = 0;
    }
    this._tintScratch = null;
    if (this.sprite) {
      if (this.sprite.parent && typeof this.sprite.parent.removeChild === "function") {
        this.sprite.parent.removeChild(this.sprite);
      }
      this.sprite.destroy?.({ texture: true, textureSource: true });
      this.sprite = null;
    } else if (this.texture) {
      this.texture.destroy?.(true);
      this.texture.source?.destroy?.();
    }
    this.texture = null;
    if (this.ctx && this.canvas) {
      this.ctx.clearRect?.(0, 0, this.canvas.width || 0, this.canvas.height || 0);
    }
    if (this.canvas) {
      this.canvas.width = 0;
      this.canvas.height = 0;
    }
    this.ctx = null;
    this.canvas = null;
  }
}

export function _initGroundDecalsForMap(map) {
  this._groundDecals?.resetForMap(map);
}

export function _drawGroundDecals(source) {
  if (!this._groundDecals) return 0;
  if (!this._groundDecals.ctx) return 0;
  const decals = Array.isArray(source)
    ? source
    : typeof source?.consumePendingGroundDecals === "function"
      ? source.consumePendingGroundDecals()
      : [];
  return this._groundDecals.stampBatch(decals, {
    onError: (label, err) => this._recordRenderError?.(label, err),
  });
}

export function stampGroundDecal(
  ctx,
  decal,
  downsample = GROUND_DECAL_TEXTURE_WORLD_SCALE,
  { atlas = null, tintScratch = null } = {},
) {
  if (decal?.decalClass === DECAL_CLASS_BUILDING_SCORCH) {
    return stampBuildingScorch(ctx, decal, downsample);
  }
  if (atlas && stampAuthoredGroundDecal(ctx, decal, atlas, downsample, tintScratch)) return true;
  return stampProceduralGroundDecal(ctx, decal, downsample);
}

function stampBuildingScorch(ctx, decal, downsample) {
  if (!ctx || !Number.isFinite(decal?.x) || !Number.isFinite(decal?.y)) return false;
  if (!Number.isFinite(decal.footprintWidth) || decal.footprintWidth <= 0) return false;
  if (!Number.isFinite(decal.footprintHeight) || decal.footprintHeight <= 0) return false;

  const width = decal.footprintWidth / downsample;
  const height = decal.footprintHeight / downsample;
  const x = decal.x / downsample - width / 2;
  const y = decal.y / downsample - height / 2;
  const rng = mulberry32(decal.seed || decal.id || 1);
  const edgeX = clamp(width * (0.09 + rng() * 0.04), 1, width * 0.24);
  const edgeY = clamp(height * (0.09 + rng() * 0.04), 1, height * 0.24);
  const coreAlpha = 0.48 + rng() * 0.08;
  ctx.save();
  stampFeatheredBuildingScorch(ctx, x, y, width, height, edgeX, edgeY, coreAlpha, rng);
  stampBuildingScorchAsh(ctx, x, y, width, height, edgeX, edgeY, decal, rng);
  ctx.restore();
  return true;
}

function stampFeatheredBuildingScorch(ctx, x, y, width, height, edgeX, edgeY, coreAlpha, rng) {
  stampBuildingScorchSootEdge(ctx, x, y, width, height, edgeX, edgeY, rng);
  const coreX = x + edgeX;
  const coreY = y + edgeY;
  const coreWidth = width - edgeX * 2;
  const coreHeight = height - edgeY * 2;
  ctx.fillStyle = rgba(SCORCH_DARK, coreAlpha * 0.86);
  ctx.fillRect(coreX, coreY, coreWidth, coreHeight);
  stampBuildingScorchEdgeBites(ctx, coreX, coreY, coreWidth, coreHeight, rng);
}

function stampBuildingScorchSootEdge(ctx, x, y, width, height, edgeX, edgeY, rng) {
  const fragmentCount = 16 + Math.floor(rng() * 7);
  for (let index = 0; index < fragmentCount; index += 1) {
    const side = Math.floor(rng() * 4);
    const fragmentWidth = Math.max(1, width * (0.025 + rng() * 0.065));
    const fragmentHeight = Math.max(1, height * (0.025 + rng() * 0.065));
    let fragmentX = x + rng() * Math.max(0, width - fragmentWidth);
    let fragmentY = y + rng() * Math.max(0, height - fragmentHeight);
    if (side === 0) fragmentY = y + rng() * edgeY * 1.5;
    else if (side === 1) fragmentX = x + width - fragmentWidth - rng() * edgeX * 1.5;
    else if (side === 2) fragmentY = y + height - fragmentHeight - rng() * edgeY * 1.5;
    else fragmentX = x + rng() * edgeX * 1.5;
    ctx.fillStyle = rgba(SCORCH_DARK, 0.04 + rng() * 0.1);
    ctx.fillRect(fragmentX, fragmentY, fragmentWidth, fragmentHeight);
  }
}

function stampBuildingScorchEdgeBites(ctx, x, y, width, height, rng) {
  const biteCount = 8 + Math.floor(rng() * 5);
  ctx.globalCompositeOperation = "destination-out";
  for (let index = 0; index < biteCount; index += 1) {
    const side = Math.floor(rng() * 4);
    const biteWidth = Math.max(1, width * (0.05 + rng() * 0.08));
    const biteHeight = Math.max(1, height * (0.05 + rng() * 0.08));
    let biteX = x + rng() * Math.max(0, width - biteWidth);
    let biteY = y + rng() * Math.max(0, height - biteHeight);
    if (side === 0) biteY = y;
    else if (side === 1) biteX = x + width - biteWidth;
    else if (side === 2) biteY = y + height - biteHeight;
    else biteX = x;
    ctx.fillStyle = rgba(0x000000, 0.16 + rng() * 0.28);
    ctx.fillRect(biteX, biteY, biteWidth, biteHeight);
  }
  ctx.globalCompositeOperation = "source-over";
}

function stampBuildingScorchAsh(ctx, x, y, width, height, edgeX, edgeY, decal, rng) {
  const innerX = x + edgeX * (1.45 + rng() * 0.25);
  const innerY = y + edgeY * (1.45 + rng() * 0.25);
  const innerWidth = Math.max(1, width - (innerX - x) * 2);
  const innerHeight = Math.max(1, height - (innerY - y) * 2);
  const fragmentCount = 7 + ((decal.variant || 0) % 3);
  for (let index = 0; index < fragmentCount; index += 1) {
    const fragmentWidth = Math.max(1, innerWidth * (0.1 + rng() * 0.15));
    const fragmentHeight = Math.max(1, innerHeight * (0.08 + rng() * 0.14));
    const fragmentX = innerX + rng() * Math.max(0, innerWidth - fragmentWidth);
    const fragmentY = innerY + rng() * Math.max(0, innerHeight - fragmentHeight);
    ctx.fillStyle = rgba(index % 3 === 0 ? SCORCH_DARK : SCORCH_ASH, 0.07 + rng() * 0.1);
    ctx.fillRect(fragmentX, fragmentY, fragmentWidth, fragmentHeight);
  }
}

function stampAuthoredGroundDecal(ctx, decal, atlas, downsample, tintScratch) {
  if (!ctx || !tintScratch || !decal || !Number.isFinite(decal.x) || !Number.isFinite(decal.y)) {
    return false;
  }
  const plan = createGroundDecalStampPlan(decal, { assetCounts: atlasAssetCounts(atlas) });
  if (!plan) return false;
  const x = (decal.x + plan.offsetWorldX) / downsample;
  const y = (decal.y + plan.offsetWorldY) / downsample;

  if (isBlastDecalClass(plan.decalClass)) {
    return stampAuthoredBlast(ctx, plan, atlas, tintScratch, x, y, downsample);
  }

  if (plan.decalClass === DECAL_CLASS_INFANTRY) {
    const mask = atlas.infantry?.[plan.variantIndex];
    if (!mask) return false;
    drawTintedMask(ctx, tintScratch, mask, SCORCH_DARK, x, y, downsample, {
      rotation: plan.rotation,
      scaleX: plan.scale * plan.flipX * 0.78,
      scaleY: plan.scale * plan.flipY * 0.78,
      opacity: plan.shadowOpacity,
    });
    return drawTintedMask(ctx, tintScratch, mask, plan.color, x, y, downsample, {
      rotation: plan.rotation,
      scaleX: plan.scale * plan.flipX,
      scaleY: plan.scale * plan.flipY,
      opacity: plan.opacity,
    });
  }

  const scorch = atlas.vehicleScorch?.[plan.variantIndex];
  if (!scorch) return false;
  const paint = atlas.vehiclePaint?.[plan.paintVariantIndex];
  const bodyScale = vehicleBodyScale(decal.kind);
  const opacityScale = vehicleScorchOpacityScale(decal.kind);
  if (decal.kind === KIND.TANK) {
    drawTintedMask(ctx, tintScratch, scorch, SCORCH_DARK, x, y, downsample, {
      rotation: plan.rotation,
      scaleX: plan.scale * bodyScale.x * 1.14 * plan.flipX,
      scaleY: plan.scale * bodyScale.y * 1.14 * plan.flipY,
      opacity: 0.12,
    });
    drawTintedMask(ctx, tintScratch, scorch, SCORCH_DARK, x, y, downsample, {
      rotation: plan.rotation,
      scaleX: plan.scale * bodyScale.x * 1.07 * plan.flipX,
      scaleY: plan.scale * bodyScale.y * 1.07 * plan.flipY,
      opacity: 0.18,
    });
  }
  drawTintedMask(ctx, tintScratch, scorch, SCORCH_DARK, x, y, downsample, {
    rotation: plan.rotation,
    scaleX: plan.scale * bodyScale.x * plan.flipX,
    scaleY: plan.scale * bodyScale.y * plan.flipY,
    opacity: clamp(plan.scorchOpacity * opacityScale.scorch, 0, 0.86),
  });
  drawTintedMask(ctx, tintScratch, scorch, SCORCH_ASH, x, y, downsample, {
    rotation: plan.rotation,
    scaleX: plan.scale * bodyScale.x * 0.66 * plan.flipX,
    scaleY: plan.scale * bodyScale.y * 0.66 * plan.flipY,
    opacity: clamp(plan.ashOpacity * opacityScale.ash, 0, 0.2),
  });
  if (paint) {
    drawTintedMask(ctx, tintScratch, paint, plan.color, x, y, downsample, {
      rotation: plan.rotation,
      scaleX: plan.scale * bodyScale.x * plan.flipX,
      scaleY: plan.scale * bodyScale.y * plan.flipY,
      opacity: clamp(plan.paintOpacity * opacityScale.paint, 0, 0.32),
    });
  }
  return true;
}

function stampAuthoredBlast(ctx, plan, atlas, tintScratch, x, y, downsample) {
  const masks = plan.decalClass === DECAL_CLASS_MORTAR_BLAST
    ? atlas.mortarBlast
    : atlas.artilleryBlast;
  const mask = masks?.[plan.variantIndex];
  if (!mask) return false;
  const scaleX = plan.scale * plan.flipX;
  const scaleY = plan.scale * plan.flipY;
  drawTintedMask(ctx, tintScratch, mask, BLAST_SOIL, x, y, downsample, {
    rotation: plan.rotation,
    scaleX,
    scaleY,
    opacity: plan.soilOpacity,
  });
  return drawTintedMask(ctx, tintScratch, mask, BLAST_CHAR, x, y, downsample, {
    rotation: plan.rotation,
    scaleX: scaleX * plan.charScale,
    scaleY: scaleY * plan.charScale,
    opacity: plan.charOpacity,
  });
}

function stampProceduralGroundDecal(ctx, decal, downsample = GROUND_DECAL_TEXTURE_WORLD_SCALE) {
  if (!ctx || !decal || !Number.isFinite(decal.x) || !Number.isFinite(decal.y)) return false;
  if (
    decal.decalClass !== DECAL_CLASS_INFANTRY &&
    decal.decalClass !== DECAL_CLASS_SCORCH &&
    !isBlastDecalClass(decal.decalClass)
  ) {
    return false;
  }
  const rng = mulberry32(decal.seed || decal.id || 1);
  const x = decal.x / downsample;
  const y = decal.y / downsample;
  const facing = Number.isFinite(decal.facing) ? decal.facing : seededAngle(rng);
  ctx.save();
  ctx.translate(x, y);
  ctx.rotate(facing);
  const variantScale = 0.92 + rng() * 0.18;
  ctx.scale(variantScale, variantScale);
  if (decal.decalClass === DECAL_CLASS_INFANTRY) {
    stampInfantry(ctx, decal, rng, downsample);
  } else if (decal.decalClass === DECAL_CLASS_SCORCH) {
    stampScorch(ctx, decal, rng, downsample);
  } else {
    stampBlast(ctx, decal, rng, downsample);
  }
  ctx.restore();
  return true;
}

function drawTintedMask(ctx, scratch, mask, color, x, y, downsample, {
  rotation,
  scaleX,
  scaleY,
  opacity,
}) {
  if (!scratch?.ctx || !scratch?.canvas || !mask?.image) return false;
  if (scratch.canvas.width !== mask.width || scratch.canvas.height !== mask.height) {
    scratch.canvas.width = mask.width;
    scratch.canvas.height = mask.height;
    scratch.ctx.imageSmoothingEnabled = false;
  }
  const scratchCtx = scratch.ctx;
  scratchCtx.globalAlpha = 1;
  scratchCtx.globalCompositeOperation = "source-over";
  scratchCtx.clearRect(0, 0, mask.width, mask.height);
  scratchCtx.drawImage(
    mask.image,
    mask.sourceX,
    mask.sourceY,
    mask.width,
    mask.height,
    0,
    0,
    mask.width,
    mask.height,
  );
  scratchCtx.globalCompositeOperation = "source-in";
  scratchCtx.fillStyle = rgba(color, 1);
  scratchCtx.fillRect(0, 0, mask.width, mask.height);
  scratchCtx.globalCompositeOperation = "source-over";

  ctx.save();
  ctx.translate(x, y);
  ctx.rotate(rotation);
  ctx.scale(scaleX, scaleY);
  ctx.globalAlpha = opacity;
  ctx.drawImage(
    scratch.canvas,
    -mask.width / (2 * downsample),
    -mask.height / (2 * downsample),
    mask.width / downsample,
    mask.height / downsample,
  );
  ctx.restore();
  return true;
}

function atlasAssetCounts(atlas) {
  return {
    infantry: atlas?.infantry?.length || 0,
    vehicleScorch: atlas?.vehicleScorch?.length || 0,
    vehiclePaint: atlas?.vehiclePaint?.length || 0,
    mortarBlast: atlas?.mortarBlast?.length || 0,
    artilleryBlast: atlas?.artilleryBlast?.length || 0,
  };
}

function vehicleBodyScale(kind) {
  const stat = STATS[kind] || {};
  const body = stat.body || {};
  const length = Math.max(22, body.length || (stat.size || 16) * 2.4);
  const width = Math.max(12, body.width || (stat.size || 16) * 1.25);
  if (kind === KIND.TANK) {
    return {
      x: TANK_SCORCH_SCALE_X,
      y: TANK_SCORCH_SCALE_Y,
    };
  }
  return {
    x: clamp(length / VEHICLE_SCORCH_MASK_LENGTH, 0.56, 1.08),
    y: clamp(width / VEHICLE_SCORCH_MASK_WIDTH, 0.48, 0.98),
  };
}

function vehicleScorchOpacityScale(kind) {
  if (kind === KIND.TANK) {
    return {
      scorch: TANK_SCORCH_OPACITY_SCALE,
      ash: TANK_ASH_OPACITY_SCALE,
      paint: TANK_PAINT_OPACITY_SCALE,
    };
  }
  return {
    scorch: 1,
    ash: 1,
    paint: 1,
  };
}

function stampInfantry(ctx, decal, rng, downsample) {
  const color = normalizeColorNumber(decal.color);
  const blobs = 3 + (decal.variant % 3);
  for (let i = 0; i < blobs; i += 1) {
    const angle = seededAngle(rng);
    const dist = (1.5 + rng() * 7) / downsample;
    const rx = (5 + rng() * 7) / downsample;
    const ry = (3 + rng() * 5) / downsample;
    const x = Math.cos(angle) * dist;
    const y = Math.sin(angle) * dist * 0.7;
    drawEllipse(ctx, x, y, rx, ry, seededAngle(rng), rgba(color, 0.28 + rng() * 0.18));
  }
  drawEllipse(ctx, 0, 0, 6 / downsample, 3 / downsample, seededAngle(rng), rgba(0x120908, 0.22));
}

function stampBlast(ctx, decal, rng, downsample) {
  const artillery = decal.decalClass === DECAL_CLASS_ARTILLERY_BLAST;
  const defaultRadius = artillery ? ARTILLERY_BLAST_RADIUS_WORLD : MORTAR_BLAST_RADIUS_WORLD;
  const worldRadius = Number.isFinite(decal.radiusWorld) && decal.radiusWorld > 0
    ? decal.radiusWorld
    : defaultRadius;
  const radius = worldRadius / downsample;
  const coreRadius = radius * (artillery ? 0.32 : 0.2);
  const rayCount = artillery ? 14 : 9;
  const maxRay = radius * (artillery ? 1 : 1.02);

  for (let i = 0; i < rayCount; i += 1) {
    const angle = (i / rayCount) * Math.PI * 2 + (rng() - 0.5) * 0.26;
    const length = maxRay * (0.6 + rng() * 0.4);
    const start = coreRadius * (0.62 + rng() * 0.28);
    const rootWidth = coreRadius * (0.32 + rng() * 0.22);
    const tipWidth = Math.max(0.4, rootWidth * (0.16 + rng() * 0.16));
    const axisX = Math.cos(angle);
    const axisY = Math.sin(angle);
    const normalX = -axisY;
    const normalY = axisX;
    ctx.fillStyle = rgba(BLAST_SOIL, artillery ? 0.32 : 0.3);
    fillPolygon(ctx, [
      [axisX * start + normalX * rootWidth, axisY * start + normalY * rootWidth],
      [axisX * length + normalX * tipWidth, axisY * length + normalY * tipWidth],
      [axisX * length * (0.88 + rng() * 0.08), axisY * length * (0.88 + rng() * 0.08)],
      [axisX * start - normalX * rootWidth, axisY * start - normalY * rootWidth],
    ]);
    if (i % 3 !== 1) {
      const charLength = length * (0.48 + rng() * 0.24);
      ctx.fillStyle = rgba(BLAST_CHAR, artillery ? 0.5 : 0.46);
      fillPolygon(ctx, [
        [axisX * (start * 0.45) + normalX * rootWidth * 0.42, axisY * (start * 0.45) + normalY * rootWidth * 0.42],
        [axisX * charLength + normalX * tipWidth * 0.45, axisY * charLength + normalY * tipWidth * 0.45],
        [axisX * charLength - normalX * tipWidth * 0.45, axisY * charLength - normalY * tipWidth * 0.45],
        [axisX * (start * 0.45) - normalX * rootWidth * 0.42, axisY * (start * 0.45) - normalY * rootWidth * 0.42],
      ]);
    }
  }

  ctx.fillStyle = rgba(BLAST_SOIL, 0.4);
  fillIrregularBlast(ctx, coreRadius * 1.12, artillery ? 15 : 12, rng, 0.77, 1.11);
  ctx.fillStyle = rgba(BLAST_CHAR, artillery ? 0.54 : 0.5);
  fillIrregularBlast(ctx, coreRadius * 0.8, artillery ? 13 : 10, rng, 0.74, 1.08);
  ctx.fillStyle = rgba(0x0a0a08, 0.42);
  fillIrregularBlast(ctx, coreRadius * 0.42, artillery ? 10 : 8, rng, 0.73, 1.06);

  const flecks = artillery ? 19 : 11;
  for (let i = 0; i < flecks; i += 1) {
    const angle = rng() * Math.PI * 2;
    const distance = coreRadius * (1.12 + rng() * (artillery ? 1.85 : 1.3));
    ctx.fillStyle = i % 3 === 0 ? rgba(BLAST_CHAR, 0.46) : rgba(BLAST_SOIL, 0.3);
    ctx.beginPath();
    ctx.arc(
      Math.cos(angle) * distance,
      Math.sin(angle) * distance,
      0.25 + rng() * (artillery ? 0.62 : 0.42),
      0,
      Math.PI * 2,
    );
    ctx.fill();
  }
}

function fillIrregularBlast(ctx, radius, points, rng, minScale, maxScale) {
  const vertices = [];
  for (let i = 0; i < points; i += 1) {
    const angle = (i / points) * Math.PI * 2;
    const distance = radius * (minScale + rng() * (maxScale - minScale));
    vertices.push([Math.cos(angle) * distance, Math.sin(angle) * distance]);
  }
  fillPolygon(ctx, vertices);
}

function fillPolygon(ctx, points) {
  if (!Array.isArray(points) || points.length === 0) return;
  ctx.beginPath();
  ctx.moveTo(points[0][0], points[0][1]);
  for (let i = 1; i < points.length; i += 1) ctx.lineTo(points[i][0], points[i][1]);
  ctx.closePath();
  ctx.fill();
}

function stampScorch(ctx, decal, rng, downsample) {
  const color = normalizeColorNumber(decal.color);
  const stat = STATS[decal.kind] || {};
  const body = stat.body || {};
  const isTank = decal.kind === KIND.TANK;
  const lengthScale = isTank ? 1.65 : 1;
  const widthScale = isTank ? 1.38 : 1;
  const length = Math.max(22, body.length || (stat.size || 16) * 2.4) * lengthScale / downsample;
  const width = Math.max(12, body.width || (stat.size || 16) * 1.25) * widthScale / downsample;
  const char = 0.8 + rng() * 0.16;
  if (isTank) {
    ctx.fillStyle = rgba(0x070706, 0.12);
    irregularHullPath(ctx, length * char * 1.14, width * 1.14 * (0.9 + rng() * 0.18), rng);
    ctx.fill();
    ctx.fillStyle = rgba(0x070706, 0.18);
    irregularHullPath(ctx, length * char * 1.07, width * 1.07 * (0.9 + rng() * 0.18), rng);
    ctx.fill();
  }
  ctx.fillStyle = rgba(0x070706, isTank ? 0.5 : 0.36);
  irregularHullPath(ctx, length * char, width * (0.9 + rng() * 0.18), rng);
  ctx.fill();
  ctx.fillStyle = rgba(0x181816, isTank ? 0.2 : 0.14);
  irregularHullPath(ctx, length * 0.76, width * 0.7, rng);
  ctx.fill();

  const chips = 2 + (decal.variant % 3);
  ctx.fillStyle = rgba(color, isTank ? 0.3 : 0.22);
  for (let i = 0; i < chips; i += 1) {
    const px = (-length * 0.35) + rng() * length * 0.7;
    const py = (-width * 0.32) + rng() * width * 0.64;
    const chipW = Math.max(1, (3 + rng() * 7) / downsample);
    const chipH = Math.max(1, (2 + rng() * 4) / downsample);
    ctx.save();
    ctx.translate(px, py);
    ctx.rotate((rng() - 0.5) * 0.8);
    ctx.fillRect(-chipW / 2, -chipH / 2, chipW, chipH);
    ctx.restore();
  }
}

function irregularHullPath(ctx, length, width, rng) {
  const halfL = length / 2;
  const halfW = width / 2;
  const jitterL = length * 0.08;
  const jitterW = width * 0.12;
  ctx.beginPath();
  ctx.moveTo(-halfL + (rng() - 0.5) * jitterL, -halfW + (rng() - 0.5) * jitterW);
  ctx.lineTo(halfL + (rng() - 0.5) * jitterL, -halfW * 0.85 + (rng() - 0.5) * jitterW);
  ctx.lineTo(halfL * 0.95 + (rng() - 0.5) * jitterL, halfW + (rng() - 0.5) * jitterW);
  ctx.lineTo(-halfL * 0.9 + (rng() - 0.5) * jitterL, halfW * 0.9 + (rng() - 0.5) * jitterW);
  ctx.closePath();
}

function drawEllipse(ctx, x, y, rx, ry, rotation, fillStyle) {
  ctx.fillStyle = fillStyle;
  ctx.beginPath();
  if (typeof ctx.ellipse === "function") {
    ctx.ellipse(x, y, rx, ry, rotation, 0, Math.PI * 2);
  } else {
    ctx.save();
    ctx.translate(x, y);
    ctx.rotate(rotation);
    ctx.scale(rx, ry);
    ctx.arc(0, 0, 1, 0, Math.PI * 2);
    ctx.restore();
  }
  ctx.fill();
}

function updateTexture(texture) {
  if (typeof texture?.update === "function") texture.update();
  else texture?.source?.update?.();
}

function isBlastDecalClass(decalClass) {
  return decalClass === DECAL_CLASS_MORTAR_BLAST || decalClass === DECAL_CLASS_ARTILLERY_BLAST;
}

function seededAngle(rng) {
  return (rng() * 2 - 1) * Math.PI;
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}
