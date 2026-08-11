import { KIND } from "../../protocol.js";
import { lightenColor } from "../shared.js";
import { createLiveFrameStrips } from "./frame_strip_routing.js";
import { liveRigIconSvgFor, LOADED_RIFLEMAN_RIG_KEY } from "./live_routing.js";
import { createLivePngRigAtlases } from "./png_routing.js";
import { resolvePngSpriteTransform } from "./png_transform.js";
import { RASTER_RIG_DEFINITIONS } from "./raster_rig_definitions.js";
import { createRigRenderContext, sampleRigAnimation } from "./animation.js";

const LIVE_FRAME_STRIPS = createLiveFrameStrips();
const LIVE_PNG_ATLASES = createLivePngRigAtlases();
const ICON_FRAME_ZOOM = 1.5;
const ICON_VISIBLE_PADDING_RATIO = 0.025;
const ICON_COMPOSITION_STATE = Object.freeze({
  transform: Object.freeze({ x: 0, y: 0, rotation: 0, scaleX: 1, scaleY: 1 }),
  pivot: Object.freeze({ x: 0, y: 0 }),
});

/**
 * Return trusted HUD markup backed by the live renderer's preferred production asset.
 * PNG frame strips/atlases are authoritative for raster units. SVG portraits are used only for
 * units that do not yet have a PNG route.
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

/**
 * Mount a renderer-authored PNG rig icon and drive its articulated parts from sampled rig poses.
 * The caller supplies the recoil envelope so gameplay state and decorative icon cycles can share
 * one authoritative timing model without teaching the icon system about attack events.
 */
export function mountLiveUnitIcon(root, kind, {
  teamColor = "#0072b2",
  periodMs = 2400,
  delayMs = 0,
  sampleCycle = null,
  requestFrame = globalThis.requestAnimationFrame?.bind(globalThis),
  cancelFrame = globalThis.cancelAnimationFrame?.bind(globalThis),
  now = () => globalThis.performance?.now?.() ?? Date.now(),
  reducedMotion = globalThis.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches === true,
} = {}) {
  const atlas = LIVE_PNG_ATLASES.get(kind);
  const definition = RASTER_RIG_DEFINITIONS.get(kind);
  if (!root || !atlas?.image || !definition) {
    if (root) root.innerHTML = liveUnitIconMarkupFor(kind, { teamColor });
    return { destroy() {} };
  }

  const icon = createAnimatedRasterIcon(root, kind, atlas, definition, normalizeTeamColor(teamColor));
  icon.setPose(0, 0);
  if (reducedMotion || typeof requestFrame !== "function" || typeof sampleCycle !== "function") {
    return { destroy() { icon.destroy(); } };
  }

  const startedAt = now();
  let frameId = null;
  let destroyed = false;
  const tick = (frameNow) => {
    if (destroyed) return;
    const cycleNow = Number.isFinite(frameNow) ? frameNow : now();
    const period = Math.max(1, Number(periodMs) || 1);
    const elapsed = ((cycleNow - startedAt - delayMs) % period + period) % period;
    const sample = sampleCycle(elapsed) || {};
    icon.setPose(sample.active ? sample.progress : 0, sample.active ? sample.phase : 0);
    frameId = requestFrame(tick);
  };
  frameId = requestFrame(tick);

  return {
    destroy() {
      if (destroyed) return;
      destroyed = true;
      if (frameId != null) cancelFrame?.(frameId);
      icon.destroy();
    },
  };
}

export function liveUnitIconRigPoseFor(kind, {
  teamColor = "#0072b2",
  recoilProgress = 0,
  recoilPhase = 0,
  facing = 0,
  weaponFacing = facing,
} = {}) {
  const definition = RASTER_RIG_DEFINITIONS.get(kind);
  if (!definition) return null;
  const entity = {
    id: 0,
    kind,
    owner: 1,
    facing,
    weaponFacing,
    teamColor: normalizeTeamColor(teamColor),
    recoilProgress,
    recoilPhase,
  };
  const context = createRigRenderContext(entity);
  return sampleRigAnimation(definition, entity, context);
}

