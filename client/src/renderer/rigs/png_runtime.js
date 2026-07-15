import { hexToInt, lightenColor } from "../shared.js";
import { sampleRigAnimation } from "./animation.js";
import { isImmutablePartSelection, normalizedPartSet, partSelectionKey } from "./part_selection.js";

const OCCUPIED_TRENCH_UNIT_SCALE = 0.85;
const ATLAS_SPRITES_CACHE = new WeakMap();
const ROUTE_COVERAGE_CACHE = new WeakMap();
const ALL_ROUTE_PARTS = Object.freeze({});

export function renderPngUnitRig(renderer, entity, colorByOwner, state, definition, options = {}) {
  const atlas = options.atlas;
  const atlasTexture = options.atlasTexture;
  if (!definition || !atlas || !atlasTexture) return null;
  const context = options.renderContext ?? renderer._rigRenderContextFor?.(entity, colorByOwner, state) ?? {};
  if (typeof options.alpha === "number") context.shotRevealAlpha = options.alpha;
  const rendered = [];
  for (const route of options.routes || []) {
    if (!options.routesCovered && !pngAtlasCanRenderRoute(definition, atlas, route)) continue;
    const pool = renderer._liveRigPools?.[route.poolName];
    if (!pool) continue;
    let instance = pool.get(entity.id);
    if (instance && !instance.matchesPngAtlasRig?.(entity.kind, definition, atlas, atlasTexture, route.parts)) {
      instance.destroy?.();
      pool.delete(entity.id);
      instance = null;
      renderer._recordRenderDiagnostic?.(`renderer.pngRig.instance.rebuilt.${route.poolName}`);
    }
    if (!instance) {
      instance = new PngAtlasRigInstance(
        entity.kind,
        definition,
        atlas,
        atlasTexture,
        renderer._rigPixiFactory ?? createDefaultPngPixiFactory(),
        { includeParts: route.parts },
      );
      renderer._recordRenderDiagnostic?.(`renderer.pngRig.instance.created.${route.poolName}`);
      renderer._recordRenderDiagnostic?.("renderer.pixi.displayObject.created.pngRigContainer");
      renderer._recordRenderDiagnostic?.("renderer.pixi.displayObject.created.pngRigPart", instance.parts?.size || 0);
    } else {
      renderer._recordRenderDiagnostic?.(`renderer.pngRig.instance.reused.${route.poolName}`);
    }
    pool.set(entity.id, instance);
    renderer._seen[route.poolName]?.add(entity.id);
    const layer = renderer.layers[route.layerName];
    if (!instance.container.parent && layer) layer.addChild(instance.container);
    instance.update(entity, context, {
      sampledAnimation: options.sampledAnimation,
      diagnostics: (label, amount = 1) => renderer._recordRenderDiagnostic?.(label, amount),
    });
    rendered.push(instance);
  }
  return rendered;
}

export function pngAtlasCanRenderRoute(definition, atlas, route) {
  if (!definition || !atlas) return false;
  const includeParts = normalizedPartSet(route?.parts);
  if (!includeParts) return atlasSprites(definition, atlas).length > 0;
  if (includeParts.size === 0) return false;
  const coverage = pngAtlasRouteCoverage(definition, atlas, route);
  return coverage.coveredParts.length === includeParts.size && coverage.missingParts.length === 0;
}

export function pngAtlasRouteCoverage(definition, atlas, route) {
  const includeParts = normalizedPartSet(route?.parts);
  if (!definition || !atlas) {
    return {
      coveredParts: [],
      missingParts: includeParts ? [...includeParts] : [],
      animationParts: [],
    };
  }
  const cacheKey = includeParts
    ? (isImmutablePartSelection(route?.parts) ? route.parts : null)
    : ALL_ROUTE_PARTS;
  const cached = cacheKey ? cachedRouteCoverage(definition, atlas, cacheKey) : null;
  if (cached) return cached;
  const sprites = atlasSprites(definition, atlas);
  const coveredParts = new Set();
  const animationParts = new Set();
  for (const sprite of sprites) {
    let covered = false;
    for (const partId of sprite.sourceParts) {
      if (!includeParts || includeParts.has(partId)) {
        coveredParts.add(partId);
        covered = true;
      }
    }
    if (covered && sprite.animationPart) animationParts.add(sprite.animationPart);
  }
  const coverage = Object.freeze({
    coveredParts: Object.freeze([...coveredParts]),
    missingParts: Object.freeze(includeParts ? [...includeParts].filter((partId) => !coveredParts.has(partId)) : []),
    animationParts: Object.freeze([...animationParts]),
  });
  if (cacheKey) cacheRouteCoverage(definition, atlas, cacheKey, coverage);
  return coverage;
}

