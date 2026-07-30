import { KIND } from "../../protocol.js";
import { lightenColor } from "../shared.js";
import { createLiveFrameStrips } from "./frame_strip_routing.js";
import { liveRigIconSvgFor, LOADED_RIFLEMAN_RIG_KEY } from "./live_routing.js";
import { createLivePngRigAtlases } from "./png_routing.js";

const LIVE_FRAME_STRIPS = createLiveFrameStrips();
const LIVE_PNG_ATLASES = createLivePngRigAtlases();
const ICON_FRAME_ZOOM = 1.5;
const ICON_VISIBLE_PADDING_RATIO = 0.025;

/**
 * Return trusted HUD markup backed by the live renderer's preferred production asset.
 * PNG frame strips/atlases win; authored SVG is retained only for units without a PNG route.
 */
export function liveUnitIconMarkupFor(kind, { teamColor = "#0072b2" } = {}) {
  const tintColor = normalizeTeamColor(teamColor);
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
      teamColor: tintColor,
    });
  }

  const atlas = LIVE_PNG_ATLASES.get(rigKey);
  const atlasIcon = atlasPortrait(atlas, tintColor);
  if (atlasIcon?.components) {
    return composedRasterIconMarkup({ ...atlasIcon, teamColor: tintColor });
  }
  if (atlasIcon) return rasterIconMarkup({ ...atlasIcon, teamColor: tintColor });

  return tintRigIconMarkup(liveRigIconSvgFor(kind), tintColor);
}

function atlasPortrait(atlas, teamColor) {
  if (!atlas?.image || !atlas?.grid) return null;
  const assembled = atlas.grid.components?.assembledReference;
  if (assembled) {
    return {
      source: "png-atlas-reference",
      image: atlas.image,
      sheetWidth: atlas.grid.width,
      sheetHeight: atlas.grid.height,
      frame: assembled,
      visibleFrame: atlas.iconVisibleBounds || assembled,
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
      visibleFrame: atlas.iconVisibleBounds || {
        x: (referenceIndex % atlas.grid.columns) * frameWidth,
        y: Math.floor(referenceIndex / atlas.grid.columns) * frameHeight,
        w: frameWidth,
        h: frameHeight,
      },
      teamTint: true,
    };
  }

  const composition = atlasComposition(atlas);
  if (composition) {
    return {
      source: "png-atlas-composition",
      image: atlas.image,
      sheetWidth: atlas.grid.width,
      sheetHeight: atlas.grid.height,
      components: composition,
      teamTint: true,
    };
  }

  // A component atlas is not necessarily a portrait atlas. Only use a component when it is the
  // complete unit body; selecting a barrel or carriage merely because it is large produces a
  // misleading icon. Component-only atlases must opt into an explicit icon composition.
  const sprite = atlas.sprites?.find?.((candidate) => candidate.id === "sprite.body");
  const baseFrame = sprite?.frame?.visibleBounds || sprite?.frame;
  const paletteFrame = sprite?.paletteFrames?.[teamColor];
  const frame = paletteFrame?.visibleBounds || paletteFrame || baseFrame;
  if (!frame) return null;
  const visibleFrame = translatedVisibleFrame(atlas.iconVisibleBounds, baseFrame, frame) || frame;
  return {
    source: "png-atlas-component",
    image: atlas.image,
    sheetWidth: atlas.grid.width,
    sheetHeight: atlas.grid.height,
    frame,
    visibleFrame,
    teamTint: sprite.tintSlot !== "fixed",
  };
}

function atlasComposition(atlas) {
  const spriteIds = atlas?.iconComposition?.sprites;
  if (!Array.isArray(spriteIds) || spriteIds.length === 0) return null;
  const spritesById = new Map(
    (atlas.sprites || []).filter((sprite) => sprite?.id && sprite?.frame)
      .map((sprite) => [sprite.id, sprite]),
  );
  const components = [];
  for (const entry of spriteIds) {
    const descriptor = typeof entry === "string" ? { spriteId: entry } : entry;
    const sprite = spritesById.get(descriptor?.spriteId);
    if (!sprite) return null;
    components.push({
      id: sprite.id,
      frame: sprite.frame,
      x: finiteNumber(descriptor.x),
      y: finiteNumber(descriptor.y),
      rotation: Number.isFinite(Number(descriptor.rotation))
        ? Number(descriptor.rotation)
        : finiteNumber(sprite.rotationOffset),
    });
  }
  return components;
}

