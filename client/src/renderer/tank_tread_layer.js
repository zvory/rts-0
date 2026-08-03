import { KIND, isRoadTerrain } from "../protocol.js";
import { mulberry32, rgba } from "./decals/selection.js";
import { createWorkerSafeCanvas } from "./raster_primitives.js";

const DOWNSAMPLE = 2;
const TILE_WORLD_SIZE = 512;
const SEGMENT_PADDING = 40;
const MIN_CONTACT_MOTION = 3;
const UPLOAD_INTERVAL_MS = 100;
const COVERAGE_CELL_WORLD_SIZE = 8;
const COVERAGE_HEADING_BINS = 32;
const MAX_LIVE_COVERAGE_CELLS = 65_536;
const TREAD_OPACITY_SCALE = 0.42;

/**
 * Renders approximate checkpointed tread history plus precise live marks for every tank currently
 * present in this client's fog-filtered entity view. Server chunks restore approximate history
 * after reconnects, replay seeks, and later fog discovery.
 */
export class TankTreadLayer {
  constructor({
    layer,
    pixi = globalThis.PIXI,
    createCanvas = createWorkerSafeCanvas,
    recordDiagnostic = null,
  } = {}) {
    this.layer = layer;
    this.pixi = pixi;
    this.createCanvas = createCanvas;
    this.recordDiagnostic = recordDiagnostic;
    this.tiles = new Map();
    this.poses = new Map();
    this.authoritativeTrailIds = new Set();
    this.authoritativeTrails = new Map();
    this.liveCoverage = new Set();
    this.worldWidth = 0;
    this.worldHeight = 0;
    this.mapTileSize = 32;
    this.mapWidthTiles = 0;
    this.mapHeightTiles = 0;
    this.roadTiles = new Set();
    this.authoritativeRevealDirty = false;
    this.lastVisibleFogRevision = null;
    this.nextUploadAt = 0;
    this.totalSegments = 0;
    this.textureUpdateCount = 0;
  }

  resetForMap(map) {
    this.destroy();
    if (!this.pixi?.Texture || !this.pixi?.Sprite || !this.layer) return false;
    const tileSize = Number.isFinite(map?.tileSize) ? map.tileSize : 32;
    this.worldWidth = Math.max(1, (map?.width || 1) * tileSize);
    this.worldHeight = Math.max(1, (map?.height || 1) * tileSize);
    this.mapTileSize = tileSize;
    this.mapWidthTiles = Math.max(1, map?.width || 1);
    this.mapHeightTiles = Math.max(1, map?.height || 1);
    for (let index = 0; index < this.mapWidthTiles * this.mapHeightTiles; index += 1) {
      if (isRoadTerrain(map?.terrain?.[index])) this.roadTiles.add(index);
    }
    return true;
  }

  stampVisibleTankPoses(entities) {
    if (!Array.isArray(entities) ||
        this.worldWidth <= 0 || this.worldHeight <= 0) return 0;
    const now = globalThis.performance?.now?.() ?? Date.now();
    const visibleTanks = entities.filter((entity) => entity?.kind === KIND.TANK && entity.hp > 0 &&
      Number.isSafeInteger(entity.id) && finitePose(entity));
    const seen = new Set(visibleTanks.map((entity) => entity.id));
    // Visibility is authoritative even between the bounded texture uploads. Forget a tank as soon
    // as it leaves the projected entity set so a brief fog/smoke gap can never be bridged by an
    // inferred trail when the same id reappears.
    for (const id of this.poses.keys()) {
      if (!seen.has(id)) this.poses.delete(id);
    }
    if (now < this.nextUploadAt) {
      for (const entity of visibleTanks) {
        if (!this.poses.has(entity.id)) {
          this.poses.set(entity.id, treadPose(entity.x, entity.y, entity.facing));
        }
      }
      return 0;
    }
    const dirty = new Set();
    let stamped = 0;
    for (const entity of visibleTanks) {
      const previous = this.poses.get(entity.id);
      const current = treadPose(entity.x, entity.y, entity.facing, previous);
      if (!previous) {
        this.poses.set(entity.id, current);
        continue;
      }
      const contactMotion = Math.hypot(current.x - previous.x, current.y - previous.y) +
        Math.abs(shortestAngleDelta(previous.facing, current.facing)) * 29;
      if (contactMotion < MIN_CONTACT_MOTION) continue;
      const segment = treadSegment(previous, current, entity.id);
      if (this._stampSegment(segment, dirty)) {
        stamped += 1;
        this._rememberLiveCoverage(segment);
      }
      this.poses.set(entity.id, current);
    }
    if (stamped > 0) {
      this.nextUploadAt = now + UPLOAD_INTERVAL_MS;
      for (const tile of dirty) tile.texture?.source?.update?.();
      this.totalSegments += stamped;
      this.textureUpdateCount += dirty.size;
      this.recordDiagnostic?.("renderer.tankTreads.segments", stamped);
      this.recordDiagnostic?.("renderer.tankTreads.tileUploads", dirty.size);
    }
    return stamped;
  }