function createDefaultPngPixiFactory(pixi = globalThis.PIXI) {
  return {
    createContainer: () => new pixi.Container(),
    createRectangle: (x, y, width, height) => new pixi.Rectangle(x, y, width, height),
    createTexture: (baseTexture, rectangle) => new pixi.Texture(baseTexture, rectangle),
    createSprite: (texture) => new pixi.Sprite(texture),
  };
}

class PngAtlasRigInstance {
  constructor(kind, definition, atlas, atlasTexture, pixiFactory, options = {}) {
    this.kind = kind;
    this.definition = definition;
    this.atlas = atlas;
    this.atlasTexture = atlasTexture;
    this._pixiFactory = pixiFactory;
    this.container = pixiFactory.createContainer();
    this.parts = new Map();
    const routeParts = normalizedPartSet(options.includeParts);
    this._routeParts = routeParts ? new Set(routeParts) : null;
    this._routePartKey = partSelectionKey(this._routeParts);
    this._destroyed = false;

    const baseTexture = atlasTexture.baseTexture ?? atlasTexture;
    for (const sprite of atlasSprites(definition, atlas)) {
      if (!spriteMatchesRoute(sprite, this._routeParts)) continue;
      const frame = sprite.frame;
      if (!frame) continue;
      const baseFrame = createFrameRecord(frame, baseTexture, pixiFactory);
      const paletteFrames = createPaletteFrameRecords(sprite.paletteFrames, baseTexture, pixiFactory);
      const display = pixiFactory.createSprite(baseFrame.texture);
      display.rtsRigPartId = sprite.id;
      display.anchor?.set?.(0, 0);
      this.parts.set(sprite.id, { sprite, display, baseFrame, paletteFrames });
      this.container.addChild(display);
    }
  }

  matchesPngAtlasRig(kind, definition, atlas, atlasTexture, includeParts = null) {
    return (
      this.kind === kind &&
      this.definition === definition &&
      this.atlas === atlas &&
      this.atlasTexture === atlasTexture &&
      this._routePartKey === partSelectionKey(includeParts)
    );
  }

  update(entity, renderContext = {}, options = {}) {
    if (this._destroyed) return;
    this.container.visible = true;
    this.container.alpha = renderContext.shotRevealAlpha ?? 1;
    setPoint(this.container.position, entity.x ?? 0, entity.y ?? 0);
    const scale = renderContext.occupiedTrench ? OCCUPIED_TRENCH_UNIT_SCALE : 1;
    setPoint(this.container.scale, scale, scale);
    this.container.rotation = 0;

    const sampled = options.sampledAnimation ?? sampleRigAnimation(
      this.definition,
      entity,
      renderContext,
      { includeParts: animationPartIds(this.parts) },
    );
    for (const [spriteId, rec] of this.parts) {
      const partState = sampled.parts[rec.sprite.animationPart];
      if (!partState) {
        rec.display.visible = false;
        options.diagnostics?.("renderer.pngRig.redraw.skipped.hidden");
        continue;
      }
      const frameRecord = frameRecordForContext(rec, sampled.context);
      if (rec.display.texture !== frameRecord.texture) rec.display.texture = frameRecord.texture;
      applySpriteState(rec.display, rec.sprite, frameRecord.frame, partState, sampled.context, options.diagnostics);
    }
  }

  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;
    this.container.parent?.removeChild?.(this.container);
    for (const rec of this.parts.values()) {
      rec.display.destroy?.({ texture: false, baseTexture: false });
      rec.baseFrame.texture?.destroy?.(false);
      for (const frame of rec.paletteFrames?.values?.() || []) frame.texture?.destroy?.(false);
    }
    this.parts.clear();
    this.container.destroy?.({ children: true });
  }
}

function createFrameRecord(frame, baseTexture, pixiFactory) {
  const rectangle = pixiFactory.createRectangle(frame.x, frame.y, frame.w, frame.h);
  return {
    frame,
    texture: pixiFactory.createTexture(baseTexture, rectangle),
  };
}

function createPaletteFrameRecords(paletteFrames, baseTexture, pixiFactory) {
  if (!paletteFrames || typeof paletteFrames !== "object") return null;
  const records = new Map();
  for (const [key, frame] of Object.entries(paletteFrames)) {
    const colorKey = normalizedColorKey(key);
    if (!colorKey || !frame) continue;
    records.set(colorKey, createFrameRecord(frame, baseTexture, pixiFactory));
  }
  return records.size > 0 ? records : null;
}

