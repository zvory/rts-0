import { assertDeepEqual } from "./assertions.mjs";
import { Minimap } from "../../client/src/minimap.js";
import { TERRAIN } from "../../client/src/protocol.js";
import { _drawFog, _fogLevel } from "../../client/src/renderer/fog.js";
import { RecordingGraphics } from "./pixi_fakes.mjs";

const visible = new Uint8Array([1, 0, 0, 0]);
const explored = new Uint8Array([1, 1, 0, 0]);
const fog = {
  width: 2,
  height: 2,
  revision: 1,
  visibleRevision: 1,
  exploredRevision: 1,
  revealAll: false,
  isVisible: (tx, ty) => visible[ty * 2 + tx] === 1,
  isExplored: (tx, ty) => explored[ty * 2 + tx] === 1,
};

{
  const graphics = new RecordingGraphics();
  const renderer = {
    _fogGfx: graphics,
    _fogRenderFog: null,
    _fogRenderMap: null,
    _fogRenderKey: null,
    _map: {
      width: 2,
      height: 2,
      tileSize: 32,
      terrain: new Array(4).fill(TERRAIN.GRASS),
    },
    _fogLevel(candidateFog, tx, ty) {
      return _fogLevel.call(this, candidateFog, tx, ty);
    },
  };

  _drawFog.call(renderer, fog);
  const fillAlphas = graphics.calls
    .filter(([name]) => name === "beginFill")
    .map(([, , alpha]) => alpha);
  assertDeepEqual(
    fillAlphas,
    [0.30, 0.60],
    "main-map fog uses the lighter explored and unexplored viewport opacities",
  );
}

{
  const fillAlphas = [];
  const context = {
    globalAlpha: 1,
    fillStyle: "",
    save() {},
    restore() {},
    fillRect() {
      fillAlphas.push(this.globalAlpha);
    },
  };
  const minimap = Object.assign(Object.create(Minimap.prototype), {
    _scale: 1,
    _worldToCanvas(x, y) {
      return { x, y };
    },
  });
  const map = {
    width: 2,
    height: 2,
    tileSize: 1,
    terrain: new Array(4).fill(TERRAIN.GRASS),
  };

  minimap._paintFog(context, map, fog);
  assertDeepEqual(
    fillAlphas,
    [0.48, 0.80],
    "minimap fog keeps its darker explored and unexplored opacities",
  );
}
