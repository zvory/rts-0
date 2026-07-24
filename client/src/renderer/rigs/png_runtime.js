import { hexToInt, lightenColor } from "../shared.js";
import { createRigAnimationStage, sampleRigAnimationInto } from "./animation.js";
import { flushRigDiagnosticCounts } from "./diagnostics.js";
import { isImmutablePartSelection, normalizedPartSet, partSelectionKey } from "./part_selection.js";

const OCCUPIED_TRENCH_UNIT_SCALE = 0.85;
const ATLAS_SPRITES_CACHE = new WeakMap();
const ROUTE_COVERAGE_CACHE = new WeakMap();
const ALL_ROUTE_PARTS = Object.freeze({});
const PNG_RIG_DIAGNOSTIC_LABELS = Object.freeze([
  "renderer.pngRig.redraw.skipped.hidden",
  "renderer.pngRig.redraw.completed",
]);
const PNG_RIG_HIDDEN = 0;
const PNG_RIG_COMPLETED = 1;

export function renderPngUnitRig(renderer, entity, colorByOwner, state, definition, options = {}) {
  const atlas = options.atlas;
  const atlasTexture = options.atlasTexture;
  if (!definition || !atlas || !atlasTexture) return null;
  const context = options.renderContext ?? renderer._rigRenderContextFor?.(entity, colorByOwner, state) ?? {};
  if (typeof options.alpha === "number") context.shotRevealAlpha = options.alpha;
  const rendered = options.collectResults === false ? null : [];
  if (options.route) {
    const instance = renderPngUnitRigRoute(renderer, entity, definition, atlas, atlasTexture, context, options.route, options);
    if (instance && rendered) rendered.push(instance);
  } else {
    for (const route of options.routes || []) {
      const instance = renderPngUnitRigRoute(renderer, entity, definition, atlas, atlasTexture, context, route, options);
      if (instance && rendered) rendered.push(instance);
    }
  }
  return rendered;
}

function renderPngUnitRigRoute(renderer, entity, definition, atlas, atlasTexture, context, route, options) {
  if (!options.routesCovered && !pngAtlasCanRenderRoute(definition, atlas, route)) return null;
  const pool = renderer._liveRigPools?.[route.poolName];
  if (!pool) return null;
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
    diagnosticRecorder: renderer,
  });
  return instance;
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
    createTexture: (source, rectangle) => new pixi.Texture({ source, frame: rectangle }),
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

    const baseTexture = atlasTexture.source;
    for (const sprite of atlasSprites(definition, atlas)) {
      if (!spriteMatchesRoute(sprite, this._routeParts)) continue;
      const frame = sprite.frame;
      if (!frame) continue;
      const baseFrame = createFrameRecord(frame, baseTexture, pixiFactory);
      const paletteFrames = createPaletteFrameRecords(sprite.paletteFrames, baseTexture, pixiFactory);
      const display = pixiFactory.createSprite(baseFrame.texture);
      display.rtsRigPartId = sprite.id;
      display.anchor?.set?.(0, 0);
      this.parts.set(sprite.id, {
        sprite,
        display,
        baseFrame,
        paletteFrames,
        transform: { initialized: false },
      });
      this.container.addChild(display);
    }
    this._animationStage = null;
    this._diagnosticCounts = new Uint32Array(PNG_RIG_DIAGNOSTIC_LABELS.length);
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
    const visualScale = Number.isFinite(renderContext.visualScale) ? renderContext.visualScale : 1;
    const scale = (renderContext.occupiedTrench ? OCCUPIED_TRENCH_UNIT_SCALE : 1)
      * Math.max(0.01, visualScale);
    setPoint(this.container.scale, scale, scale);
    this.container.rotation = 0;

    let sampled = options.sampledAnimation;
    if (!sampled) {
      if (!this._animationStage) {
        const animationPartIds = new Set([...this.parts.values()].map((rec) => rec.sprite.animationPart));
        this._animationStage = createRigAnimationStage(this.definition, { includeParts: animationPartIds });
      }
      sampled = sampleRigAnimationInto(this._animationStage, entity, renderContext);
    }
    const diagnosticCounts = this._diagnosticCounts;
    diagnosticCounts.fill(0);
    for (const [spriteId, rec] of this.parts) {
      const partState = sampled.parts[rec.sprite.animationPart];
      if (!partState) {
        rec.display.visible = false;
        diagnosticCounts[PNG_RIG_HIDDEN] += 1;
        continue;
      }
      const frameRecord = frameRecordForContext(rec, sampled.context);
      if (rec.display.texture !== frameRecord.texture) rec.display.texture = frameRecord.texture;
      applySpriteState(rec, frameRecord.frame, partState, sampled.context, diagnosticCounts);
    }
    flushRigDiagnosticCounts(options, PNG_RIG_DIAGNOSTIC_LABELS, diagnosticCounts);
  }

  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;
    this.container.parent?.removeChild?.(this.container);
    for (const rec of this.parts.values()) {
      rec.display.destroy?.({ texture: false, textureSource: false });
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
    const colorKey = normalizedColorValue(key);
    if (colorKey === null || !frame) continue;
    records.set(colorKey, createFrameRecord(frame, baseTexture, pixiFactory));
  }
  return records.size > 0 ? records : null;
}

