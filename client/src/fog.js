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
// solid dark when never explored (the renderer reads the grids directly). `revision`
// increments only when those visibility semantics change so canvas overlays can cache
// their fog view between identical frames. The full union is rebuilt every update;
// exact per-entity stamps may be reused only while every sight input is unchanged.

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
    this.revision = 0;
    this.visibleRevision = 0;
    this.exploredRevision = 0;
    this._nextVisibleGrid = new Uint8Array(mapWidth * mapHeight);
    this._sourceStampCache = new Map();
    this._sourceStampGeneration = 0;
  }

  resetMap(mapWidth, mapHeight, terrain = null) {
    this.width = mapWidth;
    this.height = mapHeight;
    this.terrain = terrain;
    this.visibleGrid = new Uint8Array(mapWidth * mapHeight);
    this.exploredGrid = new Uint8Array(mapWidth * mapHeight);
    this._nextVisibleGrid = new Uint8Array(mapWidth * mapHeight);
    this._sourceStampCache.clear();
    this._sourceStampGeneration = 0;
    this.visibleRevision += 1;
    this.exploredRevision += 1;
    this.revision += 1;
  }

  updateTerrain(terrain = null) {
    this.terrain = terrain;
    this._sourceStampCache.clear();
    this.revision += 1;
  }

  /**
   * Recompute visibility for this frame from the player's own entities.
   *
   * Clears `visibleGrid`, then for each owned entity stamps its sight shape
   * (config `STATS[kind].sight`, in tiles) into both `visibleGrid` and `exploredGrid`.
   * Buildings reveal their footprint plus sight around the footprint edge; units reveal
   * a filled circle. Explored is cumulative and is never cleared.
   *
   * @param {Array<{id?:number,kind:string,x:number,y:number}>} ownEntities entities owned by this player (world px centers)
   * @param {number} tileSize world px per tile
   * @param {ArrayLike<number>|null} serverVisibleTiles server-authoritative current visibility
   */
  update(ownEntities, tileSize, serverVisibleTiles = null) {
    if (this.revealAll) {
      this._recordGridChanges(
        this._fillGridIfChanged(this.visibleGrid, 1),
        this._fillGridIfChanged(this.exploredGrid, 1),
      );
      return;
    }
    if (serverVisibleTiles && serverVisibleTiles.length === this.visibleGrid.length) {
      let visibleChanged = false;
      let exploredChanged = false;
      for (let i = 0; i < this.visibleGrid.length; i++) {
        const visible = serverVisibleTiles[i] ? 1 : 0;
        if (this.visibleGrid[i] !== visible) {
          this.visibleGrid[i] = visible;
          visibleChanged = true;
        }
        if (visible && this.exploredGrid[i] !== 1) {
          this.exploredGrid[i] = 1;
          exploredChanged = true;
        }
      }
      this._recordGridChanges(visibleChanged, exploredChanged);
      return;
    }
    const nextVisible = this._nextVisibleGrid;
    nextVisible.fill(0);
    let exploredChanged = false;

    if (ownEntities && tileSize) {
      const sourceStampGeneration = ++this._sourceStampGeneration;
      for (const e of ownEntities) {
        const stat = STATS[e.kind];
        const sight = (stat && stat.sight) || DEFAULT_SIGHT_TILES;
        const cx = e.x / tileSize;
        const cy = e.y / tileSize;
        const sourceId = e.id;
        const cached = sourceId == null ? null : this._sourceStampCache.get(sourceId);
        const sameSource = !!cached
          && cached.kind === e.kind
          && cached.x === e.x
          && cached.y === e.y
          && cached.tileSize === tileSize
          && cached.sight === sight
          && cached.footW === stat?.footW
          && cached.footH === stat?.footH;

        if (sameSource && cached.indices) {
          cached.seenGeneration = sourceStampGeneration;
          exploredChanged = this._applyCachedStamp(cached.indices, nextVisible) || exploredChanged;
          continue;
        }

        // Capture only after observing an unchanged source twice. Interpolated
        // movers therefore do not allocate a throwaway index list every frame.
        const capturedIndices = sameSource ? [] : null;
        if (stat?.footW && stat?.footH) {
          exploredChanged =
            this._stampFootprint(
              cx,
              cy,
              stat.footW,
              stat.footH,
              sight,
              nextVisible,
              capturedIndices,
            ) ||
            exploredChanged;
        } else {
          exploredChanged =
            this._stampCircle(cx, cy, sight, nextVisible, capturedIndices) || exploredChanged;
        }
        if (sourceId != null) {
          this._sourceStampCache.set(sourceId, {
            kind: e.kind,
            x: e.x,
            y: e.y,
            tileSize,
            sight,
            footW: stat?.footW,
            footH: stat?.footH,
            indices: capturedIndices ? Uint32Array.from(capturedIndices) : null,
            seenGeneration: sourceStampGeneration,
          });
        }
      }
      for (const [sourceId, entry] of this._sourceStampCache) {
        if (entry.seenGeneration !== sourceStampGeneration) this._sourceStampCache.delete(sourceId);
      }
    } else {
      this._sourceStampCache.clear();
    }

    let visibleChanged = false;
    for (let i = 0; i < this.visibleGrid.length; i++) {
      const visible = nextVisible[i];
      if (this.visibleGrid[i] !== visible) {
        this.visibleGrid[i] = visible;
        visibleChanged = true;
      }
    }
    this._recordGridChanges(visibleChanged, exploredChanged);
  }

  /**
   * Mark a filled circle of `radius` tiles centered on tile-space (`cx`, `cy`) as
   * visible and explored. Uses a squared-distance test so the reveal is round.
   * @private
   */
  _stampCircle(
    cx,
    cy,
    radius,
    visibleGrid = this.visibleGrid,
    capturedIndices = null,
  ) {
    const r2 = radius * radius;
    const minTx = Math.max(0, Math.floor(cx - radius));
    const maxTx = Math.min(this.width - 1, Math.ceil(cx + radius));
    const minTy = Math.max(0, Math.floor(cy - radius));
    const maxTy = Math.min(this.height - 1, Math.ceil(cy + radius));
    let exploredChanged = false;

    for (let ty = minTy; ty <= maxTy; ty++) {
      // Compare against tile centers so the disc is symmetric around the entity.
      const dy = ty + 0.5 - cy;
      const rowBase = ty * this.width;
      for (let tx = minTx; tx <= maxTx; tx++) {
        const dx = tx + 0.5 - cx;
        if (dx * dx + dy * dy <= r2 && this._tileVisibleFrom(cx, cy, tx, ty)) {
          const i = rowBase + tx;
          visibleGrid[i] = 1;
          capturedIndices?.push(i);
          if (this.exploredGrid[i] !== 1) {
            this.exploredGrid[i] = 1;
            exploredChanged = true;
          }
        }
      }
    }
    return exploredChanged;
  }

  /**
   * Mark a building footprint and its rectangular sight perimeter as visible and explored.
   * Matches the authoritative server rule: a building with sight 1 sees every tile it covers,
   * plus one tile out from each footprint edge.
   * @private
   */
  _stampFootprint(
    cx,
    cy,
    footW,
    footH,
    radius,
    visibleGrid = this.visibleGrid,
    capturedIndices = null,
  ) {
    if (![cx, cy, footW, footH, radius].every(Number.isFinite)) return false;
    const r = Math.floor(radius);
    const w = Math.floor(footW);
    const h = Math.floor(footH);
    if (r <= 0 || w <= 0 || h <= 0) return false;

    const centerTx = Math.floor(cx);
    const centerTy = Math.floor(cy);
    const originMinTx = centerTx - Math.floor(w / 2);
    const originMinTy = centerTy - Math.floor(h / 2);
    let exploredChanged = false;

    for (let oy = 0; oy < h; oy++) {
      for (let ox = 0; ox < w; ox++) {
        const originTx = originMinTx + ox;
        const originTy = originMinTy + oy;
        if (originTx < 0 || originTy < 0 || originTx >= this.width || originTy >= this.height) {
          continue;
        }
        const originX = originTx + 0.5;
        const originY = originTy + 0.5;

        for (let dy = -r; dy <= r; dy++) {
          const ty = originTy + dy;
          if (ty < 0 || ty >= this.height) continue;
          const rowBase = ty * this.width;
          for (let dx = -r; dx <= r; dx++) {
            const tx = originTx + dx;
            if (tx < 0 || tx >= this.width) continue;
            if (!this._tileVisibleFrom(originX, originY, tx, ty)) continue;
            const i = rowBase + tx;
            visibleGrid[i] = 1;
            capturedIndices?.push(i);
            if (this.exploredGrid[i] !== 1) {
              this.exploredGrid[i] = 1;
              exploredChanged = true;
            }
          }
        }
      }
    }

    return exploredChanged;
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
    const next = !!enabled;
    const modeChanged = this.revealAll !== next;
    this.revealAll = next;
    let visibleChanged = false;
    let exploredChanged = false;
    if (this.revealAll) {
      visibleChanged = this._fillGridIfChanged(this.visibleGrid, 1);
      exploredChanged = this._fillGridIfChanged(this.exploredGrid, 1);
    }
    this._recordGridChanges(visibleChanged, exploredChanged, modeChanged);
  }

  _fillGridIfChanged(grid, value) {
    let changed = false;
    for (let i = 0; i < grid.length; i++) {
      if (grid[i] !== value) {
        grid[i] = value;
        changed = true;
      }
    }
    return changed;
  }

  _recordGridChanges(visibleChanged, exploredChanged, semanticChanged = false) {
    if (visibleChanged) this.visibleRevision += 1;
    if (exploredChanged) this.exploredRevision += 1;
    if (visibleChanged || exploredChanged || semanticChanged) this.revision += 1;
  }

  _applyCachedStamp(indices, visibleGrid) {
    let exploredChanged = false;
    for (let i = 0; i < indices.length; i++) {
      const tileIndex = indices[i];
      visibleGrid[tileIndex] = 1;
      if (this.exploredGrid[tileIndex] !== 1) {
        this.exploredGrid[tileIndex] = 1;
        exploredChanged = true;
      }
    }
    return exploredChanged;
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
