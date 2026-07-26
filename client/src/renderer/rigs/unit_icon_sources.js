import { KIND } from "../../protocol.js";
import { createLiveFrameStrips } from "./frame_strip_routing.js";
import { liveRigIconSvgFor, LOADED_RIFLEMAN_RIG_KEY } from "./live_routing.js";
import { createLivePngRigAtlases } from "./png_routing.js";

const LIVE_FRAME_STRIPS = createLiveFrameStrips();
const LIVE_PNG_ATLASES = createLivePngRigAtlases();
const ICON_FRAME_ZOOM = 1.5;
const ICON_VISIBLE_PADDING_RATIO = 0.08;

/**
 * Return trusted HUD markup backed by the live renderer's preferred production asset.
 * PNG frame strips/atlases win; authored SVG is retained only for units without a PNG route.
 */
export function liveUnitIconMarkupFor(kind, { teamColor = "#0072b2" } = {}) {
  const rigKey = kind === KIND.PANZERFAUST ? LOADED_RIFLEMAN_RIG_KEY : kind;
  const strip = LIVE_FRAME_STRIPS.get(rigKey);
  if (strip) {
    const frameX = strip.frameWidth * (strip.idleFrame || 0);
    return rasterIconMarkup({
      source: "frame-strip",
      image: strip.image,
      sheetWidth: strip.frameWidth * strip.frameCount,
      sheetHeight: strip.frameHeight,
      frame: {
        x: frameX,
        y: 0,
        w: strip.frameWidth,
        h: strip.frameHeight,
      },
      visibleFrame: strip.iconVisibleBounds
        ? {
            x: frameX + strip.iconVisibleBounds.x,
            y: strip.iconVisibleBounds.y,
            w: strip.iconVisibleBounds.w,
            h: strip.iconVisibleBounds.h,
          }
        : null,
      teamTint: !!strip.tintSlot,
      teamColor,
    });
  }

  const atlas = LIVE_PNG_ATLASES.get(rigKey);
  const atlasIcon = atlasPortrait(atlas);
  if (atlasIcon) return rasterIconMarkup({ ...atlasIcon, teamColor });

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

function rasterIconMarkup({
  source,
  image,
  sheetWidth,
  sheetHeight,
  frame,
  visibleFrame = null,
  teamTint = false,
  teamColor = "#0072b2",
}) {
  const safeSheetWidth = positiveDimension(sheetWidth);
  const safeSheetHeight = positiveDimension(sheetHeight);
  const safeFrame = {
    x: finiteNumber(frame?.x),
    y: finiteNumber(frame?.y),
    w: positiveDimension(frame?.w),
    h: positiveDimension(frame?.h),
  };
  if (!image || !safeSheetWidth || !safeSheetHeight || !safeFrame.w || !safeFrame.h) return "";

  const safeVisibleFrame = visibleFrame && {
    x: finiteNumber(visibleFrame.x),
    y: finiteNumber(visibleFrame.y),
    w: positiveDimension(visibleFrame.w),
    h: positiveDimension(visibleFrame.h),
  };
  const viewFrame = safeVisibleFrame?.w && safeVisibleFrame?.h
    ? paddedVisibleFrame(safeFrame, safeVisibleFrame)
    : centeredZoomFrame(safeFrame);
  const tintColor = normalizeTeamColor(teamColor);
  const tintId = `unit-icon-tint-${tintColor.slice(1).toLowerCase()}`;
  const tintFilter = teamTint
    ? `<defs><filter id="${tintId}" color-interpolation-filters="sRGB">` +
        `<feFlood flood-color="${tintColor}" result="teamColor" />` +
        `<feComposite in="teamColor" in2="SourceGraphic" operator="in" result="maskedTeamColor" />` +
        `<feBlend in="SourceGraphic" in2="maskedTeamColor" mode="multiply" />` +
      `</filter></defs>`
    : "";
  const imageFilter = teamTint ? ` filter="url(#${tintId})"` : "";
  return (
    `<svg class="unit-raster-icon${teamTint ? " team-tinted" : ""}" ` +
      `data-unit-icon-source="${source}" aria-hidden="true" focusable="false" ` +
      `viewBox="${number(viewFrame.x)} ${number(viewFrame.y)} ${number(viewFrame.w)} ${number(viewFrame.h)}" ` +
      `preserveAspectRatio="xMidYMid meet" style="overflow:hidden">` +
      tintFilter +
      `<image href="${image}" x="0" y="0" width="${number(safeSheetWidth)}" ` +
        `height="${number(safeSheetHeight)}" preserveAspectRatio="none"${imageFilter} />` +
    `</svg>`
  );
}

function paddedVisibleFrame(frame, visibleFrame) {
  const padX = visibleFrame.w * ICON_VISIBLE_PADDING_RATIO;
  const padY = visibleFrame.h * ICON_VISIBLE_PADDING_RATIO;
  const left = Math.max(frame.x, visibleFrame.x - padX);
  const top = Math.max(frame.y, visibleFrame.y - padY);
  const right = Math.min(frame.x + frame.w, visibleFrame.x + visibleFrame.w + padX);
  const bottom = Math.min(frame.y + frame.h, visibleFrame.y + visibleFrame.h + padY);
  return {
    x: left,
    y: top,
    w: right - left,
    h: bottom - top,
  };
}

function centeredZoomFrame(frame) {
  const w = frame.w / ICON_FRAME_ZOOM;
  const h = frame.h / ICON_FRAME_ZOOM;
  return {
    x: frame.x + (frame.w - w) / 2,
    y: frame.y + (frame.h - h) / 2,
    w,
    h,
  };
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

function normalizeTeamColor(value) {
  const color = String(value || "").trim();
  return /^#[0-9a-fA-F]{6}$/.test(color) ? color : "#0072b2";
}
