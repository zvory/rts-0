import { COLORS } from "../../config.js";
import { hexToInt, lightenColor } from "../shared.js";
import { sampleRigAnimation } from "./animation.js";

const OCCUPIED_TRENCH_UNIT_SCALE = 0.85;
const OCCUPIED_TRENCH_ALPHA = 0.78;

export function renderPngUnitRig(renderer, entity, colorByOwner, state, definition, options = {}) {
  const atlas = options.atlas;
  const atlasTexture = options.atlasTexture;
  if (!definition || !atlas || !atlasTexture) return null;
  const context = options.renderContext ?? renderer._rigRenderContextFor?.(entity, colorByOwner, state) ?? {};
  if (typeof options.alpha === "number") context.shotRevealAlpha = options.alpha;
  const rendered = [];
  for (const route of options.routes || []) {
    if (!pngAtlasCanRenderRoute(definition, atlas, route)) continue;
    const pool = renderer._liveRigPools?.[route.poolName];
    if (!pool) continue;
    let instance = pool.get(entity.id);
    if (instance && !instance.matchesPngAtlasRig?.(entity.kind, definition, atlas, atlasTexture)) {
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
        renderer._rigPixiFactory ?? createDefaultPngPixiFactory()
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
      includeParts: route.parts,
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
    };
  }
  const sprites = atlasSprites(definition, atlas);
  const coveredParts = new Set();
  for (const sprite of sprites) {
    for (const partId of sprite.sourceParts) {
      if (!includeParts || includeParts.has(partId)) coveredParts.add(partId);
    }
  }
  return {
    coveredParts: [...coveredParts],
    missingParts: includeParts ? [...includeParts].filter((partId) => !coveredParts.has(partId)) : [],
  };
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
  constructor(kind, definition, atlas, atlasTexture, pixiFactory) {
    this.kind = kind;
    this.definition = definition;
    this.atlas = atlas;
    this.atlasTexture = atlasTexture;
    this._pixiFactory = pixiFactory;
    this.container = pixiFactory.createContainer();
    this.parts = new Map();
    this._destroyed = false;

    const baseTexture = atlasTexture.baseTexture ?? atlasTexture;
    for (const sprite of atlasSprites(definition, atlas)) {
      const frame = sprite.frame;
      if (!frame) continue;
      const rectangle = pixiFactory.createRectangle(frame.x, frame.y, frame.w, frame.h);
      const texture = pixiFactory.createTexture(baseTexture, rectangle);
      const display = pixiFactory.createSprite(texture);
      display.rtsRigPartId = sprite.id;
      display.anchor?.set?.(0, 0);
      this.parts.set(sprite.id, { sprite, display, frame });
      this.container.addChild(display);
    }
  }

  matchesPngAtlasRig(kind, definition, atlas, atlasTexture) {
    return (
      this.kind === kind &&
      this.definition === definition &&
      this.atlas === atlas &&
      this.atlasTexture === atlasTexture
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

    const sampled = sampleRigAnimation(this.definition, entity, renderContext);
    const includeParts = normalizedPartSet(options.includeParts);
    for (const [spriteId, rec] of this.parts) {
      if (!spriteMatchesRoute(rec.sprite, includeParts)) {
        rec.display.visible = false;
        options.diagnostics?.("renderer.pngRig.redraw.skipped.hidden");
        continue;
      }
      const partState = sampled.parts[rec.sprite.animationPart];
      if (!partState) {
        rec.display.visible = false;
        options.diagnostics?.("renderer.pngRig.redraw.skipped.hidden");
        continue;
      }
      applySpriteState(rec.display, rec.sprite, rec.frame, partState, sampled.context, options.diagnostics);
    }
  }

  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;
    this.container.parent?.removeChild?.(this.container);
    for (const { display } of this.parts.values()) {
      display.destroy?.({ texture: true, baseTexture: false });
    }
    this.parts.clear();
    this.container.destroy?.({ children: true });
  }
}