  stampAuthoritativeTrails(trails) {
    if (!Array.isArray(trails) || this.worldWidth <= 0 || this.worldHeight <= 0) return 0;
    let accepted = 0;
    for (const trail of trails) {
      if (!Number.isSafeInteger(trail?.id) || trail.id <= 0) continue;
      if (this.authoritativeTrailIds.has(trail.id)) {
        accepted += 1;
        continue;
      }
      const poses = unpackAuthoritativePoses(trail.poses);
      if (poses.length < 2) continue;
      const segments = [];
      let previous = treadPose(poses[0].x, poses[0].y, poses[0].facing);
      for (let index = 1; index < poses.length; index += 1) {
        const pose = poses[index];
        const current = treadPose(pose.x, pose.y, pose.facing, previous);
        const segment = treadSegment(previous, current, trail.id ^ index);
        if (!this._consumeLiveCoverage(segment)) segments.push(segment);
        previous = current;
      }
      const segmentsByTile = this._segmentsByMapTile(segments);
      this.authoritativeTrailIds.add(trail.id);
      if (segmentsByTile.size > 0) this.authoritativeTrails.set(trail.id, segmentsByTile);
      this.authoritativeRevealDirty = true;
      accepted += 1;
    }
    return accepted;
  }

  revealAuthoritativeTrails(fog = null) {
    if (typeof fog?.isVisible !== "function" || this.authoritativeTrails.size === 0 ||
        this.worldWidth <= 0 || this.worldHeight <= 0) {
      return 0;
    }
    const fogRevision = Number.isSafeInteger(fog.visibleRevision)
      ? fog.visibleRevision
      : Number.isSafeInteger(fog.revision) ? fog.revision : null;
    if (!this.authoritativeRevealDirty && fogRevision !== null &&
        fogRevision === this.lastVisibleFogRevision) return 0;
    const dirty = new Set();
    let stamped = 0;
    for (const [trailId, segmentsByTile] of this.authoritativeTrails) {
      for (const [tileIndex, segments] of segmentsByTile) {
        const tx = tileIndex % this.mapWidthTiles;
        const ty = Math.floor(tileIndex / this.mapWidthTiles);
        if (!fog.isVisible(tx, ty)) continue;
        const clip = {
          x: tx * this.mapTileSize,
          y: ty * this.mapTileSize,
          width: this.mapTileSize,
          height: this.mapTileSize,
        };
        for (const segment of segments) {
          if (this._stampSegment(segment, dirty, clip)) stamped += 1;
        }
        segmentsByTile.delete(tileIndex);
      }
      if (segmentsByTile.size === 0) this.authoritativeTrails.delete(trailId);
    }
    this.authoritativeRevealDirty = false;
    this.lastVisibleFogRevision = fogRevision;
    if (stamped > 0) {
      for (const tile of dirty) tile.texture?.source?.update?.();
      this.totalSegments += stamped;
      this.textureUpdateCount += dirty.size;
      this.recordDiagnostic?.("renderer.tankTreads.authoritativeSegments", stamped);
      this.recordDiagnostic?.("renderer.tankTreads.tileUploads", dirty.size);
    }
    return stamped;
  }

  diagnostics() {
    return {
      tileCount: this.tiles.size,
      tileTextureSize: TILE_WORLD_SIZE / DOWNSAMPLE,
      totalSegments: this.totalSegments,
      textureUpdateCount: this.textureUpdateCount,
    };
  }