function createAnimatedRasterIcon(root, kind, atlas, definition, teamColor) {
  const tintId = `unit-icon-animated-tint-${++animatedIconSequence}`;
  const spriteEntries = (atlas.sprites || []).filter((sprite) => sprite?.frame);
  const rasterParts = new Set(spriteEntries.map((sprite) => sprite.animationPart));
  const nativeEntries = (definition.parts || []).filter((part) => (
    !rasterParts.has(part.id) && part.id !== "part.shadow" && nativeGeometryMarkup(part.geometry)
  ));
  const ordered = [
    ...spriteEntries.map((sprite) => ({ type: "raster", drawOrder: sprite.drawOrder || 0, value: sprite })),
    ...nativeEntries.map((part) => ({ type: "native", drawOrder: part.drawOrder || 0, value: part })),
  ].sort((a, b) => a.drawOrder - b.drawOrder);
  const viewBox = atlas.viewBox || { x: -40, y: -32, width: 80, height: 64 };
  const tintFilter = `<defs><filter id="${tintId}" color-interpolation-filters="sRGB">` +
    `<feFlood flood-color="${teamColor}" result="teamColor" />` +
    `<feComposite in="teamColor" in2="SourceGraphic" operator="in" result="maskedTeamColor" />` +
    `<feBlend in="SourceGraphic" in2="maskedTeamColor" mode="multiply" />` +
    `</filter></defs>`;
  const layers = ordered.map((entry) => entry.type === "raster"
    ? animatedRasterLayerMarkup(entry.value, atlas, tintId)
    : animatedNativeLayerMarkup(entry.value)).join("");
  root.innerHTML = `<svg class="unit-raster-icon unit-rig-animated-icon" ` +
    `data-unit-icon-source="png-atlas-rig" aria-hidden="true" focusable="false" ` +
    `viewBox="${number(viewBox.x)} ${number(viewBox.y)} ${number(viewBox.width)} ${number(viewBox.height)}" ` +
    `preserveAspectRatio="xMidYMid meet">${tintFilter}${layers}</svg>`;

  const records = [...root.querySelectorAll("[data-unit-icon-animation-part]")].map((node) => ({
    node,
    animationPart: node.getAttribute("data-unit-icon-animation-part"),
    sprite: spriteEntries.find((candidate) => candidate.id === node.getAttribute("data-unit-icon-sprite")) || null,
    scaleNode: node.querySelector("[data-unit-icon-scale]"),
  }));
  let lastProgress = null;
  let lastPhase = null;
  return {
    setPose(recoilProgress, recoilPhase) {
      const progress = Math.max(0, Math.min(1, Number(recoilProgress) || 0));
      const phase = Math.max(0, Math.min(1, Number(recoilPhase) || 0));
      if (progress === lastProgress && phase === lastPhase) return;
      lastProgress = progress;
      lastPhase = phase;
      const sampled = liveUnitIconRigPoseFor(kind, {
        teamColor,
        recoilProgress: progress,
        recoilPhase: phase,
      });
      for (const record of records) applyAnimatedIconPart(record, sampled.parts[record.animationPart]);
    },
    destroy() {
      root.replaceChildren?.();
    },
  };
}

let animatedIconSequence = 0;

