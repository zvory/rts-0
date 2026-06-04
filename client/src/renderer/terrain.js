import {
  angleDelta,
  clamp01,
  dashedLine,
  drawAtGun,
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
  hash2,
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

export function buildStaticMap(map) {
  this._map = { width: map.width, height: map.height, tileSize: map.tileSize, terrain: map.terrain };
  const ts = map.tileSize;

  const g = new PIXI.Graphics();
  for (let ty = 0; ty < map.height; ty++) {
    for (let tx = 0; tx < map.width; tx++) {
      const code = map.terrain[ty * map.width + tx];
      let color = terrainColor(code, tx, ty);
      g.beginFill(color);
      g.drawRect(tx * ts, ty * ts, ts, ts);
      g.endFill();

      // Coarse texture blocks keep the ground readable while selling the
      // low-resolution PS1 look. No symbols or national markings are used.
      const blocks = ts >= 32 ? 4 : 2;
      const block = ts / blocks;
      for (let by = 0; by < blocks; by++) {
        for (let bx = 0; bx < blocks; bx++) {
          const n = hash2(tx * 17 + bx, ty * 17 + by);
          if (n < 0.42) continue;
          const overlay = terrainOverlayColor(code, n);
          g.beginFill(overlay, code === 2 ? 0.22 : 0.16);
          g.drawRect(tx * ts + bx * block, ty * ts + by * block, Math.ceil(block), Math.ceil(block));
          g.endFill();
        }
      }

      drawImpassableEdge(g, map, tx, ty, code, ts);
    }
  }

  // Rasterize to a texture so the (potentially huge) terrain is a single sprite.
  const tex = this.app.renderer.generateTexture(g, {
    region: new PIXI.Rectangle(0, 0, map.width * ts, map.height * ts),
    scaleMode: PIXI.SCALE_MODES.NEAREST,
  });
  g.destroy();

  const layer = this.layers.terrain;
  if (this._terrainSprite) {
    this._terrainSprite.destroy(true);
    layer.removeChildren();
  }
  this._terrainSprite = new PIXI.Sprite(tex);
  layer.addChild(this._terrainSprite);

}