function translatedVisibleFrame(visibleFrame, baseFrame, selectedFrame) {
  if (!visibleFrame || !baseFrame || !selectedFrame) return null;
  return {
    ...visibleFrame,
    x: visibleFrame.x + finiteNumber(selectedFrame.x) - finiteNumber(baseFrame.x),
    y: visibleFrame.y + finiteNumber(selectedFrame.y) - finiteNumber(baseFrame.y),
  };
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
      `width="${number(viewFrame.w)}" height="${number(viewFrame.h)}" ` +
      `viewBox="${number(viewFrame.x)} ${number(viewFrame.y)} ${number(viewFrame.w)} ${number(viewFrame.h)}" ` +
      `preserveAspectRatio="xMidYMid meet" style="overflow:hidden">` +
      tintFilter +
      `<image href="${image}" x="0" y="0" width="${number(safeSheetWidth)}" ` +
        `height="${number(safeSheetHeight)}" preserveAspectRatio="none"${imageFilter} />` +
    `</svg>`
  );
}

function composedRasterIconMarkup({
  source,
  image,
  sheetWidth,
  sheetHeight,
  components,
  teamTint = false,
  teamColor = "#0072b2",
}) {
  const safeSheetWidth = positiveDimension(sheetWidth);
  const safeSheetHeight = positiveDimension(sheetHeight);
  if (!image || !safeSheetWidth || !safeSheetHeight || !Array.isArray(components)) return "";

  const safeComponents = components.map((component) => normalizedCompositionComponent(component))
    .filter(Boolean);
  if (safeComponents.length !== components.length || safeComponents.length === 0) return "";
  const bounds = compositionBounds(safeComponents);
  if (!bounds) return "";
  const padX = bounds.w * ICON_VISIBLE_PADDING_RATIO;
  const padY = bounds.h * ICON_VISIBLE_PADDING_RATIO;
  const viewFrame = {
    x: bounds.x - padX,
    y: bounds.y - padY,
    w: bounds.w + padX * 2,
    h: bounds.h + padY * 2,
  };
  const tintColor = normalizeTeamColor(teamColor);
  const tintId = `unit-icon-tint-${tintColor.slice(1).toLowerCase()}`;
  const tintFilter = teamTint
    ? `<defs><filter id="${tintId}" color-interpolation-filters="sRGB">` +
        `<feFlood flood-color="${tintColor}" result="teamColor" />` +
        `<feComposite in="teamColor" in2="SourceGraphic" operator="in" result="maskedTeamColor" />` +
        `<feBlend in="SourceGraphic" in2="maskedTeamColor" mode="multiply" />` +
      `</filter></defs>`
    : "";
  const groupFilter = teamTint ? ` filter="url(#${tintId})"` : "";
  const layers = safeComponents.map((component) => {
    const frame = component.frame;
    const rotation = component.rotation * 180 / Math.PI;
    return (
      `<g data-unit-icon-component="${component.id}" ` +
        `transform="translate(${number(component.x)} ${number(component.y)}) rotate(${number(rotation)})">` +
        `<svg x="${number(component.localX)}" y="${number(component.localY)}" ` +
          `width="${number(component.w)}" height="${number(component.h)}" ` +
          `viewBox="${number(frame.x)} ${number(frame.y)} ${number(frame.w)} ${number(frame.h)}" ` +
          `preserveAspectRatio="none" style="overflow:hidden">` +
          `<image href="${image}" x="0" y="0" width="${number(safeSheetWidth)}" ` +
            `height="${number(safeSheetHeight)}" preserveAspectRatio="none" />` +
        `</svg>` +
      `</g>`
    );
  }).join("");
  return (
    `<svg class="unit-raster-icon${teamTint ? " team-tinted" : ""}" ` +
      `data-unit-icon-source="${source}" aria-hidden="true" focusable="false" ` +
      `width="${number(viewFrame.w)}" height="${number(viewFrame.h)}" ` +
      `viewBox="${number(viewFrame.x)} ${number(viewFrame.y)} ${number(viewFrame.w)} ${number(viewFrame.h)}" ` +
      `preserveAspectRatio="xMidYMid meet" style="overflow:hidden">` +
      tintFilter +
      `<g${groupFilter}>${layers}</g>` +
    `</svg>`
  );
}