function frameRecordForContext(rec, context) {
  const colorKey = normalizedColorKey(context?.teamColor);
  return colorKey && rec.paletteFrames?.get(colorKey) ? rec.paletteFrames.get(colorKey) : rec.baseFrame;
}

function applySpriteState(display, part, frame, state, context, diagnostics = null) {
  display.visible = state.visible;
  if (!state.visible) {
    diagnostics?.("renderer.pngRig.redraw.skipped.hidden");
    return;
  }

  const transform = displayTransform(state, frame, part);
  applyDisplayTransform(display, transform);
  display.alpha = state.alpha;
  display.tint = tintForSlot(part.tintSlot ?? state.tintSlot, context, part);
  diagnostics?.("renderer.pngRig.redraw.completed");
}

function atlasSprites(definition, atlas) {
  const cached = cachedAtlasSprites(definition, atlas);
  if (cached) return cached;
  let sprites;
  if (Array.isArray(atlas?.sprites) && atlas.sprites.length > 0) {
    sprites = atlas.sprites
      .filter((sprite) => sprite?.frame)
      .map((sprite) => ({
        id: sprite.id,
        animationPart: sprite.animationPart,
        sourceParts: Array.isArray(sprite.sourceParts) ? sprite.sourceParts : [sprite.animationPart],
        tintSlot: sprite.tintSlot ?? "fixed",
        frame: sprite.frame,
        rotationOffset: sprite.rotationOffset ?? 0,
        rotationPivotX: sprite.rotationPivotX ?? null,
        rotationPivotY: sprite.rotationPivotY ?? null,
        rotationPivotReferenceOffset: sprite.rotationPivotReferenceOffset ?? 0,
        positionOffsetX: sprite.positionOffsetX ?? 0,
        positionOffsetY: sprite.positionOffsetY ?? 0,
        tintAdjustment: sprite.tintAdjustment ?? null,
        paletteFrames: sprite.paletteFrames,
        drawOrder: sprite.drawOrder ?? 0,
      }))
      .sort((a, b) => a.drawOrder - b.drawOrder || a.id.localeCompare(b.id));
  } else {
    sprites = (definition.parts || [])
      .map((part) => ({
        id: part.id,
        animationPart: part.id,
        sourceParts: [part.id],
        tintSlot: part.tintSlot,
        frame: atlas.frames?.[part.id],
        paletteFrames: atlas.paletteFrames?.[part.id],
        drawOrder: part.drawOrder ?? 0,
      }))
      .filter((sprite) => sprite.frame);
  }
  const frozen = Object.freeze(sprites.map((sprite) => Object.freeze(sprite)));
  cacheAtlasSprites(definition, atlas, frozen);
  return frozen;
}

function cachedAtlasSprites(definition, atlas) {
  return ATLAS_SPRITES_CACHE.get(definition)?.get(atlas) ?? null;
}

function cacheAtlasSprites(definition, atlas, sprites) {
  let byAtlas = ATLAS_SPRITES_CACHE.get(definition);
  if (!byAtlas) {
    byAtlas = new WeakMap();
    ATLAS_SPRITES_CACHE.set(definition, byAtlas);
  }
  byAtlas.set(atlas, sprites);
}

function cachedRouteCoverage(definition, atlas, routeParts) {
  return ROUTE_COVERAGE_CACHE.get(definition)?.get(atlas)?.get(routeParts) ?? null;
}

function cacheRouteCoverage(definition, atlas, routeParts, coverage) {
  let byAtlas = ROUTE_COVERAGE_CACHE.get(definition);
  if (!byAtlas) {
    byAtlas = new WeakMap();
    ROUTE_COVERAGE_CACHE.set(definition, byAtlas);
  }
  let byRoute = byAtlas.get(atlas);
  if (!byRoute) {
    byRoute = new WeakMap();
    byAtlas.set(atlas, byRoute);
  }
  byRoute.set(routeParts, coverage);
}

function animationPartIds(parts) {
  return new Set([...parts.values()].map((rec) => rec.sprite.animationPart));
}

function spriteMatchesRoute(sprite, includeParts) {
  if (!includeParts) return true;
  return sprite.sourceParts.some((partId) => includeParts.has(partId));
}