function animatedRasterLayerMarkup(sprite, atlas, tintId) {
  const frame = sprite.frame;
  const pixelsPerUnitX = frame.pixelsPerUnitX || frame.pixelsPerUnit || 1;
  const pixelsPerUnitY = frame.pixelsPerUnitY || frame.pixelsPerUnit || 1;
  const w = frame.w / Math.abs(pixelsPerUnitX);
  const h = frame.h / Math.abs(pixelsPerUnitY);
  const x = Math.min(
    -finiteNumber(frame.originX) / pixelsPerUnitX,
    (frame.w - finiteNumber(frame.originX)) / pixelsPerUnitX,
  );
  const y = Math.min(
    -finiteNumber(frame.originY) / pixelsPerUnitY,
    (frame.h - finiteNumber(frame.originY)) / pixelsPerUnitY,
  );
  const filter = sprite.tintSlot === "fixed" ? "" : ` filter="url(#${tintId})"`;
  return `<g data-unit-icon-animation-part="${sprite.animationPart}" data-unit-icon-sprite="${sprite.id}">` +
    `<g data-unit-icon-scale>` +
      `<svg x="${number(x)}" y="${number(y)}" width="${number(w)}" height="${number(h)}" ` +
        `viewBox="${frame.x} ${frame.y} ${frame.w} ${frame.h}" preserveAspectRatio="none" overflow="hidden">` +
        `<image href="${atlas.image}" x="0" y="0" width="${atlas.grid.width}" height="${atlas.grid.height}" ` +
          `preserveAspectRatio="none"${filter} />` +
      `</svg>` +
    `</g>` +
  `</g>`;
}

function animatedNativeLayerMarkup(part) {
  return `<g data-unit-icon-animation-part="${part.id}" opacity="0">` +
    `<g data-unit-icon-scale>${nativeGeometryMarkup(part.geometry, part.paint)}</g>` +
  `</g>`;
}

function nativeGeometryMarkup(geometry, paint = {}) {
  if (!geometry) return "";
  const attrs = `fill="${paint.fill || "none"}" fill-opacity="${number(paint.fillOpacity ?? 1)}" ` +
    `stroke="${paint.stroke || "none"}" stroke-width="${number(paint.strokeWidth || 0)}" ` +
    `stroke-opacity="${number(paint.strokeOpacity ?? 1)}"`;
  if (geometry.type === "circle") {
    return `<circle cx="${number(geometry.cx)}" cy="${number(geometry.cy)}" r="${number(geometry.r)}" ${attrs} />`;
  }
  if (geometry.type === "polygon") {
    const points = (geometry.points || []).map((point) => `${number(point.x)},${number(point.y)}`).join(" ");
    return points ? `<polygon points="${points}" ${attrs} />` : "";
  }
  return "";
}

