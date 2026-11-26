import { KIND, WEAPON_KIND } from "../protocol.js";
import { loadWorkerSafeTexture } from "./raster_primitives.js";
import { gfxCircle, gfxFill, gfxNoFill, gfxStroke, gfxStrokeLine } from "./native_graphics.js";

// Orb damage resolves server-side after this release and flight time. Arrows remain immediate.
// Release follows the first 100 ms pose of the four-frame, 400 ms attack cycle.
export const DISCIPLE_RELEASE_DELAY_MS = 100;
export const DISCIPLE_FLIGHT_TTL_MS = 600;
export const DISCIPLE_PROJECTILE_LIMIT = 256;
const SPEED_PX_PER_MS = 0.65;

export function isDiscipleProjectile(weaponKind) {
  return weaponKind === WEAPON_KIND.DISCIPLE_ARROW || weaponKind === WEAPON_KIND.DISCIPLE_ORB;
}

export function sampleDiscipleFlight(flash, now) {
  if (!isDiscipleProjectile(flash.weaponKind)) return null;
  const from = flash.daoOrigin;
  const to = flash.targetPos;
  if (!Number.isFinite(from?.x) || !Number.isFinite(from?.y) ||
      !Number.isFinite(to?.x) || !Number.isFinite(to?.y) ||
      !Number.isFinite(flash.createdAt) || !Number.isFinite(now)) return null;
  const elapsed = now - flash.createdAt - DISCIPLE_RELEASE_DELAY_MS;
  const dx = to.x - from.x, dy = to.y - from.y;
  const distance = Math.hypot(dx, dy);
  const duration = Math.max(60, distance / SPEED_PX_PER_MS);
  if (elapsed < 0 || elapsed > duration || now - flash.createdAt > DISCIPLE_FLIGHT_TTL_MS) return null;
  const progress = elapsed / duration;
  return {
    x: from.x + dx * progress,
    y: from.y + dy * progress,
    angle: Math.atan2(dy, dx),
    shimmer: 0.5 + 0.5 * Math.sin(elapsed * 0.04 + flash.from),
  };
}

export class DiscipleProjectileVisuals {
  constructor({ pixi, layer, trackAsset, loadTexture = loadWorkerSafeTexture }) {
    this.pixi = pixi;
    this.layer = layer;
    this.sprites = [];
    this.texture = null;
    this.destroyed = false;
    trackAsset("disciple-arrow", loadTexture(pixi, "/assets/effects/disciple-arrow/arrow.png")
      .then(texture => {
        if (this.destroyed) { texture.destroy(true); return null; }
        this.texture = texture;
        return texture;
      }), { kind: KIND.WARRIOR, source: "discipleArrow" });
  }

  update(flashes, now, graphics, colorByOwner = null) {
    if (this.destroyed) return;
    let arrows = 0;
    let flights = 0;
    for (const flash of flashes || []) {
      const flight = sampleDiscipleFlight(flash, now);
      if (!flight) continue;
      if (flights++ >= DISCIPLE_PROJECTILE_LIMIT) break;
      if (flash.weaponKind === WEAPON_KIND.DISCIPLE_ORB) {
        drawOrb(graphics, flight);
        continue;
      }
      if (!this.texture) continue;
      let sprite = this.sprites[arrows];
      if (!sprite) {
        sprite = new this.pixi.Sprite(this.texture);
        sprite.anchor.set(0.5);
        sprite.width = 22;
        // Widen across the shaft while preserving its length and flight timing.
        sprite.height = 22 * this.texture.height / this.texture.width * 3;
        this.layer.addChild(sprite);
        this.sprites.push(sprite);
      }
      sprite.tint = colorByOwner?.get(flash.daoOwner) ?? 0xffffff;
      sprite.visible = true;
      sprite.position.set(flight.x, flight.y);
      sprite.rotation = flight.angle;
      arrows++;
    }
    for (let i = arrows; i < this.sprites.length; i++) this.sprites[i].visible = false;
  }

  destroy() {
    this.destroyed = true;
    for (const sprite of this.sprites) sprite.destroy();
    this.sprites.length = 0;
    this.texture?.destroy(true);
    this.texture = null;
  }
}

function drawOrb(g, { x, y, shimmer }) {
  // Fixed work per ball, drawn into the shared feedback batch: no emitter,
  // per-orb display objects, filters, blur passes, or persistent particle state.
  gfxStroke(g, 0);
  gfxFill(g, 0x9edfff, 0.18 + shimmer * 0.12);
  gfxCircle(g, x, y, 5.5);
  gfxFill(g, 0xc6f1ff, 0.8);
  gfxCircle(g, x, y, 3.5);
  gfxFill(g, 0xffffff, 0.95);
  gfxCircle(g, x - 0.6, y - 0.6, 1.8);
  gfxNoFill(g);
  const offset = 3.5 + shimmer;
  gfxStrokeLine(g, x + offset - 1, y - offset, x + offset + 1, y - offset, 0.7, 0xffffff, shimmer);
  gfxStrokeLine(g, x + offset, y - offset - 1, x + offset, y - offset + 1, 0.7, 0xffffff, shimmer);
}