function frameRecordForContext(rec, context) {
  const colorKey = normalizedColorValue(context?.teamColor);
  return colorKey !== null && rec.paletteFrames?.has(colorKey) ? rec.paletteFrames.get(colorKey) : rec.baseFrame;
}

function applySpriteState(rec, frame, state, context, diagnosticCounts) {
  const { display, sprite: part } = rec;
  display.visible = state.visible;
  if (!state.visible) {
    diagnosticCounts[PNG_RIG_HIDDEN] += 1;
    return;
  }

  applyDisplayTransform(display, state, frame, part, rec.transform);
  display.alpha = state.alpha;
  display.tint = tintForSlot(part.tintSlot ?? state.tintSlot, context, part);
  diagnosticCounts[PNG_RIG_COMPLETED] += 1;
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

function spriteMatchesRoute(sprite, includeParts) {
  if (!includeParts) return true;
  return sprite.sourceParts.some((partId) => includeParts.has(partId));
}

function applyDisplayTransform(display, state, frame, part, last) {
  const pixelsPerUnitX = frame.pixelsPerUnitX || frame.pixelsPerUnit || 1;
  const pixelsPerUnitY = frame.pixelsPerUnitY || frame.pixelsPerUnit || 1;
  const stateRotation = state.transform.rotation;
  const cos = Math.cos(stateRotation);
  const sin = Math.sin(stateRotation);
  const localX = state.localOffset?.x ?? 0;
  const localY = state.localOffset?.y ?? 0;
  const rotatedLocalX = localX === 0 && localY === 0 ? 0 : localX * cos - localY * sin;
  const rotatedLocalY = localX === 0 && localY === 0 ? 0 : localX * sin + localY * cos;
  const defaultPivotX = frame.originX + state.pivot.x * pixelsPerUnitX;
  const defaultPivotY = frame.originY + state.pivot.y * pixelsPerUnitY;
  const pivotX = part?.rotationPivotX ?? frame.rotationPivotX ?? defaultPivotX;
  const pivotY = part?.rotationPivotY ?? frame.rotationPivotY ?? defaultPivotY;
  const spriteOffsetX = part?.positionOffsetX ?? frame.positionOffsetX ?? 0;
  const spriteOffsetY = part?.positionOffsetY ?? frame.positionOffsetY ?? 0;
  const rotatedSpriteX = spriteOffsetX === 0 && spriteOffsetY === 0
    ? 0
    : spriteOffsetX * cos - spriteOffsetY * sin;
  const rotatedSpriteY = spriteOffsetX === 0 && spriteOffsetY === 0
    ? 0
    : spriteOffsetX * sin + spriteOffsetY * cos;
  const pivotReferenceRotation = stateRotation + (
    part?.rotationPivotReferenceOffset ?? frame.rotationPivotReferenceOffset ?? 0
  );
  const pivotReferenceCos = Math.cos(pivotReferenceRotation);
  const pivotReferenceSin = Math.sin(pivotReferenceRotation);
  const pivotLocalX = (pivotX - defaultPivotX) / pixelsPerUnitX;
  const pivotLocalY = (pivotY - defaultPivotY) / pixelsPerUnitY;
  const pivotOffsetX = pivotLocalX === 0 && pivotLocalY === 0
    ? 0
    : pivotLocalX * pivotReferenceCos - pivotLocalY * pivotReferenceSin;
  const pivotOffsetY = pivotLocalX === 0 && pivotLocalY === 0
    ? 0
    : pivotLocalX * pivotReferenceSin + pivotLocalY * pivotReferenceCos;
  const x = state.transform.x + rotatedLocalX + rotatedSpriteX + pivotOffsetX;
  const y = state.transform.y + rotatedLocalY + rotatedSpriteY + pivotOffsetY;
  const scaleX = (state.transform.scaleX * (state.geometryScale?.x ?? 1)) / pixelsPerUnitX;
  const scaleY = (state.transform.scaleY * (state.geometryScale?.y ?? 1)) / pixelsPerUnitY;
  const rotation = stateRotation + (part?.rotationOffset ?? frame.rotationOffset ?? 0);

  if (!last.initialized || last.x !== x || last.y !== y) setPoint(display.position, x, y);
  if (!last.initialized || last.pivotX !== pivotX || last.pivotY !== pivotY) setPoint(display.pivot, pivotX, pivotY);
  if (!last.initialized || last.scaleX !== scaleX || last.scaleY !== scaleY) setPoint(display.scale, scaleX, scaleY);
  if (!last.initialized || last.rotation !== rotation) display.rotation = rotation;
  last.initialized = true;
  last.x = x;
  last.y = y;
  last.pivotX = pivotX;
  last.pivotY = pivotY;
  last.scaleX = scaleX;
  last.scaleY = scaleY;
  last.rotation = rotation;
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

function normalizedColorValue(color) {
  const value = typeof color === "string" || typeof color === "number" ? hexToInt(color) : null;
  if (!Number.isFinite(value)) return null;
  return value & 0xffffff;
}

function setPoint(point, x, y) {
  if (typeof point?.set === "function") point.set(x, y);
  else if (point) {
    point.x = x;
    point.y = y;
  }
}
