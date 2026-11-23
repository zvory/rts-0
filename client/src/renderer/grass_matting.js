import { KIND, isUnit } from "../protocol.js";
import { createWorkerSafeCanvas } from "./raster_primitives.js";

const VEHICLES = new Set([KIND.TANK, KIND.SCOUT_CAR, KIND.COMMAND_CAR,
  KIND.MORTAR_TEAM, KIND.ANTI_TANK_GUN, KIND.ARTILLERY, KIND.ROCKET_LAUNCHER]);

// A center-line passage integrates to 1/20 of full wear. Spatial sampling makes
// traffic depend on distance travelled, not render FPS or time spent standing.
export const INFANTRY_PATH_PASSES = 20;
const SAMPLE_STEP = 4;
const RADIUS = 28;
const SAMPLE_CREDIT = 65535 * SAMPLE_STEP / (INFANTRY_PATH_PASSES * RADIUS * 16 / 15);

/** Lab-only visual experiment. Bounded mask, no durable or replay authority. */
export class GrassPressureField {
  constructor(map) {
    this.worldWidth = map.width * map.tileSize;
    this.worldHeight = map.height * map.tileSize;
    this.scale = Math.max(4, this.worldWidth / 1024, this.worldHeight / 1024);
    this.width = Math.ceil(this.worldWidth / this.scale);
    this.height = Math.ceil(this.worldHeight / this.scale);
    this.pixels = new Uint8ClampedArray(this.width * this.height * 4);
    // R is shader-visible wear; GB retain a 16-bit traffic counter. Opaque alpha
    // prevents canvas premultiplication from changing this data texture.
    for (let i = 3; i < this.pixels.length; i += 4) this.pixels[i] = 255;
    this.poses = new Map();
  }

  update(entities) {
    const seen = new Set();
    let changed = false;
    for (const e of entities) {
      if (!isUnit(e.kind) || VEHICLES.has(e.kind) || e.kind === KIND.SCOUT_PLANE || !(e.hp > 0) ||
          e.shotReveal || e.aboveFogReveal || e.visionOnly ||
          !Number.isFinite(e.x) || !Number.isFinite(e.y)) continue;
      seen.add(e.id);
      const previous = this.poses.get(e.id);
      const distance = previous ? Math.hypot(e.x - previous.x, e.y - previous.y) : 0;
      if (!previous || distance > 96) {
        this.poses.set(e.id, { x:e.x, y:e.y, untilSample:SAMPLE_STEP / 2 });
        continue;
      }
      if (distance < .0001) continue;
      const dx = (e.x - previous.x) / distance, dy = (e.y - previous.y) / distance;
      let travel = previous.untilSample;
      for (; travel <= distance; travel += SAMPLE_STEP) {
        changed = this.stamp(previous.x + dx * travel, previous.y + dy * travel) || changed;
      }
      this.poses.set(e.id, { x:e.x, y:e.y, untilSample:travel - distance });
    }
    for (const id of this.poses.keys()) if (!seen.has(id)) this.poses.delete(id);
    return changed;
  }

  stamp(cx, cy) {
    const s = this.scale;
    const minX = Math.max(0, Math.floor((cx - RADIUS) / s));
    const maxX = Math.min(this.width - 1, Math.floor((cx + RADIUS) / s));
    const minY = Math.max(0, Math.floor((cy - RADIUS) / s));
    const maxY = Math.min(this.height - 1, Math.floor((cy + RADIUS) / s));
    let changed = false;
    for (let y = minY; y <= maxY; y++) for (let x = minX; x <= maxX; x++) {
      const dx = (x + .5) * s - cx, dy = (y + .5) * s - cy;
      const kernel = Math.max(0, 1 - (dx * dx + dy * dy) / (RADIUS * RADIUS));
      const credit = Math.round(SAMPLE_CREDIT * kernel * kernel);
      if (!credit) continue;
      const i = (y * this.width + x) * 4;
      const old = this.pixels[i + 1] * 256 + this.pixels[i + 2];
      const next = Math.min(65535, old + credit);
      if (next === old) continue;
      this.pixels[i + 1] = next >>> 8;
      this.pixels[i + 2] = next & 255;
      const visible = Math.round(next / 257);
      changed ||= visible !== this.pixels[i];
      this.pixels[i] = visible;
    }
    return changed;
  }
}

export class GrassMattingMask {
  constructor(pixi, map) {
    this.field = new GrassPressureField(map);
    this.canvas = createWorkerSafeCanvas(this.field.width, this.field.height);
    this.ctx = this.canvas.getContext("2d");
    this.image = this.ctx.createImageData(this.field.width, this.field.height);
    this.image.data.set(this.field.pixels);
    this.ctx.putImageData(this.image, 0, 0);
    this.texture = pixi.Texture.from(this.canvas);
    this.texture.source.scaleMode = "linear";
  }
  update(entities) {
    if (!this.field.update(entities)) return;
    this.image.data.set(this.field.pixels);
    this.ctx.putImageData(this.image,0,0);
    this.texture.source.update();
  }
  destroy() { this.field.poses.clear(); this.texture.destroy(true); }
}
