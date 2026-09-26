import { KIND } from "../protocol.js";
import { loadWorkerSafeTexture } from "./raster_primitives.js";
import { lightenColor } from "./shared.js";
import { rocketProjectilePose, drawRocketProjectileTrail } from "./panzerfaust_feedback.js";

export const ROCKET_COUNT = 16;
export const ROCKET_LENGTH = 8.5;

// Mirrors rules::balance::rocket_rack_slot. Coordinates refer to the enlarged truck.
export function rocketRackSlot(index) {
  const column = Math.floor(index / 6);
  const row = index % 6 + (column === 2 ? 1 : 0);
  return { x: -3.5 - column * 9.8, y: -7.9 + row * 2.8 };
}

export const rocketFlightPose = rocketProjectilePose;

export class RocketTruckVisuals {
  constructor({ pixi, unitLayer, projectileLayer, trackAsset }) {
    this.pixi = pixi;
    this.unitLayer = unitLayer;
    this.projectileLayer = projectileLayer;
    this.racks = new Map();
    this.projectiles = new Map();
    this.texture = null;
    this.destroyed = false;
    trackAsset("rocket-truck-projectile", loadWorkerSafeTexture(pixi,
      "/assets/rigs/rocket-truck-preview/rocket.png").then(texture => {
      if (this.destroyed) { texture.destroy(true); return null; }
      this.texture = texture;
      return texture;
    }), { kind: KIND.ROCKET_LAUNCHER, source: "rocketProjectile" });
  }

  sprite() {
    const sprite = new this.pixi.Sprite(this.texture);
    // Preserve the generated PNG unchanged; anchor at the painted rocket's centre.
    sprite.anchor.set(887 / 1774, 479 / 887);
    sprite.scale.set(ROCKET_LENGTH / 1535);
    return sprite;
  }

  update(entities, shells, colors, now, graphics) {
    if (!this.texture || this.destroyed) return;
    const seen = new Set();
    for (const entity of entities) {
      if (entity.kind !== KIND.ROCKET_LAUNCHER) continue;
      seen.add(entity.id);
      let rack = this.racks.get(entity.id);
      if (!rack) {
        rack = new this.pixi.Container();
        for (let index = 0; index < ROCKET_COUNT; index++) {
          const sprite = this.sprite();
          const slot = rocketRackSlot(index);
          sprite.position.set(slot.x, slot.y);
          rack.addChild(sprite);
        }
        this.unitLayer.addChild(rack);
        this.racks.set(entity.id, rack);
      }
      rack.position.set(entity.x, entity.y);
      rack.rotation = entity.facing || 0;
      rack.zIndex = entity.y + 0.001;
      const remaining = Math.max(0, Math.min(ROCKET_COUNT, entity.rocketRackCount ?? ROCKET_COUNT));
      const tint = lightenColor(colors.get(entity.owner) ?? 0x6d89b8, 0.18);
      for (let index = 0; index < ROCKET_COUNT; index++) {
        rack.children[index].visible = index >= ROCKET_COUNT - remaining;
        rack.children[index].tint = tint;
      }
    }
    for (const [id, rack] of this.racks) {
      if (seen.has(id)) continue;
      rack.destroy({ children: true });
      this.racks.delete(id);
    }

    const flying = new Set();
    for (const shot of shells) {
      if (!shot.rocket || now - shot.createdAt >= shot.durationMs) continue;
      const key = `${shot.from}:${shot.createdAt}:${shot.seed}`;
      flying.add(key);
      let sprite = this.projectiles.get(key);
      if (!sprite) {
        sprite = this.sprite();
        this.projectileLayer.addChild(sprite);
        this.projectiles.set(key, sprite);
      }
      const pose = rocketFlightPose(shot, now);
      sprite.position.set(pose.x, pose.y);
      sprite.rotation = pose.facing;
      sprite.tint = lightenColor(colors.get(shot.owner) ?? 0x6d89b8, 0.18);
      sprite.alpha = pose.travelFade;
      drawRocketProjectileTrail(graphics, pose);
    }
    for (const [key, sprite] of this.projectiles) {
      if (flying.has(key)) continue;
      sprite.destroy();
      this.projectiles.delete(key);
    }
  }

  destroy() {
    this.destroyed = true;
    for (const rack of this.racks.values()) rack.destroy({ children: true });
    for (const sprite of this.projectiles.values()) sprite.destroy();
    this.racks.clear();
    this.projectiles.clear();
    this.texture?.destroy(true);
    this.texture = null;
  }
}
