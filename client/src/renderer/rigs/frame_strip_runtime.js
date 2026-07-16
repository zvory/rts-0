import { SETUP, STATE } from "../../protocol.js";
import { hexToInt, lightenColor } from "../shared.js";

const OCCUPIED_TRENCH_UNIT_SCALE = 0.85;

export function renderFrameStripUnit(renderer, entity, strip, texture, options = {}) {
  if (!strip || !texture || !options.poolName || !options.layerName) return null;
  const pool = renderer._liveRigPools?.[options.poolName];
  if (!pool) return null;
  const renderContext = options.renderContext || {};
  if (typeof options.alpha === "number") renderContext.shotRevealAlpha = options.alpha;

  let instance = pool.get(entity.id);
  if (instance && !instance.matchesFrameStripUnit?.(entity.kind, strip, texture)) {
    instance.destroy?.();
    pool.delete(entity.id);
    instance = null;
    renderer._recordRenderDiagnostic?.(`renderer.frameStrip.instance.rebuilt.${options.poolName}`);
  }
  if (!instance) {
    const pixiFactory = renderer._rigPixiFactory ?? createDefaultFrameStripPixiFactory();
    instance = new FrameStripUnitInstance(
      entity.kind,
      strip,
      texture,
      pixiFactory,
      sharedFrameTextures(renderer, strip, texture, pixiFactory),
    );
    renderer._recordRenderDiagnostic?.(`renderer.frameStrip.instance.created.${options.poolName}`);
    renderer._recordRenderDiagnostic?.("renderer.pixi.displayObject.created.frameStripSprite");
  } else {
    renderer._recordRenderDiagnostic?.(`renderer.frameStrip.instance.reused.${options.poolName}`);
  }

  pool.set(entity.id, instance);
  renderer._seen[options.poolName]?.add(entity.id);
  const layer = renderer.layers[options.layerName];
  if (!instance.container.parent && layer) layer.addChild(instance.container);
  instance.update(entity, renderContext, {
    alpha: options.alpha,
    diagnostics: (label, amount = 1) => renderer._recordRenderDiagnostic?.(label, amount),
  });
  return instance;
}

export function frameStripFrameIndex(strip, entity, nowOrContext = 0) {
  const idleFrame = validFrame(strip, strip?.idleFrame ?? 0);
  const renderContext = typeof nowOrContext === "object" && nowOrContext !== null ? nowOrContext : {};
  const now = typeof nowOrContext === "number" ? nowOrContext : renderContext.now;
  const setupFrame = frameStripSetupFrameIndex(strip, entity, renderContext, idleFrame);
  if (setupFrame != null) return setupFrame;
  const setupFrames = validFrameList(strip, strip?.setupFrames);
  if (!frameStripEntityIsMoving(entity, renderContext)) {
    if (setupFrames.length === 0) {
      const firingFrame = frameStripFiringFrameIndex(strip, entity, renderContext, idleFrame);
      if (firingFrame != null) return firingFrame;
    }
    return idleFrame;
  }
  if (setupFrames.length === 0) {
    const firingFrame = frameStripFiringFrameIndex(strip, entity, renderContext, idleFrame);
    if (firingFrame != null) return firingFrame;
  }
  const frames = validFrameList(strip, strip?.movementFrames);
  if (frames.length === 0) return idleFrame;
  const fps = Math.max(1, finite(strip?.fps, 12));
  const frameDurationMs = 1000 / fps;
  const timeIndex = Math.floor(Math.max(0, finite(now, 0)) / frameDurationMs);
  const idOffset = Number.isFinite(entity?.id) ? Math.abs(Math.trunc(entity.id)) % frames.length : 0;
  return validFrame(strip, frames[(timeIndex + idOffset) % frames.length], idleFrame);
}

function frameStripSetupFrameIndex(strip, entity, renderContext = {}, idleFrame = 0) {
  const setupFrames = validFrameList(strip, strip?.setupFrames);
  if (setupFrames.length === 0) return null;
  const setupState = entity?.setupState || SETUP.PACKED;
  if (setupState === SETUP.DEPLOYED) {
    const firingFrame = frameStripFiringFrameIndex(strip, entity, renderContext, idleFrame);
    if (firingFrame != null) return firingFrame;
    return validFrame(strip, strip?.deployedFrame ?? setupFrames[setupFrames.length - 1], idleFrame);
  }
  if (setupState !== SETUP.SETTING_UP && setupState !== SETUP.TEARING_DOWN) return null;
  const setupVisual = renderContext?.setupVisual ?? {};
  const progress = clamp01(finite(setupVisual.frameProgress, finite(setupVisual.prongFactor, 0)));
  const index = Math.min(setupFrames.length - 1, Math.floor(progress * setupFrames.length));
  return setupFrames[index];
}

