import { KIND, isUnit } from "../protocol.js";
import { createWorkerSafeCanvas } from "./raster_primitives.js";

const VEHICLES = new Set([KIND.TANK, KIND.SCOUT_CAR, KIND.COMMAND_CAR,
  KIND.ANTI_TANK_GUN, KIND.ARTILLERY, KIND.ROCKET_LAUNCHER]);

/** Lab-only visual experiment. Bounded mask, no durable or replay authority. */
export class GrassPressureField {
  constructor(map) {
    this.worldWidth = map.width * map.tileSize;
    this.worldHeight = map.height * map.tileSize;
    this.scale = Math.max(4, this.worldWidth / 1024, this.worldHeight / 1024);
    this.width = Math.ceil(this.worldWidth / this.scale);
    this.height = Math.ceil(this.worldHeight / this.scale);
    this.pixels = new Uint8ClampedArray(this.width * this.height * 4);
    this.poses = new Map();
  }

  update(entities) {
    const seen = new Set();
    let changed = false;
    for (const e of entities) {
      if (!isUnit(e.kind) || e.kind === KIND.SCOUT_PLANE || !(e.hp > 0) ||
          e.shotReveal || e.aboveFogReveal || e.visionOnly ||
          !Number.isFinite(e.x) || !Number.isFinite(e.y)) continue;
      seen.add(e.id);
      const previous = this.poses.get(e.id);
      const distance = previous ? Math.hypot(e.x - previous.x, e.y - previous.y) : 0;
      if (!previous || distance > 96) { this.poses.set(e.id, { x:e.x, y:e.y }); continue; }
      if (distance < 2) continue;
      const dx = (e.x - previous.x) / distance, dy = (e.y - previous.y) / distance;
      const vehicle = VEHICLES.has(e.kind);
      const offset = e.kind === KIND.TANK ? 18 : 11;
      for (const side of vehicle ? [-1, 1] : [0]) {
        const ox = -dy * offset * side, oy = dx * offset * side;
        changed = this.stamp(previous.x + ox, previous.y + oy,
          e.x + ox, e.y + oy, vehicle ? 4 : 5, vehicle ? 235 : 165, dx, dy) || changed;
      }
      this.poses.set(e.id, { x:e.x, y:e.y });
    }
    for (const id of this.poses.keys()) if (!seen.has(id)) this.poses.delete(id);
    return changed;
  }

  stamp(ax, ay, bx, by, radius, pressure, dx, dy) {
    const s = this.scale, vx = bx-ax, vy = by-ay, length2 = vx*vx+vy*vy;
    const minX = Math.max(0, Math.floor((Math.min(ax,bx)-radius-s)/s));
    const maxX = Math.min(this.width-1, Math.ceil((Math.max(ax,bx)+radius+s)/s));
    const minY = Math.max(0, Math.floor((Math.min(ay,by)-radius-s)/s));
    const maxY = Math.min(this.height-1, Math.ceil((Math.max(ay,by)+radius+s)/s));
    let changed = false;
    for (let y=minY;y<=maxY;y++) for (let x=minX;x<=maxX;x++) {
      const px=(x+.5)*s, py=(y+.5)*s;
      const t=length2 ? Math.max(0,Math.min(1,((px-ax)*vx+(py-ay)*vy)/length2)) : 0;
      const distance=Math.hypot(px-ax-t*vx,py-ay-t*vy);
      const value=Math.round(pressure*Math.max(0,Math.min(1,(radius+s*.5-distance)/s)));
      const i=(y*this.width+x)*4;
      // Max pressure makes revisiting a path idempotent instead of accumulating endless stamps.
      if (value <= this.pixels[i]) continue;
      this.pixels[i]=value;
      this.pixels[i+1]=Math.round((dx*.5+.5)*255);
      this.pixels[i+2]=Math.round((dy*.5+.5)*255);
      this.pixels[i+3]=255;
      changed=true;
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
