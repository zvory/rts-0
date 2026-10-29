import { COLORS, MINIMAP_FOG_EXPLORED_ALPHA, MINIMAP_FOG_UNEXPLORED_ALPHA } from "./config.js";
import { PASSABLE } from "./protocol.js";

// One texel per authoritative fog tile. The GPU scales this nearest-filtered texture to the map;
// visibility changes transfer kilobytes instead of reading back the full backing canvas.
export class MinimapGpuFog {
  constructor(assets, createCanvas) { this.assets = assets; this.createCanvas = createCanvas; }
  draw(context, minimap, map, fog) {
    if (fog.revealAll) return;
    const visible = minimap._fogGridForMap(fog.visibleGrid, map);
    const explored = minimap._fogGridForMap(fog.exploredGrid, map);
    const key = `${map.width}:${map.height}:${fog.revision}:${fog.revealAll}`;
    if (key !== this.key || this.terrain !== map.terrain || this.visible !== visible || this.explored !== explored) {
      this.canvas ||= this.createCanvas();
      if (this.canvas.width !== map.width) this.canvas.width = map.width;
      if (this.canvas.height !== map.height) this.canvas.height = map.height;
      const pixels = new Uint8ClampedArray(map.width * map.height * 4);
      for (let y = 0; y < map.height; y++) for (let x = 0; x < map.width; x++) {
        const index = y * map.width + x;
        if (visible ? visible[index] === 1 : fog.isVisible(x, y)) continue;
        const seen = explored ? explored[index] === 1 : fog.isExplored(x, y);
        const color = seen ? COLORS.fogExplored : COLORS.fogUnexplored;
        pixels[index * 4] = (color >> 16) & 255;
        pixels[index * 4 + 1] = (color >> 8) & 255;
        pixels[index * 4 + 2] = color & 255;
        pixels[index * 4 + 3] = Math.round(255 * (seen ? MINIMAP_FOG_EXPLORED_ALPHA : MINIMAP_FOG_UNEXPLORED_ALPHA)
          * (PASSABLE[map.terrain[index]] !== true ? 0.56 : 1));
      }
      this.canvas.getContext("2d").putImageData(new ImageData(pixels, map.width, map.height), 0, 0);
      this.key = key; this.terrain = map.terrain; this.visible = visible; this.explored = explored;
    }
    const id = this.assets.image(this.canvas, { smooth: false });
    context.commands.push({ type: "image", image: id,
      rect: [minimap._offX, minimap._offY, map.width * map.tileSize * minimap._scale, map.height * map.tileSize * minimap._scale],
      matrix: [1, 0, 0, 1, 0, 0], alpha: 1 });
  }
}