  destroy() {
    this.poses.clear();
    this.authoritativeTrailIds.clear();
    this.authoritativeTrails.clear();
    this.liveCoverage.clear();
    this.nextUploadAt = 0;
    this.authoritativeRevealDirty = false;
    this.lastVisibleFogRevision = null;
    for (const tile of this.tiles.values()) {
      if (tile.sprite?.parent && typeof tile.sprite.parent.removeChild === "function") {
        tile.sprite.parent.removeChild(tile.sprite);
      }
      tile.sprite?.destroy?.({ children: true, texture: false, baseTexture: false });
      tile.texture?.destroy?.(true);
      tile.canvas.width = 0;
      tile.canvas.height = 0;
    }
    this.tiles.clear();
    this.worldWidth = 0;
    this.worldHeight = 0;
    this.mapTileSize = 32;
    this.mapWidthTiles = 0;
    this.mapHeightTiles = 0;
    this.roadTiles.clear();
    this.totalSegments = 0;
    this.textureUpdateCount = 0;
  }

  _segmentsByMapTile(segments) {
    const byTile = new Map();
    for (const segment of segments) {
      const minX = Math.max(0, Math.min(segment.previousX, segment.x) - SEGMENT_PADDING);
      const minY = Math.max(0, Math.min(segment.previousY, segment.y) - SEGMENT_PADDING);
      const maxX = Math.min(this.worldWidth, Math.max(segment.previousX, segment.x) + SEGMENT_PADDING);
      const maxY = Math.min(this.worldHeight, Math.max(segment.previousY, segment.y) + SEGMENT_PADDING);
      const firstTx = Math.floor(minX / this.mapTileSize);
      const firstTy = Math.floor(minY / this.mapTileSize);
      const lastTx = Math.floor(Math.max(minX, maxX - 0.001) / this.mapTileSize);
      const lastTy = Math.floor(Math.max(minY, maxY - 0.001) / this.mapTileSize);
      for (let ty = firstTy; ty <= lastTy; ty += 1) {
        for (let tx = firstTx; tx <= lastTx; tx += 1) {
          if (tx < 0 || ty < 0 || tx >= this.mapWidthTiles || ty >= this.mapHeightTiles) continue;
          const tileIndex = ty * this.mapWidthTiles + tx;
          const bucket = byTile.get(tileIndex) || [];
          bucket.push(segment);
          byTile.set(tileIndex, bucket);
        }
      }
    }
    return byTile;
  }

  _stampSegment(segment, dirty, clip = null) {
    const minX = Math.max(0, Math.min(segment.previousX, segment.x) - SEGMENT_PADDING);
    const minY = Math.max(0, Math.min(segment.previousY, segment.y) - SEGMENT_PADDING);
    const maxX = Math.min(this.worldWidth, Math.max(segment.previousX, segment.x) + SEGMENT_PADDING);
    const maxY = Math.min(this.worldHeight, Math.max(segment.previousY, segment.y) + SEGMENT_PADDING);
    const firstTx = Math.floor(minX / TILE_WORLD_SIZE);
    const firstTy = Math.floor(minY / TILE_WORLD_SIZE);
    const lastTx = Math.floor(Math.max(minX, maxX - 0.001) / TILE_WORLD_SIZE);
    const lastTy = Math.floor(Math.max(minY, maxY - 0.001) / TILE_WORLD_SIZE);
    let painted = false;
    for (let ty = firstTy; ty <= lastTy; ty += 1) {
      for (let tx = firstTx; tx <= lastTx; tx += 1) {
        const tile = this._tile(tx, ty);
        if (!tile) continue;
        tile.ctx.save();
        tile.ctx.translate(-(tx * TILE_WORLD_SIZE) / DOWNSAMPLE, -(ty * TILE_WORLD_SIZE) / DOWNSAMPLE);
        if (clip) {
          tile.ctx.beginPath();
          tile.ctx.rect(
            clip.x / DOWNSAMPLE,
            clip.y / DOWNSAMPLE,
            clip.width / DOWNSAMPLE,
            clip.height / DOWNSAMPLE,
          );
          tile.ctx.clip();
        }
        stampTankTreads(tile.ctx, segment, DOWNSAMPLE);
        this._clearRoadPixels(tile.ctx, tx, ty, minX, minY, maxX, maxY, clip);
        tile.ctx.restore();
        dirty.add(tile);
        painted = true;
      }
    }
    return painted;
  }