function normalizedCompositionComponent(component) {
  const frame = component?.frame;
  const pixelsPerUnitX = Number(frame?.pixelsPerUnitX ?? frame?.pixelsPerUnit);
  const pixelsPerUnitY = Number(frame?.pixelsPerUnitY ?? frame?.pixelsPerUnit);
  const frameWidth = positiveDimension(frame?.w);
  const frameHeight = positiveDimension(frame?.h);
  if (
    !component?.id ||
    !frameWidth ||
    !frameHeight ||
    !Number.isFinite(pixelsPerUnitX) ||
    !Number.isFinite(pixelsPerUnitY) ||
    pixelsPerUnitX <= 0 ||
    pixelsPerUnitY <= 0
  ) {
    return null;
  }
  return {
    id: component.id,
    frame: {
      x: finiteNumber(frame.x),
      y: finiteNumber(frame.y),
      w: frameWidth,
      h: frameHeight,
    },
    x: finiteNumber(component.x),
    y: finiteNumber(component.y),
    rotation: finiteNumber(component.rotation),
    localX: -finiteNumber(frame.originX) / pixelsPerUnitX,
    localY: -finiteNumber(frame.originY) / pixelsPerUnitY,
    w: frameWidth / pixelsPerUnitX,
    h: frameHeight / pixelsPerUnitY,
  };
}

function compositionBounds(components) {
  const points = [];
  for (const component of components) {
    const cos = Math.cos(component.rotation);
    const sin = Math.sin(component.rotation);
    for (const [x, y] of [
      [component.localX, component.localY],
      [component.localX + component.w, component.localY],
      [component.localX + component.w, component.localY + component.h],
      [component.localX, component.localY + component.h],
    ]) {
      points.push({
        x: component.x + x * cos - y * sin,
        y: component.y + x * sin + y * cos,
      });
    }
  }
  const xs = points.map((point) => point.x);
  const ys = points.map((point) => point.y);
  const left = Math.min(...xs);
  const top = Math.min(...ys);
  const right = Math.max(...xs);
  const bottom = Math.max(...ys);
  if (![left, top, right, bottom].every(Number.isFinite) || right <= left || bottom <= top) {
    return null;
  }
  return { x: left, y: top, w: right - left, h: bottom - top };
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
  return /^#[0-9a-fA-F]{6}$/.test(color) ? color.toLowerCase() : "#0072b2";
}

function tintRigIconMarkup(markup, teamColor) {
  if (!markup) return "";
  const team = Number.parseInt(teamColor.slice(1), 16);
  const colors = {
    team: teamColor,
    "team-light": cssHex(lightenColor(team, 0.12)),
    "team-light-soft": cssHex(lightenColor(team, 0.06)),
    "team-light-strong": cssHex(lightenColor(team, 0.16)),
    "team-light-08": cssHex(lightenColor(team, 0.08)),
    "team-light-10": cssHex(lightenColor(team, 0.10)),
    "team-light-14": cssHex(lightenColor(team, 0.14)),
    "team-light-24": cssHex(lightenColor(team, 0.24)),
  };
  return markup
    .split("\n")
    .map((line) => {
      const slot = line.match(/\sdata-rts-tint="([^"]+)"/)?.[1];
      if (!slot) return line;
      if (slot === "team-stroke") return replacePaint(line, "stroke", teamColor);
      if (slot === "team-fill-stroke") {
        return replacePaint(replacePaint(line, "fill", teamColor), "stroke", teamColor);
      }
      return colors[slot] ? replacePaint(line, "fill", colors[slot]) : line;
    })
    .join("\n");
}

function replacePaint(line, attribute, color) {
  const pattern = new RegExp(`\\s${attribute}="[^"]*"`);
  return pattern.test(line) ? line.replace(pattern, ` ${attribute}="${color}"`) : line;
}

function cssHex(color) {
  return `#${color.toString(16).padStart(6, "0")}`;
}
