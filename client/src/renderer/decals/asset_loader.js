import { fetchImageBitmap } from "../raster_primitives.js";
import { GROUND_DECAL_PNG_ATLAS } from "./atlas.generated.js";
import { GROUND_DECAL_ASSET_MANIFEST } from "./manifest.js";

export const GROUND_DECAL_ATLAS_STATUS = Object.freeze({
  IDLE: "idle",
  PENDING: "pending",
  READY: "ready",
  FAILED: "failed",
});

export function canLoadGroundDecalAtlas({
  fetchFn = globalThis.fetch,
  createImageBitmapFn = globalThis.createImageBitmap,
} = {}) {
  return typeof fetchFn === "function" && typeof createImageBitmapFn === "function";
}

export async function loadGroundDecalAtlas({
  manifest = GROUND_DECAL_ASSET_MANIFEST,
  atlasManifest = GROUND_DECAL_PNG_ATLAS,
  fetchFn = globalThis.fetch,
  createImageBitmapFn = globalThis.createImageBitmap,
} = {}) {
  if (!canLoadGroundDecalAtlas({ fetchFn, createImageBitmapFn })) {
    throw new Error("ground decal PNG atlas needs fetch and createImageBitmap");
  }
  validateAtlasCoverage(manifest, atlasManifest);
  const image = await fetchImageBitmap(atlasManifest.url, { fetchFn, createImageBitmapFn });
  if (image.width !== atlasManifest.width || image.height !== atlasManifest.height) {
    image.close?.();
    throw new Error(`ground decal PNG atlas dimensions ${image.width}x${image.height} do not match manifest`);
  }
  let destroyed = false;
  const atlas = {
    infantry: masksForGroup(atlasManifest.groups.infantry, image),
    vehicleScorch: masksForGroup(atlasManifest.groups.vehicleScorch, image),
    vehiclePaint: masksForGroup(atlasManifest.groups.vehiclePaint, image),
    mortarBlast: masksForGroup(atlasManifest.groups.mortarBlast, image),
    artilleryBlast: masksForGroup(atlasManifest.groups.artilleryBlast, image),
    destroy() {
      if (destroyed) return;
      destroyed = true;
      image.close?.();
      this.infantry = [];
      this.vehicleScorch = [];
      this.vehiclePaint = [];
      this.mortarBlast = [];
      this.artilleryBlast = [];
    },
  };
  return atlas;
}

export function validateAtlasCoverage(sourceManifest, atlasManifest) {
  if (atlasManifest?.version !== 1) throw new TypeError("ground decal PNG atlas manifest version is unsupported");
  for (const [group, assets] of Object.entries(sourceManifest || {})) {
    const rects = atlasManifest.groups?.[group];
    if (!Array.isArray(rects) || rects.length !== assets.length) {
      throw new Error(`ground decal PNG atlas group ${group} does not cover its SVG sources`);
    }
    for (let index = 0; index < assets.length; index += 1) {
      const source = assets[index];
      const rect = rects[index];
      if (rect.id !== source.id || rect.width !== source.width || rect.height !== source.height) {
        throw new Error(`ground decal PNG atlas entry ${group}[${index}] does not match ${source.id}`);
      }
      if (
        !Number.isInteger(rect.x) || !Number.isInteger(rect.y) || rect.x < 0 || rect.y < 0
        || rect.x + rect.width > atlasManifest.width || rect.y + rect.height > atlasManifest.height
      ) throw new RangeError(`ground decal PNG atlas rect ${source.id} is out of bounds`);
    }
  }
  return true;
}

function masksForGroup(rects, image) {
  return rects.map((rect) => Object.freeze({
    id: rect.id,
    width: rect.width,
    height: rect.height,
    sourceX: rect.x,
    sourceY: rect.y,
    image,
  }));
}