  _clearRoadPixels(ctx, textureTx, textureTy, minX, minY, maxX, maxY, clip) {
    if (this.roadTiles.size === 0) return;
    const textureMinX = textureTx * TILE_WORLD_SIZE;
    const textureMinY = textureTy * TILE_WORLD_SIZE;
    const textureMaxX = textureMinX + TILE_WORLD_SIZE;
    const textureMaxY = textureMinY + TILE_WORLD_SIZE;
    const clearMinX = Math.max(minX, textureMinX, clip?.x ?? minX);
    const clearMinY = Math.max(minY, textureMinY, clip?.y ?? minY);
    const clearMaxX = Math.min(maxX, textureMaxX, clip ? clip.x + clip.width : maxX);
    const clearMaxY = Math.min(maxY, textureMaxY, clip ? clip.y + clip.height : maxY);
    if (clearMinX >= clearMaxX || clearMinY >= clearMaxY) return;
    const firstTx = Math.max(0, Math.floor(clearMinX / this.mapTileSize));
    const firstTy = Math.max(0, Math.floor(clearMinY / this.mapTileSize));
    const lastTx = Math.min(this.mapWidthTiles - 1,
      Math.floor(Math.max(clearMinX, clearMaxX - 0.001) / this.mapTileSize));
    const lastTy = Math.min(this.mapHeightTiles - 1,
      Math.floor(Math.max(clearMinY, clearMaxY - 0.001) / this.mapTileSize));
    for (let mapTy = firstTy; mapTy <= lastTy; mapTy += 1) {
      for (let mapTx = firstTx; mapTx <= lastTx; mapTx += 1) {
        if (!this.roadTiles.has(mapTy * this.mapWidthTiles + mapTx)) continue;
        const roadX = mapTx * this.mapTileSize;
        const roadY = mapTy * this.mapTileSize;
        const roadClearX = Math.max(roadX, clearMinX);
        const roadClearY = Math.max(roadY, clearMinY);
        const roadClearMaxX = Math.min(roadX + this.mapTileSize, clearMaxX);
        const roadClearMaxY = Math.min(roadY + this.mapTileSize, clearMaxY);
        ctx.clearRect(roadClearX / DOWNSAMPLE, roadClearY / DOWNSAMPLE,
          (roadClearMaxX - roadClearX) / DOWNSAMPLE,
          (roadClearMaxY - roadClearY) / DOWNSAMPLE);
      }
    }
  }

  _rememberLiveCoverage(segment) {
    for (const pose of coveragePoses(segment)) {
      this.liveCoverage.add(coverageKey(pose.x, pose.y, pose.facing, this.worldWidth));
    }
    while (this.liveCoverage.size > MAX_LIVE_COVERAGE_CELLS) {
      const oldest = this.liveCoverage.values().next().value;
      this.liveCoverage.delete(oldest);
    }
  }

  _consumeLiveCoverage(segment) {
    const matched = [];
    for (const pose of coveragePoses(segment)) {
      const key = nearbyCoverageKey(pose, this.liveCoverage, this.worldWidth);
      if (key === null) return false;
      matched.push(key);
    }
    for (const key of matched) this.liveCoverage.delete(key);
    return matched.length > 0;
  }

  _tile(tx, ty) {
    const key = `${tx}:${ty}`;
    const existing = this.tiles.get(key);
    if (existing) return existing;
    const canvas = this.createCanvas();
    canvas.width = TILE_WORLD_SIZE / DOWNSAMPLE;
    canvas.height = TILE_WORLD_SIZE / DOWNSAMPLE;
    const ctx = canvas.getContext("2d", { alpha: true });
    if (!ctx) return null;
    ctx.imageSmoothingEnabled = false;
    const texture = this.pixi.Texture.from(canvas);
    const sprite = new this.pixi.Sprite(texture);
    sprite.position.set(tx * TILE_WORLD_SIZE, ty * TILE_WORLD_SIZE);
    sprite.scale.set(DOWNSAMPLE);
    this.layer.addChild(sprite);
    const tile = { canvas, ctx, texture, sprite };
    this.tiles.set(key, tile);
    return tile;
  }
}

