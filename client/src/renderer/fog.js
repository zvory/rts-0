import {
  COLORS,
  FOG_EXPLORED_ALPHA,
  FOG_UNEXPLORED_ALPHA,
  STATS,
  PLAYER_PALETTE,
  RESOURCE_AMOUNTS,
  ANTI_TANK_GUN_DEPLOYED_RANGE_TILES,
  ANTI_TANK_GUN_FIELD_OF_FIRE_RAD,
  isProducerBuilding,
} from "../config.js";
import { KIND, SETUP, STATE, isBuilding, isResource } from "../protocol.js";
import {
  DEPLOYED_WEAPON_ANIM_MS,
  SWEEP_EVICT_FRAMES,
  WEAPON_RECOIL_PX,
  ZERO_OFFSET,
} from "./palette.js";
import {
  angleDelta,
  clamp01,
  dashedLine,
  drawAntiTankGun,
  drawFacingWedge,
  drawImpassableEdge,
  drawInfantryBase,
  drawInfantryMachineGun,
  drawInfantryRifle,
  drawRotatedRect,
  drawScoutCar,
  drawTankFuelCue,
  drawTankHull,
  drawTankTracks,
  finiteNumber,
  hexToInt,
  isImpassableAt,
  isVehicleBodyKind,
  muzzleFlashRadius,
  normRect,
  polar,
  recoilVector,
  rectEdgePointTowardCenter,
  smoothstep01,
  tankBodyVisual,
  terrainColor,
  terrainOverlayColor,
  weaponRecoilOffset,
} from "./shared.js";

export function _drawFog(fog) {
  const g = this._fogGfx;
  this._recordRenderDiagnostic?.("renderer.graphics.clear.fog");
  g.clear();
  if (!fog || !this._map) return;
  const ts = this._map.tileSize;
  const w = fog.width;
  const h = fog.height;

  for (let ty = 0; ty < h; ty++) {
    // Run-length merge contiguous tiles sharing a fog level (0=clear,1=dim,2=dark,3=impassable-dim).
    let runStart = 0;
    let runLevel = this._fogLevel(fog, 0, ty);
    for (let tx = 1; tx <= w; tx++) {
      const level = tx < w ? this._fogLevel(fog, tx, ty) : -1;
      if (level !== runLevel) {
        if (runLevel > 0) {
          const color = runLevel === 2 ? COLORS.fogUnexplored : COLORS.fogExplored;
          const a = runLevel === 2
            ? FOG_UNEXPLORED_ALPHA
            : runLevel === 3
              ? FOG_UNEXPLORED_ALPHA * 0.56
              : FOG_EXPLORED_ALPHA;
          g.beginFill(color, a);
          g.drawRect(runStart * ts, ty * ts, (tx - runStart) * ts, ts);
          g.endFill();
        }
        runStart = tx;
        runLevel = level;
      }
    }
  }
}

export function _fogLevel(fog, tx, ty) {
  if (fog.isVisible(tx, ty)) return 0;
  if (this._map && isImpassableAt(this._map, tx, ty)) {
    return fog.isExplored(tx, ty) ? 0 : 3;
  }
  if (fog.isExplored(tx, ty)) return 1;
  return 2;
}
