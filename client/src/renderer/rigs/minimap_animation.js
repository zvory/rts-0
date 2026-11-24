import { frameStripMovementVisual } from "../frame_strip_movement.js";
import { KIND } from "../../protocol.js";
import { createLiveFrameStrips, liveFrameStripKeyForEntity } from "./frame_strip_routing.js";
import { createLivePngRigAtlases } from "./png_routing.js";
import { createLiveRigDefinitions, LOADED_RIFLEMAN_RIG_KEY } from "./live_routing.js";
import { frameStripFrameIndex, frameStripVisualFacing, frameStripSpriteOffset, frameStripWorldScale } from "./frame_strip_runtime.js";
import { createRigAnimationStage, createRigRenderContext, sampleRigAnimationInto } from "./animation.js";
import { resolvePngSpriteTransform } from "./png_transform.js";
import { supportWeaponSetupVisual } from "../support_weapon_setup_visual.js";

// Injected into the canvas minimap by the host. Production poses drive cached raster layers;
// neither image decoding nor SVG generation depends on an entity, angle, or recoil sample.
export function createMinimapAnimation() {
  const strips = createLiveFrameStrips(), atlases = createLivePngRigAtlases();
  const definitions = createLiveRigDefinitions();
  const motion = new Map();
  const records = new Map(), layers = new Map(), stages = new Map();
  const sprites = new Map([...atlases].map(([kind, atlas]) => {
    // Declared blank atlas cells contain no pixels. Exclude them before preparing images or
    // compiling poses; the visible hull, turret and independently recoiling barrel remain.
    const blankCells = new Set(atlas.grid?.normalization?.blankCells || []);
    return [kind, atlas.sprites.filter(sprite => sprite.frame && !blankCells.has(sprite.sourceCell))
      .sort((a, b) => a.drawOrder - b.drawOrder)];
  }));
  const keyFor = kind => kind === KIND.PANZERFAUST ? LOADED_RIFLEMAN_RIG_KEY : kind;
  let state = {}, now = 0;
  return {
    beginFrame(entities, nextState, nextNow) {
      state = nextState; now = nextNow; layers.clear();
      const ids = new Set(entities.map(entity => entity.id));
      for (const cache of [records, motion]) {
        for (const id of cache.keys()) if (!ids.has(id)) cache.delete(id);
      }
    },
    parts(kind) {
      const strip = strips.get(keyFor(kind));
      if (strip) {
        const parts = Array.from({ length: strip.frameCount }, (_, i) => `frame:${i}`);
        if (kind === KIND.PANZERFAUST) parts.push(...Array.from(
          { length: strips.get(KIND.RIFLEMAN).frameCount }, (_, i) => `unloaded-frame:${i}`));
        if ([KIND.WIND_ADEPT, KIND.WAR_ADEPT, KIND.MAGIC_ADEPT].includes(kind)) {
          const master = strips.get(liveFrameStripKeyForEntity({ kind, daoMaster: true }));
          parts.push(...Array.from({ length: master.frameCount }, (_, i) => `master:frame:${i}`));
        }
        return parts;
      }
      return sprites.get(kind)?.map(sprite => `sprite:${sprite.id}`) ?? null;
    },
    layers(entity) {
      if (layers.has(entity.id)) return layers.get(entity.id);
      const strip = strips.get(liveFrameStripKeyForEntity(entity));
      const entries = sprites.get(entity.kind);
      if (!strip && !entries) return null;
      const setupVisual = supportWeaponSetupVisual(entity, records, now);
      const context = createRigRenderContext(entity, { state, now, setupVisual });
      let result;
      if (strip) {
        context.frameStripMoving = frameStripMovementVisual(entity, state, motion, now).moving;
        const frame = frameStripFrameIndex(strip, entity, context);
        const rotation = frameStripVisualFacing(strip, entity, context, frame);
        const offset = frameStripSpriteOffset(strip, frame, context);
        const reference = frameStripSpriteOffset(strip, strip.idleFrame || 0);
        const width = strip.iconVisibleBounds?.w || strip.frameWidth;
        const height = strip.iconVisibleBounds?.h || strip.frameHeight;
        const scale = 1 / (Math.max(width, height) * 1.05);
        const worldScale = frameStripWorldScale(strip, entity, context, frame);
        const adjusted = !!strip.firingWorldScale;
        const bodyScale = adjusted ? worldScale / strip.worldScale : 1;
        const kick = (offset.x - reference.x) / (adjusted ? strip.worldScale : 1) * scale;
        const step = (offset.y - reference.y) / (adjusted ? strip.worldScale : worldScale) * scale;
        const frameScale = Math.max(strip.frameWidth, strip.frameHeight) * scale * bodyScale;
        const prefix = entity.daoMaster ? "master:frame" : entity.kind === KIND.PANZERFAUST && entity.panzerfaustLoaded !== true
          ? "unloaded-frame" : "frame";
        result = [{ part: `${prefix}:${frame}`, rotation, x: Math.cos(rotation) * kick - Math.sin(rotation) * step,
          y: Math.sin(rotation) * kick + Math.cos(rotation) * step, scaleX: frameScale, scaleY: frameScale, alpha: 1 }];
      } else {
        let stage = stages.get(entity.kind);
        if (!stage) {
          stage = createRigAnimationStage(definitions.get(entity.kind), {
            includeParts: new Set(entries.map(sprite => sprite.animationPart)),
          });
          stages.set(entity.kind, stage);
        }
        const pose = sampleRigAnimationInto(stage, entity, context);
        result = [];
        for (const sprite of entries) {
          const part = pose.parts[sprite.animationPart];
          if (!part?.visible || part.alpha <= 0) continue;
          const frame = sprite.frame;
          const transform = resolvePngSpriteTransform(part, frame, sprite, {}, context);
          const px = frame.pixelsPerUnitX || frame.pixelsPerUnit || 1;
          const py = frame.pixelsPerUnitY || frame.pixelsPerUnit || 1;
          const dx = (transform.pivotX - frame.originX) * transform.scaleX;
          const dy = (transform.pivotY - frame.originY) * transform.scaleY;
          const c = Math.cos(transform.rotation), s = Math.sin(transform.rotation);
          // Same padded 128-unit origin and 1.8 size compensation as articulated minimap icons.
          const scale = 1.8 / (128 * 1.05);
          result.push({ part: `sprite:${sprite.id}`, rotation: transform.rotation,
            x: (transform.x - dx * c + dy * s) * scale,
            y: (transform.y - dx * s - dy * c) * scale,
            scaleX: transform.scaleX * px, scaleY: transform.scaleY * py,
            alpha: part.alpha });
        }
      }
      layers.set(entity.id, result);
      return result;
    },
    destroy() { motion.clear(); records.clear(); layers.clear(); stages.clear(); },
  };
}
