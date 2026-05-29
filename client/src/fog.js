// Fog of war — client-side visibility overlay. See DESIGN.md §1, §4.1, §4.2.
//
// The server already withholds enemy/neutral entities the player cannot see, so
// this overlay is purely cosmetic: it makes the map look correctly fogged. Two
// row-major tile grids are maintained:
//   - `visibleGrid`  : 1 where an own entity can currently see the tile (recomputed
//                      from scratch each `update`).
//   - `exploredGrid` : 1 where a tile has *ever* been visible (cumulative, never cleared).
//
// A tile is rendered clear when visible, dimmed when explored-but-not-visible, and
// solid dark when never explored (the renderer reads the grids directly).

import { STATS } from "./config.js";

// Sight (in tiles) used when an entity kind has no `sight` entry in STATS — keeps a
// stray/unknown entity from punching a zero-radius hole in the fog.
const DEFAULT_SIGHT_TILES = 4;

export class Fog {
  /**
   * @param {number} mapWidth map width in tiles
   * @param {number} mapHeight map height in tiles
   */
  constructor(mapWidth, mapHeight) {
    /** Map width in tiles. */
    this.width = mapWidth;
    /** Map height in tiles. */
    this.height = mapHeight;
    /** 1 = currently visible. Length width*height, row-major. @type {Uint8Array} */
    this.visibleGrid = new Uint8Array(mapWidth * mapHeight);
    /** 1 = ever explored (cumulative). Length width*height, row-major. @type {Uint8Array} */
    this.exploredGrid = new Uint8Array(mapWidth * mapHeight);
  }

  /**
   * Recompute visibility for this frame from the player's own entities.
   *
   * Clears `visibleGrid`, then for each owned entity stamps a filled circle of its
   * sight radius (config `STATS[kind].sight`, in tiles) into both `visibleGrid` and
   * `exploredGrid`. Explored is cumulative and is never cleared.
   *
   * @param {Array<{kind:string,x:number,y:number}>} ownEntities entities owned by this player (world px centers)
   * @param {number} tileSize world px per tile
   */
  update(ownEntities, tileSize) {
    this.visibleGrid.fill(0);
    if (!ownEntities || !tileSize) return;

    for (const e of ownEntities) {
      const stat = STATS[e.kind];
      const sight = (stat && stat.sight) || DEFAULT_SIGHT_TILES;
      const cx = e.x / tileSize;
      const cy = e.y / tileSize;
      this._stampCircle(cx, cy, sight);
    }
  }

  /**
   * Mark a filled circle of `radius` tiles centered on tile-space (`cx`, `cy`) as
   * visible and explored. Uses a squared-distance test so the reveal is round.
   * @private
   */
  _stampCircle(cx, cy, radius) {
    const r2 = radius * radius;
    const minTx = Math.max(0, Math.floor(cx - radius));
    const maxTx = Math.min(this.width - 1, Math.ceil(cx + radius));
    const minTy = Math.max(0, Math.floor(cy - radius));
    const maxTy = Math.min(this.height - 1, Math.ceil(cy + radius));

    for (let ty = minTy; ty <= maxTy; ty++) {
      // Compare against tile centers so the disc is symmetric around the entity.
      const dy = ty + 0.5 - cy;
      const rowBase = ty * this.width;
      for (let tx = minTx; tx <= maxTx; tx++) {
        const dx = tx + 0.5 - cx;
        if (dx * dx + dy * dy <= r2) {
          const i = rowBase + tx;
          this.visibleGrid[i] = 1;
          this.exploredGrid[i] = 1;
        }
      }
    }
  }

  /**
   * @param {number} tileX
   * @param {number} tileY
   * @returns {boolean} true if the tile is visible this frame
   */
  isVisible(tileX, tileY) {
    if (tileX < 0 || tileY < 0 || tileX >= this.width || tileY >= this.height) return false;
    return this.visibleGrid[tileY * this.width + tileX] === 1;
  }

  /**
   * @param {number} tileX
   * @param {number} tileY
   * @returns {boolean} true if the tile has ever been explored
   */
  isExplored(tileX, tileY) {
    if (tileX < 0 || tileY < 0 || tileX >= this.width || tileY >= this.height) return false;
    return this.exploredGrid[tileY * this.width + tileX] === 1;
  }
}
