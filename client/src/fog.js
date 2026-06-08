// Fog of war — client-side visibility overlay. See docs/design/architecture.md and
// docs/design/client-ui.md §4.1, §4.2.
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
import { TERRAIN } from "./protocol.js";

// Sight (in tiles) used when an entity kind has no `sight` entry in STATS — keeps a
// stray/unknown entity from punching a zero-radius hole in the fog.
const DEFAULT_SIGHT_TILES = 4;

export class Fog {
  /**
   * @param {number} mapWidth map width in tiles
   * @param {number} mapHeight map height in tiles
   * @param {Uint8Array|number[]|null} terrain row-major terrain codes
   */
  constructor(mapWidth, mapHeight, terrain = null) {
    /** Map width in tiles. */
    this.width = mapWidth;
    /** Map height in tiles. */
    this.height = mapHeight;
    /** 1 = currently visible. Length width*height, row-major. @type {Uint8Array} */
    this.visibleGrid = new Uint8Array(mapWidth * mapHeight);
    /** 1 = ever explored (cumulative). Length width*height, row-major. @type {Uint8Array} */
    this.exploredGrid = new Uint8Array(mapWidth * mapHeight);
    this.terrain = terrain;
    this.revealAll = false;
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
   * @param {ArrayLike<number>|null} serverVisibleTiles server-authoritative current visibility
   */
  update(ownEntities, tileSize, serverVisibleTiles = null) {
    if (this.revealAll) {
      this.visibleGrid.fill(1);
      this.exploredGrid.fill(1);
      return;
    }
    if (serverVisibleTiles && serverVisibleTiles.length === this.visibleGrid.length) {
      for (let i = 0; i < this.visibleGrid.length; i++) {
        const visible = serverVisibleTiles[i] ? 1 : 0;
        this.visibleGrid[i] = visible;
        if (visible) this.exploredGrid[i] = 1;
      }
      return;
    }
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
        if (dx * dx + dy * dy <= r2 && this._tileVisibleFrom(cx, cy, tx, ty)) {
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
    if (this.revealAll) return true;
    if (tileX < 0 || tileY < 0 || tileX >= this.width || tileY >= this.height) return false;
    return this.visibleGrid[tileY * this.width + tileX] === 1;
  }

  /**
   * @param {number} tileX
   * @param {number} tileY
   * @returns {boolean} true if the tile has ever been explored
   */
  isExplored(tileX, tileY) {
    if (this.revealAll) return true;
    if (tileX < 0 || tileY < 0 || tileX >= this.width || tileY >= this.height) return false;
    return this.exploredGrid[tileY * this.width + tileX] === 1;
  }

  setRevealAll(enabled) {
    this.revealAll = !!enabled;
    if (this.revealAll) {
      this.visibleGrid.fill(1);
      this.exploredGrid.fill(1);
    }
  }

  _tileVisibleFrom(fromX, fromY, tileX, tileY) {
    return this._rayClear(fromX, fromY, tileX + 0.5, tileY + 0.5, true);
  }

  _rayClear(fromX, fromY, toX, toY, allowOpaqueTarget) {
    if (![fromX, fromY, toX, toY].every(Number.isFinite)) return false;
    if (fromX < 0 || fromY < 0 || toX < 0 || toY < 0) return false;
    if (fromX >= this.width || toX >= this.width || fromY >= this.height || toY >= this.height) {
      return false;
    }

    const startX = Math.floor(fromX);
    const startY = Math.floor(fromY);
    const targetX = Math.floor(toX);
    const targetY = Math.floor(toY);
    if (startX === targetX && startY === targetY) {
      return allowOpaqueTarget || !this._terrainBlocks(targetX, targetY);
    }

    let tx = startX;
    let ty = startY;
    const dx = toX - fromX;
    const dy = toY - fromY;
    const stepX = Math.sign(dx);
    const stepY = Math.sign(dy);
    let tMaxX = this._firstBoundaryT(fromX, tx, dx, stepX);
    let tMaxY = this._firstBoundaryT(fromY, ty, dy, stepY);
    const tDeltaX = stepX === 0 ? Infinity : 1 / Math.abs(dx);
    const tDeltaY = stepY === 0 ? Infinity : 1 / Math.abs(dy);

    while (tx !== targetX || ty !== targetY) {
      if (tMaxX < tMaxY) {
        tx += stepX;
        tMaxX += tDeltaX;
        if (this._stepBlocks(tx, ty, targetX, targetY, allowOpaqueTarget)) return false;
      } else if (tMaxY < tMaxX) {
        ty += stepY;
        tMaxY += tDeltaY;
        if (this._stepBlocks(tx, ty, targetX, targetY, allowOpaqueTarget)) return false;
      } else {
        const nextTx = tx + stepX;
        const nextTy = ty + stepY;
        if (this._stepBlocks(nextTx, ty, targetX, targetY, allowOpaqueTarget)) return false;
        if (this._stepBlocks(tx, nextTy, targetX, targetY, allowOpaqueTarget)) return false;
        tx = nextTx;
        ty = nextTy;
        if (this._stepBlocks(tx, ty, targetX, targetY, allowOpaqueTarget)) return false;
        tMaxX += tDeltaX;
        tMaxY += tDeltaY;
      }
    }
    return true;
  }

  _firstBoundaryT(coord, tile, delta, step) {
    if (step > 0) return (tile + 1 - coord) / delta;
    if (step < 0) return (coord - tile) / -delta;
    return Infinity;
  }

  _stepBlocks(tileX, tileY, targetX, targetY, allowOpaqueTarget) {
    if (tileX < 0 || tileY < 0 || tileX >= this.width || tileY >= this.height) return true;
    if (allowOpaqueTarget && tileX === targetX && tileY === targetY) return false;
    return this._terrainBlocks(tileX, tileY);
  }

  _terrainBlocks(tileX, tileY) {
    if (!this.terrain) return false;
    return this.terrain[tileY * this.width + tileX] === TERRAIN.ROCK;
  }
}
