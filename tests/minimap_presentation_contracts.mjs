import { Minimap } from "../client/src/minimap.js";
import { KIND } from "../client/src/protocol.js";
import { strict as assert } from "node:assert";

// Live backing resolution follows CSS/DPR, while captures and rematches keep explicit sizes.
export function runMinimapPresentationContracts({ installWindowStub, fakeRenderableCanvas }) {
  const host = installWindowStub();
  const canvas = fakeRenderableCanvas({ width: 242, height: 242 });
  canvas.ownerDocument = { defaultView: host };
  const state = { map: null, playerId: 1, players: [
    { id: 1, color: "#0072b2" }, { id: 2, color: "#d55e00" },
  ] };
  const minimap = new Minimap(canvas, state, null, null, null);
  minimap.render();
  assert(canvas.width === 484, "live minimap supersamples at DPR 1");
  host.devicePixelRatio = 3;
  minimap.render();
  assert(canvas.width === 726, "live minimap respects higher DPR");
  const ctx = canvas.context;
  minimap._viewportGroundPolygon = () => [{ x: 0, y: 0 }, { x: 100, y: 100 }];
  minimap._worldToCanvas = (x, y) => ({ x: x * 3, y: y * 3 });
  minimap._drawViewport();
  assert(ctx.calls.findLast(call => call.op === "stroke").lineWidth === 3,
    "camera outline scales with backing resolution");
  minimap._pings = [{ x: 20, y: 30, startedAt: 100, isUnderAttack: true }];
  minimap._borderPulseUntil = 800;
  ctx.calls.length = 0;
  minimap._drawPings(100);
  assert.deepEqual(ctx.calls.find(call => call.op === "scale").args, [3, 3],
    "ping rings and border pulse scale with backing resolution");
  assert.deepEqual(ctx.calls.find(call => call.op === "arc").args.slice(0, 3), [20, 30, 32],
    "scaled ping coordinates retain their world position and original radius");
  assert.deepEqual(ctx.calls.find(call => call.op === "strokeRect").args, [1.5, 1.5, 239, 239],
    "scaled border pulse still fits the canvas");
  ctx.calls.length = 0;
  minimap._drawArtilleryMarker(60, 90, 0, "#0072b2", 0);
  assert.deepEqual(ctx.calls.find(call => call.op === "translate").args, [60, 90]);
  assert.deepEqual(ctx.calls.find(call => call.op === "scale").args, [3, 3],
    "artillery markers scale without moving their center");
  canvas.width = canvas.height = 960;
  minimap.render(null, { capturePresentation: true });
  assert(canvas.width === 960, "explicit capture resolution is preserved");
  assert(minimap._blipColor({ owner: 1, kind: KIND.BARRACKS }) === "#0072b2",
    "own buildings use assigned player color");
  assert(minimap._blipColor({ owner: 2, kind: KIND.BARRACKS }) === "#d55e00",
    "other buildings use assigned player color");
  minimap.destroy();
  assert(canvas.width === 242 && canvas.height === 242, "teardown restores rematch base dimensions");
}