function displayTransform(state, frame, part = null) {
  const pixelsPerUnitX = frame.pixelsPerUnitX || frame.pixelsPerUnit || 1;
  const pixelsPerUnitY = frame.pixelsPerUnitY || frame.pixelsPerUnit || 1;
  const localOffset = rotateOffset(state.localOffset, state.transform.rotation);
  const defaultPivotX = frame.originX + state.pivot.x * pixelsPerUnitX;
  const defaultPivotY = frame.originY + state.pivot.y * pixelsPerUnitY;
  const pivotX = part?.rotationPivotX ?? frame.rotationPivotX ?? defaultPivotX;
  const pivotY = part?.rotationPivotY ?? frame.rotationPivotY ?? defaultPivotY;
  const spriteOffset = rotateOffset({
    x: part?.positionOffsetX ?? frame.positionOffsetX ?? 0,
    y: part?.positionOffsetY ?? frame.positionOffsetY ?? 0,
  }, state.transform.rotation);
  const pivotReferenceRotation = state.transform.rotation + (
    part?.rotationPivotReferenceOffset ?? frame.rotationPivotReferenceOffset ?? 0
  );
  const pivotOffset = rotateOffset({
    x: (pivotX - defaultPivotX) / pixelsPerUnitX,
    y: (pivotY - defaultPivotY) / pixelsPerUnitY,
  }, pivotReferenceRotation);
  const rotation = state.transform.rotation + (part?.rotationOffset ?? frame.rotationOffset ?? 0);
  return {
    x: state.transform.x + localOffset.x + spriteOffset.x + pivotOffset.x,
    y: state.transform.y + localOffset.y + spriteOffset.y + pivotOffset.y,
    pivotX,
    pivotY,
    scaleX: (state.transform.scaleX * (state.geometryScale?.x ?? 1)) / pixelsPerUnitX,
    scaleY: (state.transform.scaleY * (state.geometryScale?.y ?? 1)) / pixelsPerUnitY,
    rotation,
  };
}

function applyDisplayTransform(display, transform) {
  setPoint(display.position, transform.x, transform.y);
  setPoint(display.pivot, transform.pivotX, transform.pivotY);
  setPoint(display.scale, transform.scaleX, transform.scaleY);
  display.rotation = transform.rotation;
}

function tintForSlot(slot, context, part) {
  const team = context.teamColor ?? 0x6d89b8;
  let color = 0xffffff;
  if (slot === "team") color = team;
  else if (slot === "team-light") color = lightenColor(team, 0.18);
  else if (slot === "team-light-soft") color = lightenColor(team, 0.1);
  else if (slot === "team-fill-stroke") color = team;
  else if (typeof slot === "string" && /^#[0-9a-f]{6}$/i.test(slot)) color = hexToInt(slot);
  return adjustTintColor(color, part?.tintAdjustment);
}

function adjustTintColor(color, adjustment) {
  if (!adjustment || typeof adjustment !== "object") return color;
  const saturation = Number.isFinite(adjustment.saturation) ? adjustment.saturation / 100 : 1;
  const brightness = Number.isFinite(adjustment.brightness) ? adjustment.brightness / 100 : 1;
  let r = (color >> 16) & 0xff;
  let g = (color >> 8) & 0xff;
  let b = color & 0xff;
  if (saturation !== 1) {
    const luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    r = luma + (r - luma) * saturation;
    g = luma + (g - luma) * saturation;
    b = luma + (b - luma) * saturation;
  }
  if (brightness !== 1) {
    r *= brightness;
    g *= brightness;
    b *= brightness;
  }
  return (clampByte(r) << 16) | (clampByte(g) << 8) | clampByte(b);
}

function clampByte(value) {
  return Math.max(0, Math.min(255, Math.round(value)));
}

function normalizedColorKey(color) {
  const value = typeof color === "string" || typeof color === "number" ? hexToInt(color) : null;
  if (!Number.isFinite(value)) return null;
  return `#${(value & 0xffffff).toString(16).padStart(6, "0").toLowerCase()}`;
}

function rotateOffset(offset, rotation) {
  if (!offset || (offset.x === 0 && offset.y === 0)) return { x: 0, y: 0 };
  const cos = Math.cos(rotation);
  const sin = Math.sin(rotation);
  return {
    x: offset.x * cos - offset.y * sin,
    y: offset.x * sin + offset.y * cos,
  };
}

function setPoint(point, x, y) {
  if (typeof point?.set === "function") point.set(x, y);
  else if (point) {
    point.x = x;
    point.y = y;
  }
}
