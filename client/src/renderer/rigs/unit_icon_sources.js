import { KIND } from "../../protocol.js";
import { createLiveFrameStrips } from "./frame_strip_routing.js";
import { liveRigIconSvgFor, LOADED_RIFLEMAN_RIG_KEY } from "./live_routing.js";
import { createLivePngRigAtlases } from "./png_routing.js";

const LIVE_FRAME_STRIPS = createLiveFrameStrips();
const LIVE_PNG_ATLASES = createLivePngRigAtlases();
const ICON_FRAME_ZOOM = 1.5;

/**
 * Return trusted HUD markup backed by the live renderer's preferred production asset.
 * PNG frame strips/atlases win; authored SVG is retained only for units without a PNG route.
 */
export function liveUnitIconMarkupFor(kind) {
  const rigKey = kind === KIND.PANZERFAUST ? LOADED_RIFLEMAN_RIG_KEY : kind;
  const strip = LIVE_FRAME_STRIPS.get(rigKey);
  if (strip) {
    return rasterIconMarkup({
      source: "frame-strip",
      image: strip.image,
      sheetWidth: strip.frameWidth * strip.frameCount,
      sheetHeight: strip.frameHeight,
      frame: {
        x: strip.frameWidth * (strip.idleFrame || 0),
        y: 0,
        w: strip.frameWidth,
        h: strip.frameHeight,
      },
      teamTint: !!strip.tintSlot,
    });
  }

  const atlas = LIVE_PNG_ATLASES.get(rigKey);
  const atlasIcon = atlasPortrait(atlas);
  if (atlasIcon) return rasterIconMarkup(atlasIcon);

  return liveRigIconSvgFor(kind);
}

function atlasPortrait(atlas) {
  if (!atlas?.image || !atlas?.grid) return null;
  const assembled = atlas.grid.components?.assembledReference;
  if (assembled) {
    return {
      source: "png-atlas-reference",
      image: atlas.image,
      sheetWidth: atlas.grid.width,
      sheetHeight: atlas.grid.height,
      frame: assembled,
      teamTint: true,
    };
  }

  const referenceIndex = atlas.grid.cells?.indexOf?.("reference.full") ?? -1;
  if (
    referenceIndex >= 0 &&
    Number.isInteger(atlas.grid.columns) &&
    Number.isInteger(atlas.grid.rows)
  ) {
    const frameWidth = Math.ceil(atlas.grid.width / atlas.grid.columns);
    const frameHeight = Math.ceil(atlas.grid.height / atlas.grid.rows);
    return {
      source: "png-atlas-reference",
      image: sourceAssetUrl(atlas.grid.sourceSheet, atlas.grid.imageVersion),
      sheetWidth: atlas.grid.width,
      sheetHeight: atlas.grid.height,
      frame: {
        x: (referenceIndex % atlas.grid.columns) * frameWidth,
        y: Math.floor(referenceIndex / atlas.grid.columns) * frameHeight,
        w: frameWidth,
        h: frameHeight,
      },
      teamTint: true,
    };
  }

  const sprite = representativeAtlasSprite(atlas.sprites);
  const frame = sprite?.frame?.visibleBounds || sprite?.frame;
  if (!frame) return null;
  return {
    source: "png-atlas-component",
    image: atlas.image,
    sheetWidth: atlas.grid.width,
    sheetHeight: atlas.grid.height,
    frame,
    teamTint: sprite.tintSlot !== "fixed",
  };
}

function representativeAtlasSprite(sprites) {
  if (!Array.isArray(sprites)) return null;
  const preferences = [
    (sprite) => sprite.id === "sprite.body",
    (sprite) => sprite.id?.includes("barrelAssembly.deployed"),
    (sprite) => sprite.id?.includes("carriage.deployed"),
  ];
  for (const preferred of preferences) {
    const sprite = sprites.find(preferred);
    if (sprite) return sprite;
  }
  return sprites[0] || null;
}

function sourceAssetUrl(sourcePath, version = "") {
  if (!sourcePath) return "";
  const path = sourcePath.startsWith("client/") ? sourcePath.slice("client".length) : sourcePath;
  return version ? `${path}?v=${encodeURIComponent(version)}` : path;
}

function rasterIconMarkup({ source, image, sheetWidth, sheetHeight, frame, teamTint = false }) {
  const safeSheetWidth = positiveDimension(sheetWidth);
  const safeSheetHeight = positiveDimension(sheetHeight);
  const safeFrame = {
    x: finiteNumber(frame?.x),
    y: finiteNumber(frame?.y),
    w: positiveDimension(frame?.w),
    h: positiveDimension(frame?.h),
  };
  if (!image || !safeSheetWidth || !safeSheetHeight || !safeFrame.w || !safeFrame.h) return "";

  const viewWidth = safeFrame.w / ICON_FRAME_ZOOM;
  const viewHeight = safeFrame.h / ICON_FRAME_ZOOM;
  const viewX = safeFrame.x + (safeFrame.w - viewWidth) / 2;
  const viewY = safeFrame.y + (safeFrame.h - viewHeight) / 2;
  return (
    `<svg class="unit-raster-icon${teamTint ? " team-tinted" : ""}" ` +
      `data-unit-icon-source="${source}" aria-hidden="true" focusable="false" ` +
      `viewBox="${number(viewX)} ${number(viewY)} ${number(viewWidth)} ${number(viewHeight)}" ` +
      `preserveAspectRatio="xMidYMid meet">` +
      `<image href="${image}" x="0" y="0" width="${number(safeSheetWidth)}" ` +
        `height="${number(safeSheetHeight)}" preserveAspectRatio="none" />` +
    `</svg>`
  );
}

function positiveDimension(value) {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : 0;
}

function finiteNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : 0;
}

function number(value) {
  return Number(value.toFixed(4));
}