function frameStripFiringFrameIndex(strip, entity, renderContext = {}, fallback = 0) {
  const firingFrames = validFrameList(strip, strip?.firingFrames);
  if (firingFrames.length === 0) return null;
  if (clamp01(finite(renderContext.recoilProgress, 0)) <= 0) return null;
  const phase = clamp01(finite(renderContext.recoilPhase, 0));
  const holdPhase = finite(strip?.firingFrameHoldPhase, 0);
  const framePhase = holdPhase > 0 && holdPhase < 1 ? phase / holdPhase : phase;
  if (holdPhase > 0 && holdPhase < 1 && phase >= holdPhase) return null;
  const index = Math.min(firingFrames.length - 1, Math.floor(framePhase * firingFrames.length));
  return validFrame(strip, firingFrames[index], fallback);
}

export function frameStripVisualFacing(stripOrEntity, maybeEntity = null, renderContext = null) {
  const strip = maybeEntity ? stripOrEntity : null;
  const entity = maybeEntity ?? stripOrEntity;
  const moving = frameStripEntityIsMoving(entity, renderContext);
  const setupState = entity?.setupState || SETUP.PACKED;
  if (strip?.packedFacing === "body" && setupState === SETUP.PACKED && !moving && Number.isFinite(entity?.facing)) {
    return entity.facing;
  }
  if (strip && setupState !== SETUP.PACKED) {
    const setupForwardAngle = finite(strip.setupForwardAngle, 0);
    if (Number.isFinite(entity?.weaponFacing)) return entity.weaponFacing - setupForwardAngle;
    if (Number.isFinite(entity?.facing)) return entity.facing - setupForwardAngle;
  }
  if (frameStripUsesMovementFrames(strip, entity, renderContext) && Number.isFinite(entity?.facing)) {
    return entity.facing + finite(strip?.movementFacingOffset, 0);
  }
  if (!moving && Number.isFinite(entity?.weaponFacing)) return entity.weaponFacing;
  return finite(entity?.facing, 0);
}

export function frameStripWorldScale(strip, entity, renderContext = null) {
  const baseScale = Math.max(0.01, finite(strip?.worldScale, 1));
  if (frameStripUsesMovementFrames(strip, entity, renderContext)) {
    return Math.max(0.01, finite(strip?.movementWorldScale, baseScale));
  }
  return baseScale;
}

function createDefaultFrameStripPixiFactory(pixi = globalThis.PIXI) {
  return {
    createRectangle: (x, y, width, height) => new pixi.Rectangle(x, y, width, height),
    createTexture: (baseTexture, rectangle) => new pixi.Texture(baseTexture, rectangle),
    createSprite: (spriteTexture) => new pixi.Sprite(spriteTexture),
  };
}

class FrameStripUnitInstance {
  constructor(kind, strip, texture, pixiFactory, frameTextures) {
    this.kind = kind;
    this.strip = strip;
    this.texture = texture;
    this.frameTextures = frameTextures;
    this._frameIndex = -1;
    this._destroyed = false;

    this.sprite = pixiFactory.createSprite(this.frameTextures[0]);
    // Frame-strip bodies have no local offset from their per-unit containers.
    // Make the Sprite the pooled display object directly so Pixi traverses one
    // object per body while preserving its exact position in the units layer.
    this.container = this.sprite;
    this.sprite.anchor?.set?.(0.5, 0.5);
  }

  matchesFrameStripUnit(kind, strip, texture) {
    return this.kind === kind && this.strip === strip && this.texture === texture;
  }

  update(entity, renderContext = {}, options = {}) {
    if (this._destroyed) return;
    this.container.visible = true;
    this.container.alpha = renderContext.shotRevealAlpha ?? options.alpha ?? 1;
    setPoint(this.container.position, finite(entity.x, 0), finite(entity.y, 0));
    const occupiedScale = renderContext.occupiedTrench ? OCCUPIED_TRENCH_UNIT_SCALE : 1;
    this.container.rotation = frameStripVisualFacing(this.strip, entity, renderContext);

    const frameIndex = frameStripFrameIndex(this.strip, entity, renderContext);
    if (frameIndex !== this._frameIndex) {
      this.sprite.texture = this.frameTextures[frameIndex] ?? this.frameTextures[0];
      this._frameIndex = frameIndex;
    }

    const worldScale = frameStripWorldScale(this.strip, entity, renderContext);
    const displayScale = occupiedScale * worldScale;
    setPoint(this.sprite.scale, displayScale, displayScale);
    this.sprite.tint = tintForSlot(this.strip.tintSlot, renderContext);
    this.sprite.visible = true;
    options.diagnostics?.("renderer.frameStrip.redraw.completed");
  }

  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;
    this.container.parent?.removeChild?.(this.container);
    this.sprite?.destroy?.({ texture: false, baseTexture: false });
    this.frameTextures = [];
  }
}