function applyAnimatedIconPart(record, state) {
  if (!state) return;
  let transform;
  let scaleX;
  let scaleY;
  if (record.sprite) {
    const resolved = resolvePngSpriteTransform(state, record.sprite.frame, record.sprite);
    transform = `translate(${number(resolved.x)} ${number(resolved.y)}) rotate(${number(resolved.rotation * 180 / Math.PI)})`;
    scaleX = resolved.scaleX * Math.abs(record.sprite.frame.pixelsPerUnitX || record.sprite.frame.pixelsPerUnit || 1);
    scaleY = resolved.scaleY * Math.abs(record.sprite.frame.pixelsPerUnitY || record.sprite.frame.pixelsPerUnit || 1);
  } else {
    transform = `translate(${number(state.transform.x + state.localOffset.x)} ${number(state.transform.y + state.localOffset.y)}) ` +
      `rotate(${number(state.transform.rotation * 180 / Math.PI)})`;
    scaleX = state.transform.scaleX * state.geometryScale.x;
    scaleY = state.transform.scaleY * state.geometryScale.y;
  }
  record.node.setAttribute("transform", transform);
  record.node.setAttribute("opacity", number(state.alpha));
  record.scaleNode?.setAttribute("transform", `scale(${number(scaleX)} ${number(scaleY)})`);
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
    atlas.iconImage &&
    Number.isInteger(atlas.grid.columns) &&
    Number.isInteger(atlas.grid.rows)
  ) {
    const frameWidth = Math.ceil(atlas.grid.width / atlas.grid.columns);
    const frameHeight = Math.ceil(atlas.grid.height / atlas.grid.rows);
    return {
      source: "png-atlas-reference",
      image: atlas.iconImage,
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
      teamTint: composition.some((component) => component.teamTint),
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
    const part = {
      ...sprite,
      positionOffsetX: optionalFiniteNumber(descriptor.x) ?? sprite.positionOffsetX,
      positionOffsetY: optionalFiniteNumber(descriptor.y) ?? sprite.positionOffsetY,
      rotationOffset: optionalFiniteNumber(descriptor.rotation) ?? sprite.rotationOffset,
      rotationPivotX: optionalFiniteNumber(descriptor.pivotX) ?? sprite.rotationPivotX,
      rotationPivotY: optionalFiniteNumber(descriptor.pivotY) ?? sprite.rotationPivotY,
      rotationPivotReferenceOffset:
        optionalFiniteNumber(descriptor.pivotReferenceRotation)
        ?? sprite.rotationPivotReferenceOffset,
    };
    const transform = resolvePngSpriteTransform(ICON_COMPOSITION_STATE, sprite.frame, part);
    components.push({
      id: sprite.id,
      frame: sprite.frame,
      x: transform.x,
      y: transform.y,
      rotation: transform.rotation,
      pivotX: transform.pivotX,
      pivotY: transform.pivotY,
      teamTint: sprite.tintSlot !== "fixed",
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
  const layers = safeComponents.map((component) => {
    const frame = component.frame;
    const rotation = component.rotation * 180 / Math.PI;
    const layerFilter = component.teamTint ? ` filter="url(#${tintId})"` : "";
    return (
      `<g data-unit-icon-component="${component.id}" ` +
        `transform="translate(${number(component.x)} ${number(component.y)}) rotate(${number(rotation)})"${layerFilter}>` +
        `<g transform="scale(${component.scaleX} ${component.scaleY})">` +
          `<svg x="${number(component.renderX)}" y="${number(component.renderY)}" ` +
            `width="${number(component.w)}" height="${number(component.h)}" ` +
            `viewBox="${number(frame.x)} ${number(frame.y)} ${number(frame.w)} ${number(frame.h)}" ` +
            `preserveAspectRatio="none" style="overflow:hidden">` +
            `<image href="${image}" x="0" y="0" width="${number(safeSheetWidth)}" ` +
              `height="${number(safeSheetHeight)}" preserveAspectRatio="none" />` +
          `</svg>` +
        `</g>` +
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
      `<g>${layers}</g>` +
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
    pixelsPerUnitX === 0 ||
    pixelsPerUnitY === 0
  ) {
    return null;
  }
  const scaleX = Math.sign(pixelsPerUnitX);
  const scaleY = Math.sign(pixelsPerUnitY);
  const w = frameWidth / Math.abs(pixelsPerUnitX);
  const h = frameHeight / Math.abs(pixelsPerUnitY);
  const defaultPivotX = finiteNumber(frame.originX);
  const defaultPivotY = finiteNumber(frame.originY);
  const pivotX = optionalFiniteNumber(component.pivotX) ?? defaultPivotX;
  const pivotY = optionalFiniteNumber(component.pivotY) ?? defaultPivotY;
  const originWorldX = -pivotX / pixelsPerUnitX;
  const farWorldX = (frameWidth - pivotX) / pixelsPerUnitX;
  const originWorldY = -pivotY / pixelsPerUnitY;
  const farWorldY = (frameHeight - pivotY) / pixelsPerUnitY;
  const localX = Math.min(originWorldX, farWorldX);
  const localY = Math.min(originWorldY, farWorldY);
  return {
    id: component.id,
    teamTint: component.teamTint === true,
    frame: {
      x: finiteNumber(frame.x),
      y: finiteNumber(frame.y),
      w: frameWidth,
      h: frameHeight,
    },
    x: finiteNumber(component.x),
    y: finiteNumber(component.y),
    rotation: finiteNumber(component.rotation),
    localX,
    localY,
    renderX: scaleX < 0 ? -(localX + w) : localX,
    renderY: scaleY < 0 ? -(localY + h) : localY,
    scaleX,
    scaleY,
    w,
    h,
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

function optionalFiniteNumber(value) {
  if (value === null || value === undefined || value === "") return null;
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
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
