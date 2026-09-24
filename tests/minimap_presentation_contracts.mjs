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