function applySpriteState(display, part, frame, state, context, diagnostics = null) {
  display.visible = state.visible;
  if (!state.visible) {
    diagnostics?.("renderer.pngRig.redraw.skipped.hidden");
    return;
  }

  const transform = displayTransform(state, frame);
  applyDisplayTransform(display, transform);
  display.alpha = state.alpha * occupiedTrenchAlpha(context, part);
  display.tint = tintForSlot(state.tintSlot, context, part);
  diagnostics?.("renderer.pngRig.redraw.completed");
}

function atlasSprites(definition, atlas) {
  if (Array.isArray(atlas?.sprites) && atlas.sprites.length > 0) {
    return atlas.sprites
      .filter((sprite) => sprite?.frame)
      .map((sprite) => ({
        id: sprite.id,
        animationPart: sprite.animationPart,
        sourceParts: Array.isArray(sprite.sourceParts) ? sprite.sourceParts : [sprite.animationPart],
        tintSlot: sprite.tintSlot ?? "fixed",
        frame: sprite.frame,
        drawOrder: sprite.drawOrder ?? 0,
      }))
      .sort((a, b) => a.drawOrder - b.drawOrder || a.id.localeCompare(b.id));
  }
  return (definition.parts || [])
    .map((part) => ({
      id: part.id,
      animationPart: part.id,
      sourceParts: [part.id],
      tintSlot: part.tintSlot,
      frame: atlas.frames?.[part.id],
      drawOrder: part.drawOrder ?? 0,
    }))
    .filter((sprite) => sprite.frame);
}

function spriteMatchesRoute(sprite, includeParts) {
  if (!includeParts) return true;
  return sprite.sourceParts.some((partId) => includeParts.has(partId));
}

function displayTransform(state, frame) {
  const pixelsPerUnitX = frame.pixelsPerUnitX || frame.pixelsPerUnit || 1;
  const pixelsPerUnitY = frame.pixelsPerUnitY || frame.pixelsPerUnit || 1;
  const localOffset = rotateOffset(state.localOffset, state.transform.rotation);
  return {
    x: state.transform.x + localOffset.x,
    y: state.transform.y + localOffset.y,
    pivotX: frame.originX + state.pivot.x * pixelsPerUnitX,
    pivotY: frame.originY + state.pivot.y * pixelsPerUnitY,
    scaleX: (state.transform.scaleX * (state.geometryScale?.x ?? 1)) / pixelsPerUnitX,
    scaleY: (state.transform.scaleY * (state.geometryScale?.y ?? 1)) / pixelsPerUnitY,
    rotation: state.transform.rotation,
  };
}

function applyDisplayTransform(display, transform) {
  setPoint(display.position, transform.x, transform.y);
  setPoint(display.pivot, transform.pivotX, transform.pivotY);
  setPoint(display.scale, transform.scaleX, transform.scaleY);
  display.rotation = transform.rotation;
}

function tintForSlot(slot, context, part) {
  if (context.occupiedTrench && part?.id !== "part.shadow" && !String(part?.id || "").includes(".shadow")) {
    return COLORS.trenchDirt;
  }
  const team = context.teamColor ?? 0x6d89b8;
  if (slot === "team") return team;
  if (slot === "team-light") return lightenColor(team, 0.18);
  if (slot === "team-light-soft") return lightenColor(team, 0.1);
  if (slot === "team-fill-stroke") return team;
  if (typeof slot === "string" && /^#[0-9a-f]{6}$/i.test(slot)) return hexToInt(slot);
  return 0xffffff;
}

function occupiedTrenchAlpha(context, part) {
  if (!context.occupiedTrench) return 1;
  if (part?.id === "part.shadow" || String(part?.id || "").includes(".shadow")) return 1;
  return OCCUPIED_TRENCH_ALPHA;
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

function normalizedPartSet(parts) {
  if (!Array.isArray(parts)) return null;
  return new Set(parts);
}

function setPoint(point, x, y) {
  if (typeof point?.set === "function") point.set(x, y);
  else if (point) {
    point.x = x;
    point.y = y;
  }
}