function sharedFrameTextures(renderer, strip, texture, pixiFactory) {
  const cache = renderer._frameStripTextureSets ??= new Map();
  let stripTextures = cache.get(strip);
  if (!stripTextures) {
    stripTextures = new Map();
    cache.set(strip, stripTextures);
  }

  let entry = stripTextures.get(texture);
  if (!entry) {
    const frames = [];
    const frameWidth = Math.max(1, finite(strip.frameWidth, 1));
    const frameHeight = Math.max(1, finite(strip.frameHeight, 1));
    const frameCount = Math.max(1, Math.trunc(finite(strip.frameCount, 1)));
    const baseTexture = texture.baseTexture ?? texture;
    for (let i = 0; i < frameCount; i += 1) {
      const rectangle = pixiFactory.createRectangle(i * frameWidth, 0, frameWidth, frameHeight);
      frames.push(pixiFactory.createTexture(baseTexture, rectangle));
    }
    entry = frames;
    stripTextures.set(texture, entry);
  }
  return entry;
}

export function destroySharedFrameTextures(renderer) {
  const cache = renderer?._frameStripTextureSets;
  if (!cache) return;
  for (const stripTextures of cache.values()) {
    for (const frameTextures of stripTextures.values()) {
      for (const frameTexture of frameTextures) frameTexture?.destroy?.(false);
    }
    stripTextures.clear();
  }
  cache.clear();
}

function tintForSlot(slot, context) {
  const team = Number.isFinite(context.teamColor) ? context.teamColor : hexToInt(context.teamColor);
  if (slot === "team") return team;
  if (slot === "team-light") return lightenColor(team, 0.18);
  if (slot === "team-light-soft") return lightenColor(team, 0.1);
  if (typeof slot === "string" && /^#[0-9a-f]{6}$/i.test(slot)) return hexToInt(slot);
  return 0xffffff;
}

function validFrame(strip, frame, fallback = 0) {
  const count = Math.max(1, Math.trunc(finite(strip?.frameCount, 1)));
  const value = Math.trunc(finite(frame, fallback));
  if (value < 0 || value >= count) return Math.trunc(finite(fallback, 0));
  return value;
}

function validFrameList(strip, frames) {
  if (!Array.isArray(frames)) return [];
  const count = Math.max(1, Math.trunc(finite(strip?.frameCount, 1)));
  const out = [];
  for (const frame of frames) {
    if (!Number.isFinite(frame)) continue;
    const value = Math.trunc(frame);
    if (value >= 0 && value < count) out.push(value);
  }
  return out;
}

function frameStripUsesMovementFrames(strip, entity, renderContext = null) {
  if (!frameStripEntityIsMoving(entity, renderContext)) return false;
  const movementFrames = validFrameList(strip, strip?.movementFrames);
  if (movementFrames.length === 0) return false;
  const setupFrames = validFrameList(strip, strip?.setupFrames);
  if (setupFrames.length === 0) return true;
  const setupState = entity?.setupState || SETUP.PACKED;
  return setupState !== SETUP.DEPLOYED && setupState !== SETUP.SETTING_UP && setupState !== SETUP.TEARING_DOWN;
}

function frameStripEntityIsMoving(entity, renderContext = null) {
  if (entity?.state !== STATE.MOVE) return false;
  if (typeof renderContext?.frameStripMoving === "boolean") return renderContext.frameStripMoving;
  if (Number.isFinite(renderContext?.frameStripMovementActivity)) {
    return renderContext.frameStripMovementActivity > 0.01;
  }
  return true;
}

function clamp01(value) {
  return Math.max(0, Math.min(1, finite(value, 0)));
}

function setPoint(point, x, y) {
  if (point?.set) point.set(x, y);
  else if (point) {
    point.x = x;
    point.y = y;
  }
}

function finite(value, fallback) {
  return Number.isFinite(value) ? value : fallback;
}