function unpackAuthoritativePoses(records) {
  const poses = [];
  for (const record of Array.isArray(records) ? records : []) {
    if (!Array.isArray(record) || record.length !== 3 ||
        !Number.isInteger(record[0]) || !Number.isInteger(record[1]) ||
        !Number.isInteger(record[2]) || record[0] < 0 || record[0] > 0xffff ||
        record[1] < 0 || record[1] > 0xffff || record[2] < -0x8000 ||
        record[2] > 0x7fff) continue;
    poses.push({
      x: record[0] / 4,
      y: record[1] / 4,
      facing: record[2] * Math.PI / 32767,
    });
  }
  return poses;
}

function finitePose(entity) {
  return Number.isFinite(entity.x) && Number.isFinite(entity.y) && Number.isFinite(entity.facing);
}

function treadPose(x, y, facing, previous = null) {
  if (!previous) return { x, y, facing, treadPhaseLeft: 0, treadPhaseRight: 0 };
  const turn = shortestAngleDelta(previous.facing, facing);
  const middleFacing = previous.facing + turn * 0.5;
  const forwardTravel = (x - previous.x) * Math.cos(middleFacing) +
    (y - previous.y) * Math.sin(middleFacing);
  return {
    x,
    y,
    facing,
    treadPhaseLeft: previous.treadPhaseLeft + forwardTravel - turn * 11.8,
    treadPhaseRight: previous.treadPhaseRight + forwardTravel + turn * 11.8,
  };
}

function treadSegment(previous, current, seed) {
  return {
    seed,
    x: current.x,
    y: current.y,
    facing: current.facing,
    previousX: previous.x,
    previousY: previous.y,
    previousFacing: previous.facing,
    treadPhaseLeft: current.treadPhaseLeft,
    treadPhaseRight: current.treadPhaseRight,
  };
}

function coveragePoses(segment) {
  const turn = shortestAngleDelta(segment.previousFacing, segment.facing);
  const travel = Math.hypot(segment.x - segment.previousX, segment.y - segment.previousY);
  const steps = Math.max(1, Math.ceil((travel + Math.abs(turn) * 29) / 4));
  const poses = [];
  for (let step = 0; step <= steps; step += 1) {
    poses.push(interpolatedPose(segment, turn, step / steps));
  }
  return poses;
}

function coverageKey(x, y, facing, worldWidth) {
  const width = Math.ceil(worldWidth / COVERAGE_CELL_WORLD_SIZE);
  const cellX = Math.floor(x / COVERAGE_CELL_WORLD_SIZE);
  const cellY = Math.floor(y / COVERAGE_CELL_WORLD_SIZE);
  const heading = normalizedHeadingBin(facing);
  return ((cellY * width + cellX) * COVERAGE_HEADING_BINS) + heading;
}

function nearbyCoverageKey(pose, coverage, worldWidth) {
  const heading = normalizedHeadingBin(pose.facing);
  for (let dy = -1; dy <= 1; dy += 1) {
    for (let dx = -1; dx <= 1; dx += 1) {
      for (let dh = -1; dh <= 1; dh += 1) {
        const candidateHeading = (heading + dh + COVERAGE_HEADING_BINS) % COVERAGE_HEADING_BINS;
        const key = coverageKey(
          pose.x + dx * COVERAGE_CELL_WORLD_SIZE,
          pose.y + dy * COVERAGE_CELL_WORLD_SIZE,
          candidateHeading * Math.PI * 2 / COVERAGE_HEADING_BINS,
          worldWidth,
        );
        if (coverage.has(key)) return key;
      }
    }
  }
  return null;
}

function normalizedHeadingBin(facing) {
  const normalized = ((facing % (Math.PI * 2)) + Math.PI * 2) % (Math.PI * 2);
  return Math.round(normalized * COVERAGE_HEADING_BINS / (Math.PI * 2)) % COVERAGE_HEADING_BINS;
}

