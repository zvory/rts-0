import { teamPaintTextureFor } from "./team_paint.js";
import { updateItemRadiance, destroyItemRadiance } from "../item_radiance.js";
import { goldenAttackTrailFrame } from "./golden_attack_strip.js";
import { SETUP, STATE } from "../../protocol.js";
import { hexToInt, lightenColor } from "../shared.js";
import { rigContainerScale } from "./animation.js";
import { applyWorldYDepth } from "../world_y_depth.js";

export function renderFrameStripUnit(renderer, entity, strip, texture, options = {}) {
  if (!strip || !texture || !options.poolName || !options.layerName) return null;
  const pool = renderer._liveRigPools?.[options.poolName];
  if (!pool) return null;
  const renderContext = options.renderContext || {};
  const sourceTexture = texture;
  if (strip.teamPaint) texture = teamPaintTextureFor(texture, renderContext.teamColor);
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
      renderer._rigPixiFactory ?? createDefaultFrameStripPixiFactory(),
      options.silhouetteShadow === true,
      sourceTexture,
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
    shadowOffset: options.shadowOffset,
    diagnostics: (label, amount = 1) => renderer._recordRenderDiagnostic?.(label, amount),
  });
  return instance;
}

export function frameStripFrameIndex(strip, entity, nowOrContext = 0) {
  const idleFrame = validFrame(strip, strip?.idleFrame ?? 0);
  const renderContext = typeof nowOrContext === "object" && nowOrContext !== null ? nowOrContext : {};
  const now = typeof nowOrContext === "number" ? nowOrContext : renderContext.now;
  if (strip?.directional) {
    const moving = frameStripEntityIsMoving(entity, renderContext);
    const firing = strip.directional.firingFrames > 0 && finite(renderContext.recoilProgress, 0) > 0
      && (!strip.firingWeaponKinds?.length || strip.firingWeaponKinds.includes(renderContext.recoilWeaponKind));
    const facing = (firing || !moving) && Number.isFinite(entity?.weaponFacing)
      ? entity.weaponFacing : finite(entity?.facing, 0);
    const count = Math.max(1, Math.trunc(finite(strip.directional.directionCount, strip.frameCount)));
    const turn = (facing - finite(strip.directional.firstFacing, 0)) / (2 * Math.PI);
    const direction = ((Math.round(turn * count) % count) + count) % count;
    if (firing) {
      const phase = Math.min(strip.directional.firingFrames - 1,
        Math.floor(clamp01(finite(renderContext.recoilPhase, 0)) * strip.directional.firingFrames));
      return validFrame(strip, strip.directional.firingStart + direction * strip.directional.firingFrames + phase);
    }
    const poses = Math.max(0, Math.trunc(finite(strip.directional.runFrames, 0)));
    if (moving && poses > 0) {
      const phase = Math.floor(Math.max(0, finite(now, 0)) * finite(strip.fps, 10) / 1000) % poses;
      return validFrame(strip, finite(strip.directional.runStart, count) + direction * poses + phase);
    }
    return direction;
  }
  const setupFrame = frameStripSetupFrameIndex(strip, entity, renderContext, idleFrame);
  if (setupFrame != null) return setupFrame;
  const windupFrame = frameStripWindupFrameIndex(strip, entity, idleFrame);
  if (windupFrame != null) return windupFrame;
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

function frameStripWindupFrameIndex(strip, entity, fallback = 0) {
  const frames = validFrameList(strip, strip?.windupFrames);
  const progressField = strip?.windupProgressField;
  const rawProgress = typeof progressField === "string" ? entity?.[progressField] : null;
  if (frames.length === 0 || !Number.isFinite(rawProgress)) return null;
  const progress = clamp01(rawProgress);
  const index = Math.min(frames.length - 1, Math.floor(progress * frames.length));
  return validFrame(strip, frames[index], fallback);
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
  const firingWeaponKinds = Array.isArray(strip?.firingWeaponKinds) ? strip.firingWeaponKinds : [];
  if (
    firingWeaponKinds.length > 0 &&
    !firingWeaponKinds.includes(renderContext.recoilWeaponKind)
  ) {
    return null;
  }
  if (clamp01(finite(renderContext.recoilProgress, 0)) <= 0) return null;
  const phase = clamp01(finite(renderContext.recoilPhase, 0));
  const holdPhase = finite(strip?.firingFrameHoldPhase, 0);
  const framePhase = holdPhase > 0 && holdPhase < 1 ? phase / holdPhase : phase;
  if (holdPhase > 0 && holdPhase < 1 && phase >= holdPhase) return null;
  const index = Math.min(firingFrames.length - 1, Math.floor(framePhase * firingFrames.length));
  return validFrame(strip, firingFrames[index], fallback);
}

export function frameStripVisualFacing(stripOrEntity, maybeEntity = null, renderContext = null, frameIndex = null) {
  const strip = maybeEntity ? stripOrEntity : null;
  // Oblique directional art already contains the view of the upright body.
  if (strip?.directional) return 0;
  const entity = maybeEntity ?? stripOrEntity;
  const resolvedFrame = frameIndex ?? frameStripFrameIndex(strip, entity, renderContext ?? {});
  const facingOffset = frameStripFacingOffset(strip, resolvedFrame);
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
  const firingFrames = validFrameList(strip, strip?.firingFrames);
  if (firingFrames.includes(resolvedFrame) && Number.isFinite(entity?.weaponFacing)) {
    return entity.weaponFacing + facingOffset;
  }
  if (frameStripUsesMovementFrames(strip, entity, renderContext) && Number.isFinite(entity?.facing)) {
    return entity.facing + finite(strip?.movementFacingOffset, facingOffset);
  }
  if (!moving && Number.isFinite(entity?.weaponFacing)) return entity.weaponFacing + facingOffset;
  return finite(entity?.facing, 0) + facingOffset;
}

function frameStripFacingOffset(strip, frameIndex) {
  const baseOffset = finite(strip?.facingOffset, 0);
  const firingFrames = validFrameList(strip, strip?.firingFrames);
  if (firingFrames.includes(frameIndex)) return finite(strip?.firingFacingOffset, baseOffset);
  return baseOffset;
}

export function frameStripWorldScale(strip, entity, renderContext = null, frameIndex = null) {
  const baseScale = Math.max(0.01, finite(strip?.worldScale, 1));
  if (strip?.firingWorldScale && strip.firingFrames?.includes(
    frameIndex ?? frameStripFrameIndex(strip, entity, renderContext ?? {}))) {
    return Math.max(0.01, finite(strip.firingWorldScale, baseScale));
  }
  if (frameStripUsesMovementFrames(strip, entity, renderContext)) {
    return Math.max(0.01, finite(strip?.movementWorldScale, baseScale));
  }
  return baseScale;
}

export function frameStripSpriteOffset(strip, frameIndex, renderContext = {}) {
  const defaultForwardPx = finite(strip?.originForwardPx, 0);
  const windupFrames = validFrameList(strip, strip?.windupFrames);
  const forwardPx = windupFrames.includes(frameIndex)
    ? finite(strip?.windupOriginForwardPx, defaultForwardPx)
    : defaultForwardPx;
  const firingPose = strip?.firingFrames?.includes(frameIndex);
  const poseForwardPx = firingPose ? finite(strip?.firingOriginForwardPx, forwardPx) : forwardPx;
  const lateralPx = firingPose
    ? finite(strip?.firingOriginLateralPx, finite(strip?.originLateralPx, 0))
    : finite(strip?.originLateralPx, 0);
  const firingFrames = validFrameList(strip, strip?.firingFrames);
  const recoilPx = firingFrames.includes(frameIndex) ? Math.max(0, finite(strip?.firingRecoilPx, 0)) : 0;
  // The overhead attack art faces local +Y. Move the whole pose through anticipation,
  // drive, extension and recovery without changing the authoritative entity position.
  let lunge = 0;
  if (strip?.firingLungePx > 0 && firingFrames.includes(frameIndex) && renderContext.recoilProgress > 0) {
    const holdPhase = finite(strip?.firingFrameHoldPhase, 0);
    const phase = clamp01(finite(renderContext.recoilPhase, 0) / (holdPhase > 0 && holdPhase < 1 ? holdPhase : 1));
    let start, end, from, to;
    if (phase < 0.2) { start = 0; end = 0.2; from = 0; to = -0.18; }
    else if (phase < 0.5) { start = 0.2; end = 0.5; from = -0.18; to = 1; }
    else if (phase < 0.7) { start = 0.5; end = 0.7; from = 1; to = 1; }
    else { start = 0.7; end = 1; from = 1; to = 0; }
    const t = (phase - start) / (end - start);
    lunge = (from + (to - from) * t * t * (3 - 2 * t)) * Math.max(0, finite(strip?.firingLungePx, 0));
  }
  return { x: poseForwardPx - recoilPx, y: lateralPx + lunge };
}

function createDefaultFrameStripPixiFactory(pixi = globalThis.PIXI) {
  return {
    createContainer: () => new pixi.Container(),
    createRectangle: (x, y, width, height) => new pixi.Rectangle(x, y, width, height),
    createTexture: (source, rectangle) => new pixi.Texture({ source, frame: rectangle }),
    createSprite: (spriteTexture) => new pixi.Sprite(spriteTexture),
    createGraphics: () => new pixi.Graphics(),
    createShadowBlur: () => new pixi.BlurFilter({ strength: 2, quality: 4, resolution: 2 }),
  };
}

class FrameStripUnitInstance {
  constructor(kind, strip, texture, pixiFactory, silhouetteShadow = false, sourceTexture = texture) {
    this.kind = kind;
    this.strip = strip;
    this.texture = texture;
    this._pixiFactory = pixiFactory;
    this.container = pixiFactory.createContainer();
    this.frameTextures = [];
    this.itemGlowFrames = [];
    this._frameIndex = -1;
    this._destroyed = false;
    this.silhouetteShadow = silhouetteShadow;
    if (silhouetteShadow) {
      this.shadowBlur = pixiFactory.createShadowBlur();
      this.container.filters = [this.shadowBlur];
    }

    const frameWidth = Math.max(1, finite(strip.frameWidth, 1));
    const frameHeight = Math.max(1, finite(strip.frameHeight, 1));
    const frameCount = Math.max(1, Math.trunc(finite(strip.frameCount, 1)));
    const baseTexture = texture.source;
    const columns = Math.max(1, Math.trunc(finite(strip.frameColumns, frameCount)));
    for (let i = 0; i < frameCount; i += 1) {
      const rectangle = pixiFactory.createRectangle((i % columns) * frameWidth, Math.floor(i / columns) * frameHeight, frameWidth, frameHeight);
      this.frameTextures.push(pixiFactory.createTexture(baseTexture, rectangle));
      if (strip.teamPaint && !silhouetteShadow) {
        this.itemGlowFrames.push(pixiFactory.createTexture(sourceTexture.source, rectangle));
      }
    }

    if (strip.groundContactShadow && !silhouetteShadow) {
      this.groundShadow = pixiFactory.createGraphics();
      this.groundShadow.ellipse(0, 0, 7, 2.5).fill({ color: 0x000000, alpha: 0.28 });
      this.container.addChild(this.groundShadow);
    }
    this.sprite = pixiFactory.createSprite(this.frameTextures[0]);
    this.sprite.anchor?.set?.(finite(strip.anchorX, 0.5), finite(strip.anchorY, 0.5));
    this.sprite.position?.set?.(0, 0);
    this.container.addChild(this.sprite);
    this.trailTextures = [];
    if (strip.attackTrail && !silhouetteShadow) {
      for (let i = 0; i < strip.attackTrail.frameCount; i += 1) {
        const rect = pixiFactory.createRectangle(i * frameWidth, strip.attackTrail.row * frameHeight, frameWidth, frameHeight);
        this.trailTextures.push(pixiFactory.createTexture(baseTexture, rect));
      }
      this.trailSprite = pixiFactory.createSprite(this.trailTextures[0]);
      this.trailSprite.anchor?.set?.(0.5, 0.5);
      this.trailSprite.visible = false;
      this.container.addChild(this.trailSprite);
    }
  }

  matchesFrameStripUnit(kind, strip, texture) {
    return this.kind === kind && this.strip === strip && this.texture === texture;
  }

  update(entity, renderContext = {}, options = {}) {
    if (this._destroyed) return;
    this.container.visible = true;
    this.container.alpha = (renderContext.shotRevealAlpha ?? options.alpha ?? 1)
      * (this.silhouetteShadow ? 0.25 : 1);
    setPoint(this.container.position,
      finite(entity.x, 0) + finite(options.shadowOffset?.x, 0),
      finite(entity.y, 0) + finite(options.shadowOffset?.y, 0));
    applyWorldYDepth(this.container, entity);
    const scale = rigContainerScale(renderContext);
    setPoint(this.container.scale, scale, scale);
    const frameIndex = frameStripFrameIndex(this.strip, entity, renderContext);
    this.container.rotation = frameStripVisualFacing(this.strip, entity, renderContext, frameIndex);
    if (frameIndex !== this._frameIndex) {
      this.sprite.texture = this.frameTextures[frameIndex] ?? this.frameTextures[0];
      this.itemGlowTexture = this.itemGlowFrames[frameIndex] ?? null;
      this._frameIndex = frameIndex;
    }

    const worldScale = frameStripWorldScale(this.strip, entity, renderContext, frameIndex);
    setPoint(this.sprite.scale, worldScale, worldScale);
    const spriteOffset = frameStripSpriteOffset(this.strip, frameIndex, renderContext);
    setPoint(this.sprite.position, spriteOffset.x, spriteOffset.y);
    this.sprite.alpha = 1;
    this.sprite.tint = this.silhouetteShadow ? 0x000000 : this.strip.teamPaint ? 0xffffff : tintForSlot(this.strip.tintSlot, renderContext);
    this.sprite.visible = true;
    updateItemRadiance(this, entity, renderContext.now || 0);
    if (this.trailSprite) {
      const trailFrame = goldenAttackTrailFrame(this.strip, renderContext);
      this.trailSprite.visible = trailFrame != null;
      if (trailFrame != null) {
        this.trailSprite.texture = this.trailTextures[trailFrame];
        const effectScale = this.strip.attackTrail.scale || 1;
        const center = this.strip.attackTrail.centers?.[trailFrame] || [this.strip.frameWidth / 2, this.strip.frameHeight / 2];
        const sourceCenter = this.strip.attackTrail.sourceCenters?.[trailFrame] || center;
        setPoint(this.trailSprite.scale, worldScale * effectScale, worldScale * effectScale);
        setPoint(this.trailSprite.position,
          spriteOffset.x + ((center[0] - this.strip.frameWidth / 2) - (sourceCenter[0] - this.strip.frameWidth / 2) * effectScale) * worldScale,
          spriteOffset.y + ((center[1] - this.strip.frameHeight / 2) - (sourceCenter[1] - this.strip.frameHeight / 2) * effectScale) * worldScale);
        this.trailSprite.tint = 0xffffff;
      }
    }
    options.diagnostics?.("renderer.frameStrip.redraw.completed");
  }

  destroy() {
    if (this._destroyed) return;
    this._destroyed = true;
    destroyItemRadiance(this);
    this.shadowBlur?.destroy?.();
    this.container.parent?.removeChild?.(this.container);
    this.sprite?.destroy?.({ texture: false, textureSource: false });
    this.trailSprite?.destroy?.({ texture: false, textureSource: false });
    for (const texture of this.trailTextures) texture?.destroy?.(false);
    this.trailTextures = [];
    for (const texture of this.frameTextures) texture?.destroy?.(false);
    this.frameTextures = [];
    for (const texture of this.itemGlowFrames) texture.destroy?.(false);
    this.itemGlowFrames = [];
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
  if (entity?.state !== STATE.MOVE && entity?.state !== STATE.ATTACK && entity?.state !== STATE.IDLE) return false;
  if (typeof renderContext?.frameStripMoving === "boolean") return renderContext.frameStripMoving;
  if (Number.isFinite(renderContext?.frameStripMovementActivity)) {
    return renderContext.frameStripMovementActivity > 0.01;
  }
  return entity?.state === STATE.MOVE;
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
