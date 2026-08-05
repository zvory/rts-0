import { COLORS } from "../client/src/config.js";
import { Minimap } from "../client/src/minimap.js";
import { minimapTerrainColor } from "../client/src/minimap_terrain.js";
import { TERRAIN } from "../client/src/protocol.js";
import { terrainColor } from "../client/src/renderer/terrain_palette.js";

const assert = (condition, message) => {
  if (!condition) throw new Error(message || "Assertion failed");
};

const recordingContext = (label) => ({
  label,
  calls: [],
  fillStyle: "",
  strokeStyle: "",
  lineWidth: 1,
  globalAlpha: 1,
  clearRect(...args) { this.calls.push({ op: "clearRect", args }); },
  fillRect(...args) { this.calls.push({ op: "fillRect", args, fillStyle: this.fillStyle }); },
  strokeRect(...args) { this.calls.push({ op: "strokeRect", args }); },
  drawImage(source, ...args) { this.calls.push({ op: "drawImage", source: source?.label, args }); },
  save() { this.calls.push({ op: "save" }); },
  restore() { this.calls.push({ op: "restore" }); },
  beginPath() { this.calls.push({ op: "beginPath" }); },
  arc(...args) { this.calls.push({ op: "arc", args }); },
  stroke() { this.calls.push({ op: "stroke" }); },
  fill() { this.calls.push({ op: "fill", fillStyle: this.fillStyle }); },
  moveTo(...args) { this.calls.push({ op: "moveTo", args }); },
  lineTo(...args) { this.calls.push({ op: "lineTo", args }); },
  closePath() { this.calls.push({ op: "closePath" }); },
  translate(...args) { this.calls.push({ op: "translate", args }); },
  rotate(...args) { this.calls.push({ op: "rotate", args }); },
});

const staticCanvasFactory = (layers) => () => {
  const label = `static-${layers.length}`;
  const context = recordingContext(label);
  const canvas = {
    label,
    width: 1,
    height: 1,
    getContext() { return context; },
  };
  layers.push({ canvas, context });
  return canvas;
};

const renderableCanvas = () => {
  const context = recordingContext("main");
  return {
    width: 24,
    height: 24,
    context,
    getContext() { return context; },
    getBoundingClientRect() { return { left: 0, top: 0, width: 24, height: 24 }; },
    addEventListener() {},
    removeEventListener() {},
  };
};

export function runMinimapRoadContracts() {
  globalThis.window = {
    devicePixelRatio: 1,
    addEventListener() {},
    removeEventListener() {},
  };
  const roadCodes = [
    TERRAIN.ROAD_BARE,
    TERRAIN.ROAD_HORIZONTAL,
    TERRAIN.ROAD_VERTICAL,
    TERRAIN.ROAD_DIAGONAL_NW_SE,
    TERRAIN.ROAD_DIAGONAL_NE_SW,
  ];
  for (const code of roadCodes) {
    const sampledColors = new Set();
    for (let ty = 0; ty < 16; ty++) {
      for (let tx = 0; tx < 16; tx++) {
        const minimapColor = minimapTerrainColor(code, tx, ty);
        sampledColors.add(minimapColor);
        assert(
          minimapColor === terrainColor(code, tx, ty),
          `minimap road ${code} matches the world surface at ${tx},${ty}`,
        );
      }
    }
    assert(sampledColors.size === 2, `minimap road ${code} uses both charcoal variants`);
    assert(sampledColors.has(COLORS.road), `minimap road ${code} uses the dark charcoal surface`);
    assert(sampledColors.has(COLORS.roadAlt), `minimap road ${code} uses the alternate charcoal surface`);
  }

  const layers = [];
  const canvas = renderableCanvas();
  const state = {
    playerId: 1,
    map: {
      width: 3,
      height: 2,
      tileSize: 1,
      terrain: [
        TERRAIN.ROAD_BARE, TERRAIN.ROAD_HORIZONTAL, TERRAIN.GRASS,
        TERRAIN.GRASS, TERRAIN.ROAD_VERTICAL, TERRAIN.GRASS,
      ],
      resources: [{ id: 10, kind: "steel", x: 2.5, y: 1.5, remaining: 100 }],
      doodads: [{ id: 11, typeId: "tree.oak", x: 1.5, y: 1.5 }],
    },
    selectedEntities() { return []; },
    entitiesInterpolated() { return []; },
    players: [],
  };
  const fog = {
    width: 3,
    height: 2,
    visibleGrid: new Uint8Array(6),
    exploredGrid: new Uint8Array(6),
    revision: 1,
    visibleRevision: 1,
    exploredRevision: 1,
    revealAll: false,
    isVisible() { return false; },
    isExplored() { return false; },
  };
  const camera = { viewportGroundPolygon() { return []; }, centerOn() {} };
  const minimap = new Minimap(canvas, state, camera, fog, { issueCommand() {} }, null, {
    staticCanvasFactory: staticCanvasFactory(layers),
  });

  minimap.render();
  assert(layers.length === 5, "marked-road minimap creates terrain, forest, fog, road, and resource layers");
  const [terrainLayer, forestLayer, fogLayer, roadLayer, resourceLayer] = layers;
  const drawIndex = (layer) => canvas.context.calls.findIndex((call) =>
    call.op === "drawImage" && call.source === layer.canvas.label);
  const terrainIndex = drawIndex(terrainLayer);
  const forestIndex = drawIndex(forestLayer);
  const fogIndex = drawIndex(fogLayer);
  const roadIndex = drawIndex(roadLayer);
  const resourceIndex = drawIndex(resourceLayer);
  assert(terrainIndex >= 0 && forestIndex > terrainIndex, "forest symbols draw above terrain");
  assert(fogIndex > forestIndex, "fog dims forest symbols together with terrain");
  assert(roadIndex > fogIndex, "yellow road dots stay visible above unexplored fog");
  assert(resourceIndex > roadIndex, "resource blips retain priority above road dots");
  const roadDots = roadLayer.context.calls.filter((call) => call.op === "arc");
  assert(roadDots.length === 2, "only authored marked-road tiles produce yellow dots");
  assert(
    roadLayer.context.calls.filter((call) => call.op === "fill")
      .every((call) => call.fillStyle === "#f1cb43"),
    "marked-road dots use the dedicated bright yellow",
  );

  minimap.render();
  assert(
    roadLayer.context.calls.filter((call) => call.op === "arc").length === roadDots.length,
    "second render reuses the cached road-marking layer",
  );
  minimap.destroy();
}