function stampTankTreads(ctx, segment, downsample) {
  const turn = shortestAngleDelta(segment.previousFacing, segment.facing);
  const travel = Math.hypot(segment.x - segment.previousX, segment.y - segment.previousY);
  const steps = Math.max(1, Math.min(48, Math.ceil((travel + Math.abs(turn) * 29) / 1.25)));
  const middleFacing = segment.previousFacing + turn * 0.5;
  const forwardTravel = (segment.x - segment.previousX) * Math.cos(middleFacing) +
    (segment.y - segment.previousY) * Math.sin(middleFacing);
  const leftDelta = forwardTravel - turn * 11.8;
  const rightDelta = forwardTravel + turn * 11.8;
  const previousLeft = segment.treadPhaseLeft - leftDelta;
  const previousRight = segment.treadPhaseRight - rightDelta;
  const rng = mulberry32(segment.seed || 1);

  for (let step = 1; step <= steps; step += 1) {
    const t = step / steps;
    const pose = interpolatedPose(segment, turn, t);
    const churn = Math.min(1, Math.abs(turn) * 9);
    ctx.fillStyle = rgba(0x4a3520,
      ((0.004 + churn * 0.018) / steps) * TREAD_OPACITY_SCALE);
    stampContactBed(ctx, pose, downsample, -11.8);
    stampContactBed(ctx, pose, downsample, 11.8);
    ctx.fillStyle = rgba(0x241a10,
      ((0.035 + rng() * 0.008) / Math.sqrt(steps)) * TREAD_OPACITY_SCALE);
    stampShoes(ctx, pose, downsample, -11.8, lerp(previousLeft, segment.treadPhaseLeft, t));
    stampShoes(ctx, pose, downsample, 11.8, lerp(previousRight, segment.treadPhaseRight, t));
  }
  stampShear(ctx, segment, downsample, -11.8, leftDelta, rng);
  stampShear(ctx, segment, downsample, 11.8, rightDelta, rng);
}

function interpolatedPose(segment, turn, t) {
  return {
    x: lerp(segment.previousX, segment.x, t),
    y: lerp(segment.previousY, segment.y, t),
    facing: segment.previousFacing + turn * t,
  };
}

function stampContactBed(ctx, pose, downsample, lateral) {
  ctx.save();
  ctx.translate(pose.x / downsample, pose.y / downsample);
  ctx.rotate(pose.facing);
  ctx.fillRect(-25 / downsample, (lateral - 3.7) / downsample, 50 / downsample, 7.4 / downsample);
  ctx.restore();
}

function stampShoes(ctx, pose, downsample, lateral, phase) {
  const spacing = 7;
  const first = -25 + positiveModulo(-phase + 25, spacing);
  ctx.save();
  ctx.translate(pose.x / downsample, pose.y / downsample);
  ctx.rotate(pose.facing);
  for (let longitudinal = first; longitudinal <= 25; longitudinal += spacing) {
    ctx.fillRect((longitudinal - 1.15) / downsample, (lateral - 4.1) / downsample,
      2.3 / downsample, 8.2 / downsample);
  }
  ctx.restore();
}

function stampShear(ctx, segment, downsample, lateral, beltDelta, rng) {
  const before = { x: segment.previousX, y: segment.previousY, facing: segment.previousFacing };
  const after = { x: segment.x, y: segment.y, facing: segment.facing };
  for (let longitudinal = -22; longitudinal <= 22; longitudinal += 11) {
    const from = treadPoint(before, longitudinal, lateral);
    const to = treadPoint(after, longitudinal - beltDelta, lateral);
    const slip = Math.hypot(to.x - from.x, to.y - from.y);
    if (slip < 0.08) continue;
    ctx.fillStyle = rgba(0x2c2014,
      Math.min(0.16, 0.025 + slip * 0.045) * (0.88 + rng() * 0.24) * TREAD_OPACITY_SCALE);
    stampSweptRect(ctx, from, to, 2.1, downsample);
  }
}

function treadPoint(pose, longitudinal, lateral) {
  const cos = Math.cos(pose.facing);
  const sin = Math.sin(pose.facing);
  return {
    x: pose.x + longitudinal * cos - lateral * sin,
    y: pose.y + longitudinal * sin + lateral * cos,
  };
}

function stampSweptRect(ctx, from, to, width, downsample) {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const length = Math.hypot(dx, dy);
  ctx.save();
  ctx.translate((from.x + to.x) / (2 * downsample), (from.y + to.y) / (2 * downsample));
  ctx.rotate(Math.atan2(dy, dx));
  ctx.fillRect(-length / (2 * downsample), -width / (2 * downsample),
    length / downsample, width / downsample);
  ctx.restore();
}

function shortestAngleDelta(from, to) {
  let delta = (to - from + Math.PI) % (Math.PI * 2);
  if (delta < 0) delta += Math.PI * 2;
  return delta - Math.PI;
}

function positiveModulo(value, modulus) {
  return ((value % modulus) + modulus) % modulus;
}

function lerp(from, to, t) {
  return from + (to - from) * t;
}
