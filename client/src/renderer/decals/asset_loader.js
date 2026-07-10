import { GROUND_DECAL_ASSET_MANIFEST } from "./manifest.js";

export const GROUND_DECAL_ATLAS_STATUS = Object.freeze({
  IDLE: "idle",
  PENDING: "pending",
  READY: "ready",
  FAILED: "failed",
});

export function canLoadGroundDecalAtlas({
  documentRef = typeof document !== "undefined" ? document : null,
  imageFactory = null,
} = {}) {
  return Boolean(
    documentRef
      && typeof documentRef.createElement === "function"
      && (typeof imageFactory === "function" || typeof globalThis.Image === "function"),
  );
}

export async function loadGroundDecalAtlas({
  manifest = GROUND_DECAL_ASSET_MANIFEST,
  documentRef = typeof document !== "undefined" ? document : null,
  imageFactory = null,
} = {}) {
  if (!canLoadGroundDecalAtlas({ documentRef, imageFactory })) {
    throw new Error("ground decal SVG atlas needs document.createElement and Image");
  }

  const makeImage = imageFactory || (() => new globalThis.Image());
  const atlas = {
    infantry: [],
    vehicleScorch: [],
    vehiclePaint: [],
    mortarBlast: [],
    artilleryBlast: [],
    destroy() {
      destroyMasks(this.infantry);
      destroyMasks(this.vehicleScorch);
      destroyMasks(this.vehiclePaint);
      destroyMasks(this.mortarBlast);
      destroyMasks(this.artilleryBlast);
      this.infantry = [];
      this.vehicleScorch = [];
      this.vehiclePaint = [];
      this.mortarBlast = [];
      this.artilleryBlast = [];
    },
  };

  try {
    atlas.infantry = await loadMaskSet(manifest.infantry, { documentRef, makeImage });
    atlas.vehicleScorch = await loadMaskSet(manifest.vehicleScorch, { documentRef, makeImage });
    atlas.vehiclePaint = await loadMaskSet(manifest.vehiclePaint, { documentRef, makeImage });
    atlas.mortarBlast = await loadMaskSet(manifest.mortarBlast, { documentRef, makeImage });
    atlas.artilleryBlast = await loadMaskSet(manifest.artilleryBlast, { documentRef, makeImage });
    return atlas;
  } catch (err) {
    atlas.destroy();
    throw err;
  }
}

async function loadMaskSet(assets, context) {
  return Promise.all((assets || []).map((asset) => loadMask(asset, context)));
}

async function loadMask(asset, { documentRef, makeImage }) {
  const image = await loadImage(asset, makeImage);
  const canvas = documentRef.createElement("canvas");
  canvas.width = Math.max(1, asset.width | 0);
  canvas.height = Math.max(1, asset.height | 0);
  const ctx = canvas.getContext("2d", { alpha: true });
  if (!ctx) throw new Error(`ground decal mask ${asset.id} could not create a 2d context`);
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(image, 0, 0, canvas.width, canvas.height);
  return {
    id: asset.id,
    url: asset.url,
    width: canvas.width,
    height: canvas.height,
    canvas,
  };
}

function loadImage(asset, makeImage) {
  return new Promise((resolve, reject) => {
    const image = makeImage();
    let settled = false;
    const finish = (fn, value) => {
      if (settled) return;
      settled = true;
      image.onload = null;
      image.onerror = null;
      fn(value);
    };
    image.onload = () => finish(resolve, image);
    image.onerror = () => finish(reject, new Error(`failed to load ground decal SVG ${asset.url}`));
    image.src = asset.url;
    if (image.complete && image.naturalWidth !== 0) finish(resolve, image);
  });
}

function destroyMasks(masks) {
  for (const mask of masks || []) {
    if (mask?.canvas) {
      mask.canvas.width = 0;
      mask.canvas.height = 0;
    }
  }
}
