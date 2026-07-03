import { STATE } from "../../protocol.js";
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
    instance = new FrameStripUnitInstance(
      entity.kind,
      strip,
      texture,
      renderer._rigPixiFactory ?? createDefaultFrameStripPixiFactory()
    );
    renderer._recordRenderDiagnostic?.(`renderer.frameStrip.instance.created.${options.poolName}`);
    renderer._recordRenderDiagnostic?.("renderer.pixi.displayObject.created.frameStripContainer");
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

export function frameStripFrameIndex(strip, entity, now = 0) {
  const idleFrame = validFrame(strip, strip?.idleFrame ?? 0);
  if (entity?.state !== STATE.MOVE) return idleFrame;
  const frames = Array.isArray(strip?.movementFrames) ? strip.movementFrames : [];
  if (frames.length === 0) return idleFrame;
  const fps = Math.max(1, finite(strip?.fps, 12));
  const frameDurationMs = 1000 / fps;
  const timeIndex = Math.floor(Math.max(0, finite(now, 0)) / frameDurationMs);
  const idOffset = Number.isFinite(entity?.id) ? Math.abs(Math.trunc(entity.id)) % frames.length : 0;
  return validFrame(strip, frames[(timeIndex + idOffset) % frames.length], idleFrame);
}

export function frameStripVisualFacing(entity) {
  const moving = entity?.state === STATE.MOVE;
  if (moving && Number.isFinite(entity?.facing)) return entity.facing;
  if (!moving && Number.isFinite(entity?.weaponFacing)) return entity.weaponFacing;
  return finite(entity?.facing, 0);
}

function createDefaultFrameStripPixiFactory(pixi = globalThis.PIXI) {
  return {
    createContainer: () => new pixi.Container(),
    createRectangle: (x, y, width, height) => new pixi.Rectangle(x, y, width, height),
    createTexture: (baseTexture, rectangle) => new pixi.Texture(baseTexture, rectangle),
    createSprite: (spriteTexture) => new pixi.Sprite(spriteTexture),
  };
}

class FrameStripUnitInstance {
  constructor(kind, strip, texture, pixiFactory) {
    this.kind = kind;
    this.strip = strip;
    this.texture = texture;
    this._pixiFactory = pixiFactory;
    this.container = pixiFactory.createContainer();
    this.frameTextures = [];
    this._frameIndex = -1;
    this._destroyed = false;

    const frameWidth = Math.max(1, finite(strip.frameWidth, 1));
    const frameHeight = Math.max(1, finite(strip.frameHeight, 1));
    const frameCount = Math.max(1, Math.trunc(finite(strip.frameCount, 1)));
    const baseTexture = texture.baseTexture ?? texture;
    for (let i = 0; i < frameCount; i += 1) {
      const rectangle = pixiFactory.createRectangle(i * frameWidth, 0, frameWidth, frameHeight);
      this.frameTextures.push(pixiFactory.createTexture(baseTexture, rectangle));
    }

    this.sprite = pixiFactory.createSprite(this.frameTextures[0]);
    this.sprite.anchor?.set?.(0.5, 0.5);
    this.sprite.position?.set?.(0, 0);
    this.container.addChild(this.sprite);
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
    setPoint(this.container.scale, occupiedScale, occupiedScale);
    this.container.rotation = frameStripVisualFacing(entity);

    const frameIndex = frameStripFrameIndex(this.strip, entity, renderContext.now);
    if (frameIndex !== this._frameIndex) {
      this.sprite.texture = this.frameTextures[frameIndex] ?? this.frameTextures[0];
      this._frameIndex = frameIndex;
    }

    const worldScale = Math.max(0.01, finite(this.strip.worldScale, 1));
    setPoint(this.sprite.scale, worldScale, worldScale);
    this.sprite.alpha = 1;
    this.sprite.tint = tintForSlot(this.strip.tintSlot, renderContext);
    this.sprite.visible = true;
    options.diagnostics?.("renderer.frameStrip.redraw.completed");
  }

  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;
    this.container.parent?.removeChild?.(this.container);
    this.sprite?.destroy?.({ texture: false, baseTexture: false });
    for (const texture of this.frameTextures) texture?.destroy?.(false);
    this.frameTextures = [];
    this.container.destroy?.({ children: true });
  }
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
