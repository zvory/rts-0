import { ABILITY, ABILITY_OBJECT_KIND, KIND } from "../protocol.js";
import { loadWorkerSafeTexture } from "./raster_primitives.js";

// Consumes only fog-filtered ability objects; the caster need not be visible.
export class ThrowSwordVisuals {
  constructor({ pixi, layer, trackAsset }) {
    this.pixi = pixi;
    this.layer = layer;
    this.sprites = new Map();
    this.texture = null;
    this.destroyed = false;
    trackAsset("throw-sword", loadWorkerSafeTexture(pixi,
      "/assets/effects/throw-sword/sword.png").then(texture => {
      if (this.destroyed) { texture.destroy(true); return null; }
      this.texture = texture;
      return texture;
    }), { kind: KIND.WARRIOR, source: "throwSword" });
  }

  update(objects, now) {
    if (!this.texture || this.destroyed) return;
    const seen = new Set();
    for (const object of objects || []) {
      if (object.ability !== ABILITY.THROW_SWORD || object.kind !== ABILITY_OBJECT_KIND.LINE_PROJECTILE ||
          !Number.isFinite(object.x) || !Number.isFinite(object.y)) continue;
      seen.add(object.id);
      let sprite = this.sprites.get(object.id);
      if (!sprite) {
        sprite = new this.pixi.Sprite(this.texture);
        // Original generated image, including its transparent padding. Painted sword is ~1170 px long.
        sprite.anchor.set(0.505, 0.505);
        sprite.scale.set(64 / 1170);
        this.layer.addChild(sprite);
        this.sprites.set(object.id, sprite);
      }
      sprite.position.set(object.x, object.y);
      sprite.rotation = now * Math.PI * 2 / 240;
    }
    for (const [id, sprite] of this.sprites) {
      if (seen.has(id)) continue;
      sprite.destroy();
      this.sprites.delete(id);
    }
  }

  destroy() {
    this.destroyed = true;
    for (const sprite of this.sprites.values()) sprite.destroy();
    this.sprites.clear();
    this.texture?.destroy(true);
    this.texture = null;
  }
}
